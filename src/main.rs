use anyhow::{Context, Result}; 
use flate2::read::GzDecoder;
#[cfg(target_os = "linux")] 
use nix::sched::{unshare, CloneFlags};
use std::io::{self, Write};
use std::os::unix::fs as unix_fs;
use std::path::Path;
use tempfile::tempdir;
use reqwest;


#[cfg(target_os = "linux")]
fn unshare_pid() -> Result<()> {
    unshare(CloneFlags::CLONE_NEWPID)?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn unshare_pid() -> Result<()> {
    Ok(())
}

fn download_image_from_docker_and_store_in_filesystem(image: &str, temp_dir_path: &Path) -> Result<()> {
    // first, get an auth token
    let auth_url = format!("https://auth.docker.io/token?service=registry.docker.io&scope=repository:library/{}:pull", image);
    let auth_response = reqwest::blocking::get(&auth_url)?;
    let json_response: serde_json::Value = serde_json::from_str(&auth_response.text()?)?; 
    let token = json_response["token"].as_str().unwrap();

    // then, get the image manifest
    let url = format!("https://registry.hub.docker.com/v2/library/{}/manifests/latest", image);
    let manifest_response = reqwest::blocking::Client::new()
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.docker.distribution.manifest.v2+json")
        .send()?;

    let json_response: serde_json::Value = serde_json::from_str(&manifest_response.text()?)?;
    let layers = json_response["layers"].as_array().unwrap();

    // lastsly, download the layers and unpack them to the temporary directory
//    Object({"config": Object({"digest": String("sha256:1d34ffeaf190be23d3de5a8de0a436676b758f48f835c3a2d4768b798c15a7f1"), "mediaType": String("application/vnd.docker.container.image.
//v1+json"), "size": Number(1472)}), "layers": Array([Object({"digest": String("sha256:d25f557d7f31bf7acfac935859b5153da41d13c41f2b468d16f729a5b883634f"), "mediaType": String("application/vnd.dock
//er.image.rootfs.diff.tar.gzip"), "size": Number(3622094)})]), "mediaType": String("application/vnd.docker.distribution.manifest.v2+json"), "schemaVersion": Number(2)})

    for layer in layers {
        let digest = layer["digest"].as_str().unwrap();
        let layer_url = format!("https://registry.hub.docker.com/v2/library/{}/blobs/{}", image, digest);
        let layer_response = reqwest::blocking::Client::new()
            .get(&layer_url)
            .header("Authorization", format!("Bearer {}", token))
            .send()?;
        let layer_bytes = layer_response.bytes()?;
        let tar = GzDecoder::new(layer_bytes.as_ref());
        let mut layer_tar = tar::Archive::new(tar);
        layer_tar.set_preserve_permissions(true);
        layer_tar.set_unpack_xattrs(true);
        layer_tar.unpack(temp_dir_path)?;
    }
    Ok(())
}

// Usage: your_docker.sh run <image> <command> <arg1> <arg2> ...
fn main() -> Result<()> {
    // temporary directory to run the command in
    let temp_dir = tempdir()?;

    let args: Vec<_> = std::env::args().collect();
    // the image to run the command in
    let image = &args[2];
    // path to the command executable
    let command = &args[3];
    let command_args = &args[4..];

    // append the path to the command executable to the temporary directory
    let to_command_path = temp_dir
        .path()
        .join(command.strip_prefix('/').unwrap_or(command));
    // create parent directories before copying the command executable
    //std::fs::create_dir_all(to_command_path.parent().unwrap())?;
   
    download_image_from_docker_and_store_in_filesystem(&image, temp_dir.path())?;
    
    // create parent directories before copying the command executable
    std::fs::create_dir_all(to_command_path.parent().unwrap())?;

    // copy the command executable to the temporary directory
    std::fs::copy(command, to_command_path.clone())?;

    // chroot to the temporary directory
    unix_fs::chroot(temp_dir.path())?;
    // chdir to root since chroot doesn't change the working directory
    std::env::set_current_dir("/")?;
    // check that /dev/null exists in the chroot
    if !Path::new("dev/null").exists() {
        // create /dev/null if it doesn't exist
        std::fs::create_dir_all("dev")?;
        std::fs::File::create("dev/null")?;
    }
    // chroot doesn't change the working directory so commands that interact
    // with the filesystem may not work as expected unless we do this right after chroot
    unshare_pid()?;

    let output = std::process::Command::new(command)
        .args(command_args)
        .output()
        .with_context(|| {
            format!(
                "Tried to run '{}' with arguments {:?}",
                command, command_args
            )
        })?;

    let std_out = std::str::from_utf8(&output.stdout)?;
    let std_err = std::str::from_utf8(&output.stderr)?;
    print!("{}", std_out);
    eprint!("{}", std_err);
    io::stdout().flush()?;

    let exit_status = output.status.code().unwrap_or(1);
    std::process::exit(exit_status);
}
