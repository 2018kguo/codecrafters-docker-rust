use anyhow::{Context, Result};
use std::io::{self, Write};

// Usage: your_docker.sh run <image> <command> <arg1> <arg2> ...
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let command = &args[3];
    let command_args = &args[4..];
    let output = std::process::Command::new(command)
        .args(command_args)
        .output()
        .with_context(|| {
            format!(
                "Tried to run '{}' with arguments {:?}",
                command, command_args
            )
        })?;

    if output.status.success() {
        let std_out = std::str::from_utf8(&output.stdout)?;
        let std_err = std::str::from_utf8(&output.stderr)?;
        print!("{}", std_out);
        eprint!("{}", std_err);
        io::stdout().flush()?;
    } else {
        let exit_status = output.status.code().unwrap();
        std::process::exit(exit_status);
    }

    Ok(())
}
