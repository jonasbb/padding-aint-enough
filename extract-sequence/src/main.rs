use failure::Error;
use misc_utils::fs;
use sequences::pcap::load_pcap_file_real;
use std::{
    net::SocketAddrV4,
    path::{Path, PathBuf},
};
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
    /// Print a list of all parsed TLS records
    #[structopt(short = "v", long = "verbose")]
    verbose: bool,
    /// Specify the IP and port of the DNS server
    ///
    /// The program tries its best to determine this automatically.
    #[structopt(short = "f", long = "filter")]
    filter: Option<SocketAddrV4>,
    /// List of PCAP files
    #[structopt(name = "PCAPS")]
    pcap_files: Vec<String>,
    // Creates a `.json.xz` file for each pcap in the same directory
    #[structopt(long = "convert-to-json")]
    convert_to_json: bool,
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
        let seq = load_pcap_file_real(Path::new(&file), cli_args.filter, true, cli_args.verbose)?;
        if cli_args.convert_to_json {
            let mut path = PathBuf::from(&file);
            path.set_extension("pcap.json.xz");
            let _ = fs::write(&path, seq.to_json()?);
        }

        /*
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

        let mut filter = cli_args.filter;
        if filter.is_none() {
            filter = (|| -> Result<Option<SocketAddrV4>, Error> {
                // Try to guess what the sever might have been
                let endpoints: HashSet<_> = records
                    .values()
                    .flat_map(std::convert::identity)
                    .map(|record| SocketAddrV4::new(record.sender, record.sender_port))
                    .collect();

                // Check different ports I use for DoT
                let candidates: Vec<_> = endpoints
                    .iter()
                    .cloned()
                    .filter(|sa| sa.port() == 853)
                    .collect();
                if candidates.len() == 1 {
                    return Ok(Some(candidates[0]));
                } else if candidates.len() > 1 {
                    bail!(make_error(candidates))
                }

                let candidates: Vec<_> = endpoints
                    .iter()
                    .cloned()
                    .filter(|sa| sa.port() == 8853)
                    .collect();
                if candidates.len() == 1 {
                    return Ok(Some(candidates[0]));
                } else if candidates.len() > 1 {
                    bail!(make_error(candidates))
                }

                bail!(make_error(endpoints))
            })()?;
        }
        // Filter was set to Some() in the snippet above
        let filter = filter.unwrap();

        // Filter to only those records containing DNS
        records.values_mut().for_each(|records| {
            // `filter_tls_records` takes the Vec by value, which is why we first need to move it out
            // of the HashMap and back it afterwards.
            let mut tmp = Vec::new();
            mem::swap(records, &mut tmp);
            tmp = filter_tls_records(tmp, (*filter.ip(), filter.port()));
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
        */
    }

    Ok(())
}
