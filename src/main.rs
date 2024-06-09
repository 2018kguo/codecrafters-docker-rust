use anyhow::{Context, Result};
use std::io::{self, Write};
use std::os::unix::fs as unix_fs;
use tempfile::tempdir;

// Usage: your_docker.sh run <image> <command> <arg1> <arg2> ...
fn main() -> Result<()> {
    // temporary directory to run the command in
    let temp_dir = tempdir()?;

    let args: Vec<_> = std::env::args().collect();
    // path to the command executable
    let command = &args[3];
    let command_args = &args[4..];

    // append the path to the command executable to the temporary directory
    let to_path = temp_dir
        .path()
        .join(command.strip_prefix('/').unwrap_or(command));
    // create parent directories before copying the command executable
    std::fs::create_dir_all(to_path.parent().unwrap())?;
    // copy the command executable to the temporary directory
    std::fs::copy(command, to_path)?;

    let dev_null_path = temp_dir.path().join("dev/null");
    // create dev/null directory and an empty file
    std::fs::create_dir(&dev_null_path.parent().unwrap())?;
    // create the empty file
    std::fs::File::create(&dev_null_path)?;

    // chroot to the temporary directory
    unix_fs::chroot(temp_dir.path())?;

    let output = std::process::Command::new(command)
        .args(command_args)
        .output()
        .with_context(|| {
            format!(
                "Tried to run '{}' with arguments {:?}",
                command, command_args
            )
        })?;

    //println!("{}", output.status.code().unwrap());
    //if output.status.success() {
    let std_out = std::str::from_utf8(&output.stdout)?;
    let std_err = std::str::from_utf8(&output.stderr)?;
    print!("{}", std_out);
    eprint!("{}", std_err);
    io::stdout().flush()?;
    //} else {
    let exit_status = output.status.code().unwrap_or(1);
    std::process::exit(exit_status);
    //}
}
