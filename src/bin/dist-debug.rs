use anyhow::{Context as _, Error};
use glob::glob;
use log::{debug, info, warn};
use rayon::prelude::*;
use sequences::{sequence_stats, Sequence};
use std::path::PathBuf;
use structopt::{self, StructOpt};

#[derive(StructOpt, Debug)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
struct CliArgs {
    dnstap_group: Vec<String>,
}

fn main() -> Result<(), Error> {
    // generic setup
    env_logger::init();
    let cli_args = CliArgs::from_args();

    info!("Start loading dnstap files...");
    let mut data = load_all_dnstap_files(&cli_args.dnstap_group)?;
    // Remove empty groups as otherwise we get a divide by 0
    data.retain(|group| !group.is_empty());
    data.iter_mut()
        .for_each(|group| group.sort_by(|a, b| a.id().cmp(b.id())));
    info!("Done loading dnstap files.");

    println!("Loaded {} elements.", data.iter().flatten().count());

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
        print!(
            "{:width$}: ",
            format!("Data for Group {}: Average/Median distance to group", gid),
            width = id_len
        );

        for other_group in &data {
            let (_, _, avg_avg, avg_median) = sequence_stats(group, other_group);
            print!("{: >4}/{: >4} ", avg_avg, avg_median);
        }
        println!();

        for d in group {
            print!("{:width$}: ", d.id(), width = id_len);
            for other_group in &data {
                let (average_distance, median_distance, _, _) =
                    sequence_stats(&[d.clone()], other_group);
                print!("{: >4}/{: >4} ", average_distance[0], median_distance[0]);
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
            glob(pattern)
                .context("Invalid pattern")?
                .map(|path| -> Result<_, Error> {
                    let path = path?;
                    if path.is_file() && path.to_string_lossy().contains(".dnstap") {
                        Ok(Some(path))
                    } else {
                        Ok(None)
                    }
                })
                .filter_map(Result::transpose)
                .collect::<Result<_, _>>()
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
                    match Sequence::from_path(&*dnstap_file).with_context(|| {
                        format!("Processing dnstap file '{}'", dnstap_file.display())
                    }) {
                        Ok(seq) => Some(seq),
                        Err(err) => {
                            warn!("{}", err);
                            None
                        }
                    }
                })
                .collect()
        })
        .collect())
}
