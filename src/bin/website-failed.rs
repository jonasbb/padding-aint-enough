extern crate csv;
extern crate encrypted_dns;
extern crate env_logger;
extern crate failure;
extern crate glob;
extern crate misc_utils;
extern crate rayon;
extern crate sequences;
#[macro_use]
extern crate serde;
extern crate serde_json;
extern crate structopt;

use csv::WriterBuilder;
use encrypted_dns::{chrome::ChromeDebuggerMessage, ErrorExt};
use failure::{Error, ResultExt};
use glob::glob;
use misc_utils::fs::file_open_read;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sequences::common_sequence_classifications::*;
use std::{io, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, Serialize)]
struct Record {
    file: PathBuf,
    reason: &'static str,
}

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

    let encountered_error: bool;
    let stdout = io::stdout();
    {
        let mut writer = WriterBuilder::new()
            .has_headers(true)
            .from_writer(stdout.lock());

        encountered_error = cli_args
            .files_to_check
            .par_iter()
            .flat_map(|pattern| {
                glob(pattern)
                    .unwrap()
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>()
            })
            .map(|path| -> Result<(PathBuf, Option<&'static str>), Error> {
                // open file and parse it
                let mut rdr = file_open_read(&path)
                    .with_context(|_| format!("Failed to read {}", path.display()))?;
                let mut content = String::new();
                rdr.read_to_string(&mut content)
                    .with_context(|_| format!("Error while reading '{}'", path.display()))?;
                let msgs: Vec<ChromeDebuggerMessage> = serde_json::from_str(&content)
                    .with_context(|_| format!("Error while deserializing '{}'", path.display()))?;
                Ok((path, is_problematic_case(&msgs)))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|res| match res {
                Ok((path, Some(error))) => writer
                    .serialize(Record {
                        file: path,
                        reason: error,
                    })
                    .is_err(),
                Err(err) => {
                    eprintln!("{}", err.display_causes());
                    true
                }
                _ => false,
            })
            .any(|is_error| is_error);
        writer.flush().context("Flushing writer failed")?;
    }

    if encountered_error {
        std::process::exit(1)
    }

    Ok(())
}

fn is_problematic_case<S>(msgs: &[ChromeDebuggerMessage<S>]) -> Option<&'static str>
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
        return Some(R008);
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
        return Some(R009);
    }

    // default case is `false`, meaning the data is good
    None
}
