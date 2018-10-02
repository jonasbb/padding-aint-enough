extern crate encrypted_dns;
extern crate env_logger;
extern crate failure;
extern crate glob;
extern crate misc_utils;
extern crate rayon;
extern crate serde_json;
extern crate structopt;

use encrypted_dns::{chrome::ChromeDebuggerMessage, ErrorExt};
use failure::{Error, ResultExt};
use glob::glob;
use misc_utils::fs::file_open_read;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(
    author = "",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct CliArgs {
    files_to_check: Vec<String>,
}

fn main() {
    use std::io::{self, Write};

    if let Err(err) = run() {
        let stderr = io::stderr();
        let mut out = stderr.lock();
        // cannot handle a write error here, we are already in the outermost layer
        let _ = writeln!(out, "An error occured:");
        for fail in err.iter_chain() {
            let _ = writeln!(out, "  {}", fail);
        }
        let _ = writeln!(out, "{}", err.backtrace());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    // generic setup
    env_logger::init();
    let cli_args = CliArgs::from_args();

    let encountered_error: bool = cli_args
        .files_to_check
        .par_iter()
        .flat_map(|pattern| {
            glob(pattern)
                .unwrap()
                .filter_map(Result::ok)
                .collect::<Vec<_>>()
        })
        .map(|path| -> Result<(PathBuf, bool), Error> {
            // open file and parse it
            let mut rdr = file_open_read(&path)
                .with_context(|_| format!("Failed to read {}", path.display()))?;
            let mut content = String::new();
            rdr.read_to_string(&mut content)
                .with_context(|_| format!("Error while reading '{}'", path.display()))?;
            let msgs: Vec<ChromeDebuggerMessage<&str>> = serde_json::from_str(&content)
                .with_context(|_| format!("Error while deserializing '{}'", path.display()))?;
            Ok((path, is_problematic_case(&msgs)))
        })
        .map(|res| match res {
            Ok((path, is_error)) => {
                if is_error {
                    println!("{}", path.display());
                }
                is_error
            }
            Err(err) => {
                eprintln!("{}", err.display_causes());
                true
            }
        })
        .reduce(|| false, |accu, is_error| accu || is_error);

    if encountered_error {
        std::process::exit(1)
    }

    Ok(())
}

fn is_problematic_case<S>(msgs: &[ChromeDebuggerMessage<S>]) -> bool
where
    S: AsRef<str>,
{
    // test if there is a chrome error page
    let contains_chrome_error = msgs.iter().any(|msg| {
        if let ChromeDebuggerMessage::NetworkRequestWillBeSent { document_url, .. } = msg {
            document_url.as_ref() == "chrome-error://chromewebdata/"
        } else {
            false
        }
    });
    if contains_chrome_error {
        return true;
    }

    // Ensure at least one network request has succeeded.
    let contains_response_received = msgs.iter().any(|msg| {
        if let ChromeDebuggerMessage::NetworkResponseReceived { .. } = msg {
            true
        } else {
            false
        }
    });
    let contains_data_received = msgs.iter().any(|msg| {
        if let ChromeDebuggerMessage::NetworkDataReceived { .. } = msg {
            true
        } else {
            false
        }
    });
    if !(contains_response_received && contains_data_received) {
        return true;
    }

    // default case is `false`, meaning the data is good
    false
}
