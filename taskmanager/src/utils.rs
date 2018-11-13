//! This module contains different utility functions, such as command invocations

use failure::{bail, Error, ResultExt};
use std::{
    ffi::OsStr,
    fs, io,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};
use wait_timeout::ChildExt;

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
/// * `host_dir` Mounts the path to the `/output` location in the container and uses it for the container ID file
/// * `command` is an optional command to be run *inside* the docker container.
/// * `timeout` make sure the container is kill after the duration specified in timeout. This functions makes sure to kill and remove the container.
pub fn docker_run(
    image: &str,
    host_dir: &Path,
    command: Option<&str>,
    timeout: Duration,
) -> Result<ExitStatus, Error> {
    let mut cmd = Command::new("docker");
    cmd.args(&[
        "run",
        "--privileged",
        "--cpus",
        "1",
        &format!("--cidfile={}/cidfile", host_dir.to_string_lossy()),
        "-v",
        &format!("{}:/output", host_dir.to_string_lossy()),
        "-v",
        "/tmp/.X11-unix:/tmp/.X11-unix:ro",
        "--dns=127.0.0.1",
        "--shm-size=2g",
        "--rm",
    ])
    .arg(image)
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    if let Some(command) = command {
        cmd.arg(command);
    }
    let mut child = cmd.spawn()?;
    match child.wait_timeout(timeout) {
        Ok(Some(status)) => Ok(status.into()),
        Ok(None) => {
            // container has not exited yet
            let containerid = fs::read_to_string(host_dir.join("cidfile"))?;
            docker_kill(containerid.trim())?;
            Ok(child.wait()?.into())
        }
        Err(err) => {
            let containerid = fs::read_to_string(host_dir.join("cidfile"))?;
            docker_kill(containerid.trim())?;
            Err(err.into())
        }
    }
}

/// Make really really sure the docker container will not be running afterwards
///
/// Required the id of the container to kill.
fn docker_kill(containerid: &str) -> Result<(), Error> {
    let mut error_msg = String::new();
    let status = Command::new("docker")
        .args(&["kill", containerid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|_| format!("Could not kill docker container '{}'\n", containerid))?;
    if !status.success() {
        error_msg += &format!("Could not kill docker container '{}'\n", containerid);
    }
    let status = Command::new("docker")
        .args(&["rm", "--force=true", containerid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|_| format!("Could not remove docker container '{}'\n", containerid))?;
    if !status.success() {
        error_msg += &format!("Could not remove docker container '{}'\n", containerid);
    }
    if !error_msg.is_empty() {
        bail!(error_msg)
    } else {
        Ok(())
    }
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct ExitStatus {
    code: Option<i32>,
    signal: Option<i32>,
}

impl ExitStatus {
    pub fn success(&self) -> bool {
        self.code == Some(0) && self.signal == None
    }
}

impl From<wait_timeout::ExitStatus> for ExitStatus {
    fn from(status: wait_timeout::ExitStatus) -> Self {
        Self {
            code: status.code(),
            signal: status.unix_signal(),
        }
    }
}

#[cfg(unix)]
impl From<std::process::ExitStatus> for ExitStatus {
    fn from(status: std::process::ExitStatus) -> Self {
        use std::os::unix::process::ExitStatusExt;
        Self {
            code: status.code(),
            signal: status.signal(),
        }
    }
}
