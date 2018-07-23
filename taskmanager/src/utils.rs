//! This module contains different utility functions, such as command invocations

use failure::{Error, ResultExt};
use std::{
    ffi::{OsStr, OsString},
    fs, io,
    path::Path,
    process::{Command, Stdio},
};
use taskmanager::Executor;

/// Specifies the direction of the copy of the scp command
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ScpDirection {
    LocalToRemote,
    RemoteToLocal,
}

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

/// Copy files between local and remote in both directions
pub fn scp_file(
    executor: &Executor,
    direction: ScpDirection,
    local_path: &Path,
    remote_path: &Path,
) -> Result<(), Error> {
    let mut cmd = Command::new("scp");
    cmd.arg("-pr");
    let mut remote =
        OsString::with_capacity(executor.name.len() + remote_path.as_os_str().len() + 4);
    remote.push(&executor.name);
    remote.push(":");
    remote.push(remote_path);
    match direction {
        ScpDirection::LocalToRemote => cmd.args(&[local_path.as_os_str(), &*remote]),
        ScpDirection::RemoteToLocal => cmd.args(&[&*remote, local_path.as_os_str()]),
    };
    let res = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .context("Could not start scp")?;
    if !res.success() {
        bail!("scp did not finish successfully")
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
