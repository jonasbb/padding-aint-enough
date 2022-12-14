use anyhow::Error;
use misc_utils::fs;
use sequences::{pcap::build_sequence, LoadSequenceConfig};
use std::{
    net::SocketAddrV4,
    path::{Path, PathBuf},
};
use structopt::{clap::arg_enum, StructOpt};

arg_enum! {
    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub enum GapMode {
        Log2,
        Ident
    }
}

impl From<GapMode> for sequences::GapMode {
    fn from(gm: GapMode) -> Self {
        match gm {
            GapMode::Log2 => sequences::GapMode::Log2,
            GapMode::Ident => sequences::GapMode::Ident,
        }
    }
}

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
    /// Creates a `.json.xz` file for each pcap in the same directory
    #[structopt(long = "convert-to-json")]
    convert_to_json: bool,
    /// Method to convert the time between messages into a gap value
    #[structopt(long = "gap-mode", possible_values = &GapMode::variants(), case_insensitive = true)]
    gap_mode: Option<GapMode>,
}

fn main() -> Result<(), Error> {
    // generic setup
    env_logger::init();
    let cli_args = CliArgs::from_args();
    let mut config = LoadSequenceConfig::default();
    if let Some(gap_mode) = cli_args.gap_mode {
        config.gap_mode = gap_mode.into();
    }

    for file in cli_args.pcap_files {
        let seq = build_sequence(Path::new(&file), cli_args.filter, cli_args.verbose, config)?;
        if cli_args.convert_to_json {
            let mut path = PathBuf::from(&file);
            path.set_extension("json.xz");
            let _ = fs::write(&path, seq.to_json()?);
        }
    }

    Ok(())
}
