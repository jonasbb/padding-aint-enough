/// Extract TLDs from an Alexa Top x Domains file
///
/// Read the Alexa Top x Domains CSV file and collect all observable domains in it
extern crate csv;
extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate misc_utils;
#[macro_use]
extern crate structopt;
extern crate serde;
#[macro_use]
extern crate serde_derive;

use csv::ReaderBuilder;
use failure::{Error, ResultExt};
use misc_utils::fs::file_open_read;
use std::{
    collections::BTreeSet,
    io::{self, Write},
    path::PathBuf,
};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(author = "", raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
struct CliArgs {
    #[structopt(parse(from_os_str))]
    alexa_top_file: PathBuf,
    #[structopt(default_value = "std::u32::MAX")]
    limit: u32,
}

fn main() {
    use std::io::{self, Write};

    if let Err(err) = run() {
        let stderr = io::stderr();
        let mut out = stderr.lock();
        // cannot handle a write error here, we are already in the outermost layer
        let _ = writeln!(out, "An error occured:");
        for fail in err.causes() {
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
    info!("Process file '{}'", cli_args.alexa_top_file.display());

    let file = file_open_read(&cli_args.alexa_top_file).map_err(|err| {
        format_err!(
            "Opening alexa top file at '{}' failed: {}",
            cli_args.alexa_top_file.display(),
            err
        )
    })?;
    let mut rdr = ReaderBuilder::new()
        .delimiter(b',')
        .has_headers(false)
        .from_reader(file);

    let tlds: Result<BTreeSet<String>, _> = rdr
        .deserialize()
        .into_iter()
        .take(cli_args.limit as usize)
        .map(|record: Result<CsvLine, _>| {
            record.map(|r| {
                // Get everything after the last dot (the TLD)
                // Append a trailing dot to make sure it is interpreted as TLD
                [
                    r.domain
                        .rsplitn(2, '.')
                        .next()
                        .expect("The domain is never empty, thus one substring always exists."),
                    ".",
                ].join("")
            })
        })
        .collect::<Result<BTreeSet<String>, _>>()
        .context("Failed to process Alexa top x list");
    let out = io::stdout();
    let mut stdout = out.lock();
    for tld in tlds? {
        writeln!(stdout, "{}", tld)?;
    }

    Ok(())
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize)]
struct CsvLine {
    rank: u32,
    domain: String,
}
