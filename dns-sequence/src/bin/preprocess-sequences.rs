use dns_sequence::{load_all_files, prepare_confusion_domains, SimulateOption};
use failure::Error;
use log::info;
use std::{ffi::OsString, io::Write, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
struct CliArgs {
    /// Base directory containing per domain a folder which contains the dnstap files
    #[structopt(parse(from_os_str))]
    base_dir: PathBuf,
    /// Some domains are known similar. Specify a CSV file renaming the "original" domain to some other identifier.
    /// This option can be applied multiple times. It is not permitted to have conflicting entries to the same domain.
    #[structopt(short = "d", long = "confusion_domains", parse(from_os_str))]
    confusion_domains: Vec<PathBuf>,
    /// File extension which must be available in the file to be recognized as a Sequence file
    ///
    /// This can be `pcap`, `dnstap`, `json`
    #[structopt(
        long = "extension",
        value_name = "ext",
        default_value = "dnstap",
        parse(from_os_str)
    )]
    file_extension: OsString,
    #[structopt(
        long = "simulate",
        default_value = "Normal",
        possible_values = &SimulateOption::variants(),
        case_insensitive = true
    )]
    simulate: SimulateOption,
    #[structopt(short = "o", long = "out", value_name = "FILE", parse(from_os_str))]
    outfile: PathBuf,
}

fn main() {
    use std::io;

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

    info!("Start loading confusion domains...");
    prepare_confusion_domains(&cli_args.confusion_domains)?;
    info!("Done loading confusion domains.");

    info!("Start loading dnstap files...");
    let training_data = load_all_files(
        &cli_args.base_dir,
        &cli_args.file_extension,
        cli_args.simulate,
    )?;
    info!(
        "Done loading dnstap files. Found {} domains.",
        training_data.len()
    );

    let writer = misc_utils::fs::file_open_write(cli_args.outfile, Default::default())?;
    serde_json::to_writer(writer, &training_data)?;

    Ok(())
}
