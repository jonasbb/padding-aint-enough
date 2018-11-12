extern crate log;

use log::{debug, error, warn};
use std::{
    fmt,
    io::{self, Read},
    os::unix::process::ExitStatusExt,
    process::{Child, Command, Stdio},
    str::FromStr,
};

#[derive(Debug)]
pub struct Xvfb {
    /// Handle to the running Xvfb process
    process: Child,
    /// The X display opened by this instance of Xvfb
    display: XDisplay,
}

#[derive(Debug)]
pub enum ProcessStatus {
    Alive,
    Exited {
        exitcode: Option<i32>,
        signal: Option<i32>,
    },
    Error(io::Error),
}

impl Xvfb {
    pub fn new() -> io::Result<Self> {
        let mut process = Command::new("Xvfb")
            .args(&["-displayfd", "1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let mut display_num = String::with_capacity(5);
        let display = match process.stdout.as_mut() {
            Some(stdout) => {
                stdout.read_to_string(&mut display_num)?;
                XDisplay(u16::from_str(display_num.trim()).unwrap())
            }
            None => {
                error!("No stdout");
                unimplemented!()
            }
        };
        Ok(Xvfb { process, display })
    }

    pub fn get_display(&self) -> XDisplay {
        self.display
    }

    pub fn process_status(&mut self) -> ProcessStatus {
        match self.process.try_wait() {
            Ok(None) => ProcessStatus::Alive,
            Ok(Some(status)) => ProcessStatus::Exited {
                exitcode: status.code(),
                signal: status.signal(),
            },
            Err(err) => ProcessStatus::Error(err),
        }
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        match self.process.kill() {
            Ok(()) => debug!("Stopped Xvfb process {}", self.process.id()),
            Err(err) => {
                // InvalidInput is raise, if process was already dead
                if err.kind() != io::ErrorKind::InvalidInput {
                    warn!(
                        "Failed to stop Xvfb process {} due to {}",
                        self.process.id(),
                        err
                    )
                }
            }
        }
    }
}

/// Represents an X display number
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct XDisplay(u16);

impl fmt::Display for XDisplay {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, ":{}", self.0)
    }
}

#[test]
fn test_create_xvfb() {
    assert!(Xvfb::new().is_ok());
}

#[test]
fn test_x_display() {
    let xvfb = Xvfb::new().unwrap();
    assert!(format!("{}", xvfb.get_display()).starts_with(':'));
}
