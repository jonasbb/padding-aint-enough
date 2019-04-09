use colored::Colorize;
use extract_sequence::{build_sequence, extract_tls_records, filter_tls_records};
use failure::Error;
use itertools::Itertools;
use std::{mem, net::Ipv4Addr};
use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    author = "",
    raw(
        // Enable color output for the help
        setting = "structopt::clap::AppSettings::ColoredHelp",
        // Print help, if no arguments are given
        setting = "structopt::clap::AppSettings::ArgRequiredElseHelp"
    )
)]
struct CliArgs {
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
    /// List of PCAP files
    #[structopt(name = "PCAPS")]
    pcap_files: Vec<String>,
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

    for file in cli_args.pcap_files {
        println!("{}{}", "Processing file: ".bold(), file);

        // let file = "./tests/data/CF-constant-rate-400ms-2packets.pcap";
        let mut records = extract_tls_records(&file)?;

        // If verbose show all records in RON notation
        if cli_args.verbose {
            println!(
                "{}\n{}\n",
                "List of all TLS records in RON notation:".underline(),
                ron::ser::to_string_pretty(
                    &records,
                    ron::ser::PrettyConfig {
                        enumerate_arrays: true,
                        ..Default::default()
                    }
                )
                .unwrap()
            );
        }

        // Filter to only those records containing DNS
        records.values_mut().for_each(|records| {
            // `filter_tls_records` takes the Vec by value, which is why we first need to move it out
            // of the HashMap and back it afterwards.
            let mut tmp = Vec::new();
            mem::swap(records, &mut tmp);
            tmp = filter_tls_records(tmp, (Ipv4Addr::new(1, 0, 0, 1), 853));
            mem::swap(records, &mut tmp);
        });
        println!("{}", "TLS Records with DNS responses:".underline());
        for r in records.values().flat_map(std::convert::identity).sorted() {
            println!("{:?}", r);
        }

        // Build final Sequence
        let seq = build_sequence(records, file);
        println!("\n{}", "Final DNS Sequence:".underline());
        if let Some(seq) = seq {
            println!("{:?}", seq);
        }

        println!();
    }

    Ok(())
}
