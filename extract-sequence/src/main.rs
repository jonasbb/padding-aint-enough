use failure::Error;
use misc_utils::fs;
use sequences::pcap::load_pcap_file_real;
use std::{
    net::SocketAddrV4,
    path::{Path, PathBuf},
};
use structopt::StructOpt;

#[derive(Clone, Debug, StructOpt)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands,
    // Print help, if no arguments are given
    structopt::clap::AppSettings::ArgRequiredElseHelp
]))]
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
    }

    Ok(())
}
