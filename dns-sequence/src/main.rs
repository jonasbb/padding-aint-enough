mod jsonl;
mod stats;

use crate::{jsonl::JsonlFormatter, stats::StatsCollector};
use anyhow::{anyhow, Context as _, Error};
use dns_sequence::{load_all_files, prepare_confusion_domains, SimulateOption};
use log::{error, info};
use misc_utils::fs::file_write;
use sequences::{
    knn::{self, ClassificationResult, LabelledSequences},
    Sequence,
};
use serde::Serialize;
use serde_json::Serializer as JsonSerializer;
use std::{ffi::OsString, fs::OpenOptions, io::Write, path::PathBuf};
use string_cache::DefaultAtom as Atom;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
struct CliArgs {
    /// Subcommand to execute: Command is more specific set
    #[structopt(subcommand)]
    cmd: Option<SubCommand>,
    /// Base directory containing per domain a folder which contains the dnstap files
    #[structopt(parse(from_os_str))]
    base_dir: PathBuf,
    /// Some domains are known similar. Specify a CSV file renaming the "original" domain to some other identifier.
    /// This option can be applied multiple times. It is not permitted to have conflicting entries to the same domain.
    #[structopt(short = "d", long = "confusion_domains", parse(from_os_str))]
    confusion_domains: Vec<PathBuf>,
    /// Path to dump a CSV file containing all the wrongly classified data
    #[structopt(long = "misclassifications", parse(from_os_str))]
    misclassifications: Option<PathBuf>,
    /// Path for the resulting CSV-statistics file and plot/json-files
    #[structopt(long = "statistics", parse(from_os_str))]
    statistics: Option<PathBuf>,
    /// The largest `k` to be used for knn. Only odd numbers are tested.
    #[structopt(short = "k", default_value = "1")]
    k: usize,
    /// Only test a single k. Overwrites `-k` option.
    #[structopt(long = "exact-k", value_name = "k")]
    exact_k: Option<usize>,
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
}

#[derive(StructOpt, Debug, Clone)]
enum SubCommand {
    /// Perform crossvalidation within the trainings data
    #[structopt(
        name = "crossvalidate",
        global_settings(&[
            structopt::clap::AppSettings::ColoredHelp,
            structopt::clap::AppSettings::VersionlessSubcommands
        ])
    )]
    Crossvalidate {
        #[structopt(long = "dist-thres")]
        distance_threshold: Option<f32>,
        #[structopt(long = "use-cr-mode")]
        use_cr_mode: bool,
        #[structopt(
            long = "simulate",
            default_value = "Normal",
            possible_values = &SimulateOption::variants(),
            case_insensitive = true
        )]
        simulate: SimulateOption,
    },
    /// Perform classification of the test data against the trainings data
    #[structopt(
        name = "classify",
        global_settings(&[
            structopt::clap::AppSettings::ColoredHelp,
            structopt::clap::AppSettings::VersionlessSubcommands
        ])
    )]
    Classify {
        /// Data to be classified. Directory containing a folder per domain, like `base_dir`.
        #[structopt(long = "test-data", parse(from_os_str))]
        test_data: PathBuf,
        #[structopt(long = "dist-thres")]
        distance_threshold: Option<f32>,
        #[structopt(long = "use-cr-mode")]
        use_cr_mode: bool,
        #[structopt(
            long = "simulate",
            default_value = "Normal",
                possible_values = &SimulateOption::variants(),
                case_insensitive = true
        )]
        simulate: SimulateOption,
    },
}

fn main() -> Result<(), Error> {
    // generic setup
    env_logger::init();
    let mut cli_args = CliArgs::from_args();

    let writer: Box<dyn Write> = cli_args
        .misclassifications
        .as_ref()
        .map(|path| file_write(path).create(true).truncate())
        .unwrap_or_else(|| {
            Ok(Box::new(
                OpenOptions::new().write(true).open("/dev/null").unwrap(),
            ))
        })
        .context("Cannot open writer for misclassifications.")?;
    let mut mis_writer = JsonSerializer::with_formatter(writer, JsonlFormatter::new());

    info!("Start loading confusion domains...");
    prepare_confusion_domains(&cli_args.confusion_domains)?;
    info!("Done loading confusion domains.");

    info!("Start loading dnstap files...");
    let simulate = match &cli_args.cmd {
        None => SimulateOption::Normal,
        Some(SubCommand::Crossvalidate { simulate, .. }) => *simulate,
        Some(SubCommand::Classify { simulate, .. }) => *simulate,
    };
    let training_data = load_all_files(&cli_args.base_dir, &cli_args.file_extension, simulate)?;
    info!(
        "Done loading dnstap files. Found {} domains.",
        training_data.len()
    );

    // Collect the stats during the execution and print them at the end
    let mut stats = StatsCollector::new();

    match cli_args.cmd {
        None => {
            // In case of `None` overwrite it to make sure the individual functions never have to handle a `None`.
            cli_args.cmd = Some(SubCommand::Crossvalidate {
                distance_threshold: None,
                use_cr_mode: false,
                simulate: SimulateOption::Normal,
            });
            run_crossvalidation(&cli_args, training_data, &mut stats, &mut mis_writer)
        }
        Some(SubCommand::Crossvalidate { .. }) => {
            run_crossvalidation(&cli_args, training_data, &mut stats, &mut mis_writer)
        }
        Some(SubCommand::Classify { .. }) => {
            run_classify(&cli_args, training_data, &mut stats, &mut mis_writer)?;
        }
    }

    // TODO print final stats
    println!("{}", stats);
    if let Some(path) = &cli_args.statistics {
        stats.dump_stats_to_file(path)?;
        // the file extension will be overwritten later
        stats.plot(&path.with_extension("placeholder"))?;
    }

    Ok(())
}

fn run_crossvalidation(
    cli_args: &CliArgs,
    data: Vec<LabelledSequences>,
    stats: &mut StatsCollector,
    mis_writer: &mut JsonSerializer<impl Write, impl serde_json::ser::Formatter>,
) {
    if let Some(SubCommand::Crossvalidate {
        distance_threshold,
        use_cr_mode,
        ..
    }) = cli_args.cmd.clone()
    {
        for fold in 0..10 {
            info!("Testing for fold {}", fold);
            info!("Start splitting trainings and test data...");
            let (training_data, test) = knn::split_training_test_data(&*data, fold as u8);
            let len = test.len();
            let (test_labels, test_data) = test.into_iter().fold(
                (Vec::with_capacity(len), Vec::with_capacity(len)),
                |(mut test_labels, mut data), elem| {
                    test_labels.push((elem.true_domain, elem.mapped_domain));
                    data.push(elem.sequence);
                    (test_labels, data)
                },
            );
            info!("Done splitting trainings and test data.");

            let ks: Vec<usize>;
            if let Some(exact_k) = cli_args.exact_k {
                ks = vec![exact_k];
            } else {
                ks = (1..=(cli_args.k)).step_by(2).collect();
            }

            for k in ks {
                classify_and_evaluate(
                    k,
                    distance_threshold,
                    use_cr_mode,
                    &*training_data,
                    &*test_data,
                    &*test_labels,
                    stats,
                    mis_writer,
                );
            }
        }
    } else {
        unreachable!("The value of `SubCommand` must be a `Crossvalidate`.")
    }
}

fn run_classify(
    cli_args: &CliArgs,
    data: Vec<LabelledSequences>,
    stats: &mut StatsCollector,
    mis_writer: &mut JsonSerializer<impl Write, impl serde_json::ser::Formatter>,
) -> Result<(), Error> {
    if let Some(SubCommand::Classify {
        test_data,
        distance_threshold,
        use_cr_mode,
        simulate,
    }) = cli_args.cmd.clone()
    {
        info!("Start loading test data dnstap files...");
        let test_data = load_all_files(&test_data, &cli_args.file_extension, simulate)?;
        info!(
            "Done loading test data dnstap files. Found {} domains.",
            test_data.len()
        );

        // Separate labels from sequences
        let len = test_data.len();
        let (test_labels, test_sequences) = test_data.into_iter().fold(
            (Vec::with_capacity(len), Vec::with_capacity(len)),
            |(mut test_labels, mut test_sequences), elem| {
                for seq in elem.sequences {
                    test_labels.push((elem.true_domain.clone(), elem.mapped_domain.clone()));
                    test_sequences.push(seq);
                }
                (test_labels, test_sequences)
            },
        );

        let ks: Vec<usize>;
        if let Some(exact_k) = cli_args.exact_k {
            ks = vec![exact_k];
        } else {
            ks = (1..=(cli_args.k)).step_by(2).collect();
        }

        for k in ks {
            classify_and_evaluate(
                k,
                distance_threshold,
                use_cr_mode,
                &*data,
                &*test_sequences,
                &*test_labels,
                stats,
                mis_writer,
            )
        }

        Ok(())
    } else {
        unreachable!("The value of `SubCommand` must be a `Classify`.")
    }
}

/// This function takes trainings and test data and performs classification with them.
///
/// Results of the classification process are logged to the `stats/StatsCollector` and
/// `mis_writer`/`JsonSerializer`.
///
/// The parameters `k` and `distance_threshold` configure the behaviour of the function. `k` refers
/// to the k in k-NN, while the `distance_threshold`, if not `None`, allows to specify an additional
/// threshold, in which case no classification should happen. This toggles the two different k-NN
/// variants from the paper.
#[allow(clippy::too_many_arguments)]
fn classify_and_evaluate(
    // The `k` for k-NN
    k: usize,
    distance_threshold: Option<f32>,
    use_cr_mode: bool,
    training_data: &[LabelledSequences],
    test_data: &[Sequence],
    test_labels: &[(Atom, Atom)],
    stats: &mut StatsCollector,
    mis_writer: &mut JsonSerializer<impl Write, impl serde_json::ser::Formatter>,
) {
    info!("Start classification for k={}...", k);
    let classification;
    if let Some(distance_threshold) = distance_threshold {
        classification = knn::knn_with_threshold(
            &*training_data,
            &*test_data,
            k as u8,
            f64::from(distance_threshold),
            use_cr_mode,
        )
    } else {
        classification = knn::knn(&*training_data, &*test_data, k as u8, use_cr_mode)
    }
    assert_eq!(classification.len(), test_labels.len());
    info!("Done classification for k={}, start evaluation...", k);
    classification
        .iter()
        .zip(test_labels)
        .zip(test_data)
        .for_each(|((class_result, (true_domain, mapped_domain)), sequence)| {
            let result_quality = class_result.determine_quality(&*mapped_domain);
            let known_problems = sequence.classify().map(Atom::from);

            stats.update(
                k as u8,
                true_domain.clone(),
                mapped_domain.clone(),
                result_quality,
                known_problems.clone(),
            );

            if let Err(err) = log_misclassification(
                mis_writer,
                k,
                &sequence,
                &mapped_domain,
                &class_result,
                known_problems.as_ref().map(|x| &**x),
            ) {
                error!(
                    "Cannot log misclassification for sequence `{}`: {}",
                    sequence.id(),
                    err,
                );
            }
        });
    info!("Done evaluation for k={}", k);
}

#[allow(clippy::too_many_arguments)]
fn log_misclassification<W, FMT>(
    writer: &mut JsonSerializer<W, FMT>,
    k: usize,
    sequence: &Sequence,
    label: &str,
    class_result: &ClassificationResult,
    reason: Option<&str>,
) -> Result<(), Error>
where
    W: Write,
    FMT: serde_json::ser::Formatter,
{
    #[derive(Serialize)]
    struct Out<'a> {
        id: &'a str,
        k: usize,
        label: &'a str,
        class_result: &'a ClassificationResult,
        reason: Option<&'a str>,
    };

    let out = Out {
        id: sequence.id(),
        k,
        label,
        class_result,
        reason,
    };

    out.serialize(writer).map_err(|err| anyhow!("{}", err))
}

/// Calculate the reverse cumulitive sum
///
/// The input `counts` is a slice which specifies how often the value `i` occured, where `i` is
/// the vector index. The result is a new vector with the reverse cumulitive sum like:misc_utils
///
/// `sum(0..n), sum(1..n), sum(2..n), ..., sum(n-1..n)`
///
/// where `n` is the number of elements in the input slice
#[must_use]
fn reverse_cum_sum(counts: &[usize]) -> Vec<usize> {
    let mut accu = 0;
    // convert the counts per "correctness level" into accumulative counts
    let mut tmp: Vec<_> = counts
        .iter()
        // go from 10 to 0
        .rev()
        // sum them like
        // 10; 10 + 9; 10 + 9 + 8; etc.
        .map(|&count| {
            accu += count;
            accu
        })
        .collect();
    // revert them again to go from 0 to 10
    tmp.reverse();
    tmp
}

#[test]
fn test_reverse_cum_sum() {
    assert_eq!(Vec::<usize>::new(), reverse_cum_sum(&[]));
    assert_eq!(vec![1], reverse_cum_sum(&[1]));
    assert_eq!(vec![1, 1], reverse_cum_sum(&[0, 1]));
    assert_eq!(vec![1, 0], reverse_cum_sum(&[1, 0]));
    assert_eq!(vec![1, 1, 0], reverse_cum_sum(&[0, 1, 0]));
    assert_eq!(
        vec![10, 9, 8, 7, 6, 5, 4, 3, 2, 1],
        reverse_cum_sum(&[1, 1, 1, 1, 1, 1, 1, 1, 1, 1])
    );
}
