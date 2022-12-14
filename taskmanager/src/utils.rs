//! This module contains different utility functions, such as command invocations

use anyhow::{bail, Context as _, Error};
use log::trace;
use std::{
    collections::HashMap,
    ffi::OsStr,
    fs, io,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, ExitStatus, Stdio},
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
        .args(&["-7", "--force"])
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
    environment: &HashMap<String, String>,
) -> Result<ExitStatus, Error> {
    // Change permissions, such that if a different user than the docker user creates the
    // host_dir, the docker container can still write to it
    let mut perms = fs::metadata(host_dir)?.permissions();
    perms.set_mode(0o777);
    fs::set_permissions(host_dir, perms)?;

    let mut cmd = Command::new("docker");
    cmd.args(&[
        "run",
        "--privileged",
        "--cpus",
        "4",
        &format!("--cidfile={}/cidfile", host_dir.to_string_lossy()),
        "-v",
        &format!("{}:/output", host_dir.to_string_lossy()),
        "-v",
        "/tmp/.X11-unix:/tmp/.X11-unix:ro",
        "--dns=127.0.0.1",
        "--shm-size=2g",
        "--sysctl=net.ipv6.conf.all.disable_ipv6=1",
        "--rm",
    ])
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    for (var_name, var_value) in environment {
        cmd.args(&["-e", &format!("{}={}", var_name, var_value)]);
    }
    cmd.arg(image);
    if let Some(command) = command {
        cmd.arg(command);
    }
    trace!("Execute command: {:?}", cmd);
    let mut child = cmd.spawn()?;
    match child.wait_timeout(timeout) {
        Ok(Some(status)) => Ok(status),
        Ok(None) => {
            // container has not exited yet
            let containerid = fs::read_to_string(host_dir.join("cidfile"))?;
            docker_kill(containerid.trim());
            // if docker container cannot be killed, at least kill the child process
            let _ = child.kill();
            Ok(child.wait()?)
        }
        Err(err) => {
            let containerid = fs::read_to_string(host_dir.join("cidfile"))?;
            docker_kill(containerid.trim());
            // if docker container cannot be killed, at least kill the child process
            let _ = child.kill();
            // try to reap it to avoid zombies
            let _ = child.try_wait();
            Err(err.into())
        }
    }
}

/// Run a docker container
///
/// * `image` specifies the docker image to use
/// * `host_dir` Mounts the path to the `/output` location in the container and uses it for the container ID file
/// * `command` is an optional command to be run *inside* the docker container.
/// * `timeout` make sure the container is kill after the duration specified in timeout. This functions makes sure to kill and remove the container.
pub fn docker_run_ssh(
    host: &str,
    image: &str,
    host_dir: &Path,
    command: Option<&str>,
    timeout: Duration,
    environment: &HashMap<String, String>,
) -> Result<ExitStatus, Error> {
    // Change permissions, such that if a different user than the docker user creates the
    // host_dir, the docker container can still write to it
    // let mut perms = fs::metadata(host_dir)?.permissions();
    // perms.set_mode(0o777);
    // fs::set_permissions(host_dir, perms)?;
    let status = Command::new("ssh")
        .args(&[host, "chmod", "-R", "0777"])
        .arg(host_dir)
        .status()?;
    if !status.success() {
        bail!("Cannot chmod the remote folder {}", host_dir.display());
    }

    let mut cmd = Command::new("ssh");
    cmd.args(&[
        host,
        "docker",
        "run",
        "--privileged",
        &format!("--cidfile={}/cidfile", host_dir.to_string_lossy()),
        "-v",
        &format!("{}:/output", host_dir.to_string_lossy()),
        "--dns=127.0.0.1",
        "--shm-size=2g",
        "--rm",
    ])
    .stdout(Stdio::null())
    .stderr(Stdio::null());
    for (var_name, var_value) in environment {
        cmd.args(&["-e", &format!("{}={}", var_name, var_value)]);
    }
    cmd.arg(image);
    if let Some(command) = command {
        cmd.arg(command);
    }
    trace!("Execute command: {:?}", cmd);
    let mut child = cmd.spawn()?;
    match child.wait_timeout(timeout) {
        Ok(Some(status)) => Ok(status),
        Ok(None) => {
            // container has not exited yet
            let output = Command::new("ssh")
                .arg(host)
                .arg("cat")
                .stderr(Stdio::null())
                .output()
                .context("Cannot read cidfile via SSH")?;
            let containerid = String::from_utf8_lossy(&output.stdout);
            docker_kill_ssh(host, containerid.trim());
            // if docker container cannot be killed, at least kill the child process
            let _ = child.kill();
            Ok(child.wait()?)
        }
        Err(err) => {
            let output = Command::new("ssh")
                .arg(host)
                .arg("cat")
                .stderr(Stdio::null())
                .output()
                .context("Cannot read cidfile via SSH")?;
            let containerid = String::from_utf8_lossy(&output.stdout);
            docker_kill_ssh(host, containerid.trim());
            // if docker container cannot be killed, at least kill the child process
            let _ = child.kill();
            // try to reap it to avoid zombies
            let _ = child.try_wait();
            Err(err.into())
        }
    }
}

/// Make really really sure the docker container will not be running afterwards
///
/// Required the id of the container to kill.
fn docker_kill(containerid: &str) {
    let _ = Command::new("docker")
        .args(&["kill", containerid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("docker")
        .args(&["rm", "--force=true", containerid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Like [`docker_kill`] but via SSH
fn docker_kill_ssh(host: &str, containerid: &str) {
    let _ = Command::new("ssh")
        .args(&[host, "docker", "kill", containerid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = Command::new("ssh")
        .args(&[host, "docker", "rm", "--force=true", containerid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Check if the docker image exists on the local machine
pub(crate) fn ensure_docker_image_exists(image: &str) -> Result<(), Error> {
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

/// Like [`ensure_docker_image_exists`] but via SSH
pub(crate) fn ensure_docker_image_exists_ssh(host: &str, image: &str) -> Result<(), Error> {
    let output = Command::new("ssh")
        .args(&[host, "docker", "images", "-q", image])
        .output()?;
    if output.stdout.len() < 10 {
        bail!("Docker image {} does not exist.", image)
    }
    Ok(())
}
