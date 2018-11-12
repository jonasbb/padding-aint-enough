//! This module contains different utility functions, such as command invocations

use failure::{bail, Error, ResultExt};
use std::{
    ffi::OsStr,
    fs, io,
    path::Path,
    process::{Command, Stdio},
};

/// Compress a file with xz
pub fn xz(path: &Path) -> Result<(), Error> {
    // skip already compressed files
    if path.extension() == Some(OsStr::new("xz")) {
        return Ok(());
    }

    let res = Command::new("xz")
        .args(&["-9", "--force"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .context("Could not start xz")?;
    if !res.success() {
        bail!("xz did not finish successfully")
    }
    Ok(())
}

/// Ensure the given path exists and if not create it
pub fn ensure_path_exists(path: &Path) -> io::Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}

/// Run a docker container
///
/// * `image` specifies the docker image to use
/// * `extra_args` allows to pass extra configurations to docker. Make sure to not influence the `image`.
/// * `command` is an optional command to be run *inside* the docker container.
pub fn run_docker<I, S>(image: &str, extra_args: I, command: Option<&str>) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new("timeout");
    cmd.args(&[
        "--kill-after=2m",
        "1.5m",
        "docker",
        "run",
        "--privileged",
        "-v",
        "/tmp/.X11-unix:/tmp/.X11-unix:ro",
        "--dns=127.0.0.1",
        "--shm-size=2g",
        "--rm",
    ])
    .args(extra_args)
    .arg(image);
    if let Some(command) = command {
        cmd.arg(command);
    }
    cmd
}

pub fn docker_mount_option(host: &Path) -> (String, String) {
    ("-v".into(), format!("{}:/output", host.to_string_lossy()))
}

pub fn ensure_docker_image_exists(image: &str) -> Result<(), Error> {
    let output = Command::new("docker")
        .arg("images")
        .arg("-q")
        .arg(image)
        .output()?;
    if output.stdout.len() < 10 {
        bail!("Docker image {} does not exist.", image)
    }
    Ok(())
}
