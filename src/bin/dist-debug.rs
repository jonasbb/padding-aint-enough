#![feature(transpose_result)]

extern crate encrypted_dns;
extern crate env_logger;
extern crate failure;
extern crate glob;
#[macro_use]
extern crate log;
extern crate rayon;
#[macro_use]
extern crate structopt;

use encrypted_dns::{dnstap_to_sequence, sequences::Sequence, FailExt};
use failure::{Error, ResultExt};
use glob::glob;
use rayon::prelude::*;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(
    author = "",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct CliArgs {
    dnstap_group: Vec<String>,
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

    info!("Start loading dnstap files...");
    let mut data = load_all_dnstap_files(&cli_args.dnstap_group)?;
    data.iter_mut()
        .for_each(|group| group.sort_by(|a, b| a.id().cmp(&b.id())));
    info!("Done loading dnstap files.");

    println!("Loaded {} elements.", data.iter().flat_map(|x| x).count());

    let id_len = data
        .iter()
        .flat_map(|group| group.iter().map(|d| d.id().len()))
        .max()
        .unwrap_or(0);

    for (gid, group) in data.iter().enumerate() {
        println!("Data for Group {}: Distance within group", gid);

        for (i, d1) in group.iter().enumerate() {
            print!("{:width$}: ", d1.id(), width = id_len);
            for d2 in group.iter().take(i + 1) {
                if d1.id() == d2.id() {
                    print!("{: >4} ", "-");
                } else {
                    print!("{: >4} ", d1.distance(d2));
                }
            }
            println!();
        }
        println!();
    }

    for (gid, group) in data.iter().enumerate() {
        println!("Data for Group {}: Average distance to group", gid);

        for d in group {
            print!("{:width$}: ", d.id(), width = id_len);
            for other_group in &data {
                let average_distance = other_group
                    .iter()
                    .map(|other_d| d.distance(other_d))
                    .sum::<usize>() / other_group.len();
                print!("{: >4} ", average_distance);
            }
            println!();
        }
        println!();
    }

    Ok(())
}

fn load_all_dnstap_files<P>(patterns: &[P]) -> Result<Vec<Vec<Sequence>>, Error>
where
    P: AsRef<str>,
{
    let files: Vec<Vec<PathBuf>> = patterns
        .iter()
        .map(|pattern| -> Result<Vec<PathBuf>, Error> {
            let pattern = pattern.as_ref();
            Ok(glob(pattern)
                .context("Invalid pattern")?
                .into_iter()
                .map(|path| -> Result<_, Error> {
                    let path = path?;
                    if path.is_file() && path.to_string_lossy().contains(".dnstap") {
                        Ok(Some(path))
                    } else {
                        Ok(None)
                    }
                })
                .filter_map(|x| x.transpose())
                .collect::<Result<_, _>>()?)
        })
        .collect::<Result<_, Error>>()?;

    // Pairs of Label with Data (the Sequences)
    Ok(files
        .into_par_iter()
        .map(|file_group| {
            file_group
                .into_par_iter()
                .filter_map(|dnstap_file| {
                    debug!("Processing dnstap file '{}'", dnstap_file.display());
                    match dnstap_to_sequence(&*dnstap_file).with_context(|_| {
                        format!("Processing dnstap file '{}'", dnstap_file.display())
                    }) {
                        Ok(seq) => Some(seq),
                        Err(err) => {
                            warn!("{}", err.display_causes());
                            None
                        }
                    }
                })
                .collect()
        })
        .collect())
}
