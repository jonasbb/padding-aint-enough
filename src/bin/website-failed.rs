use chrome::ChromeDebuggerMessage;
use csv::WriterBuilder;
use encrypted_dns::{chrome_log_contains_errors, ErrorExt};
use env_logger;
use failure::{Error, ResultExt};
use glob::glob;
use misc_utils::fs::file_open_read;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use serde::Serialize;
use serde_json;
use std::{io, path::PathBuf};
use structopt::{self, StructOpt};

#[derive(Debug, Serialize)]
struct Record {
    file: PathBuf,
    reason: &'static str,
}

#[derive(StructOpt, Debug)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
struct CliArgs {
    files_to_check: Vec<String>,
}

fn main() {
    use std::io::Write;

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
                Ok((path, chrome_log_contains_errors(&msgs)))
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
