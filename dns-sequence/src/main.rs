mod jsonl;
mod stats;

use crate::{jsonl::JsonlFormatter, stats::StatsCollector};
use csv::ReaderBuilder;
use failure::{bail, format_err, Error, ResultExt};
use lazy_static::lazy_static;
use log::{error, info};
use misc_utils::fs::{file_open_read, file_open_write, WriteOptions};
use sequences::{
    common_sequence_classifications::*,
    knn::{self, ClassificationResult, ClassificationResultQuality},
    replace_loading_failed, LabelledSequences, Sequence,
};
use serde::{Deserialize, Serialize};
use serde_json::Serializer as JsonSerializer;
use std::{
    collections::HashMap,
    fs::OpenOptions,
    io::{BufReader, Write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use string_cache::DefaultAtom as Atom;
use structopt::StructOpt;

lazy_static! {
    static ref CONFUSION_DOMAINS: RwLock<Arc<HashMap<Atom, Atom>>> = RwLock::default();
}

#[derive(StructOpt, Debug)]
#[structopt(
    author = "",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
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
    /// List of file names which did not load properly.
    /// Also see the `website-failed` tool.
    #[structopt(long = "loading_failed", parse(from_os_str))]
    loading_failed: Option<PathBuf>,
    /// Path to dump a CSV file containing all the wrongly classified data
    #[structopt(long = "misclassifications", parse(from_os_str))]
    misclassifications: Option<PathBuf>,
    /// Path for the resulting CSV-statistics file and plot/pickle-files
    #[structopt(long = "statistics", parse(from_os_str))]
    statistics: Option<PathBuf>,
    /// The largest `k` to be used for knn. Only odd numbers are tested.
    #[structopt(short = "k", default_value = "1")]
    k: usize,
}

#[derive(StructOpt, Debug, Clone)]
enum SubCommand {
    /// Perform crossvalidation within the trainings data
    #[structopt(name = "crossvalidate")]
    Crossvalidate,
    /// Perform classification of the test data against the trainings data
    #[structopt(name = "classify")]
    Classify {
        /// Data to be classified. Directory containing a folder per domain, like `base_dir`.
        #[structopt(long = "test-data", parse(from_os_str))]
        test_data: PathBuf,
        /// If specified, the data is treated as open-world data with the corresponding distance function
        #[structopt(short = "O", long = "open-world")]
        open_world: bool,
        #[structopt(long = "dist-thres")]
        distance_threshold: Option<f32>,
    },
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
    let mut cli_args = CliArgs::from_args();

    let writer: Box<Write> = cli_args
        .misclassifications
        .as_ref()
        .map(|path| {
            file_open_write(
                path,
                WriteOptions::new()
                    .set_open_options(OpenOptions::new().create(true).truncate(true)),
            )
        })
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

    if let Some(ref path) = &cli_args.loading_failed {
        info!("Start loading of failed domains...");
        prepare_failed_domains(path)?;
        info!("Done loading of failed domains.");
    }

    info!("Start loading dnstap files...");
    let training_data = load_all_dnstap_files(&cli_args.base_dir)?;
    info!(
        "Done loading dnstap files. Found {} domains.",
        training_data.len()
    );
    {
        // delete non-permanent memory
        let mut lock = CONFUSION_DOMAINS
            .write()
            .expect("CONFUSION_DOMAINS should still be accessible");
        *lock = Arc::default();
    }

    // Collect the stats during the execution and print them at the end
    let mut stats = StatsCollector::new();

    match cli_args.cmd {
        None | Some(SubCommand::Crossvalidate) => {
            // In case of `None` overwrite it to make sure the individual functions never have to handle a `None`.
            cli_args.cmd = Some(SubCommand::Crossvalidate);
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

        for k in (1..=(cli_args.k)).step_by(2) {
            classify_and_evaluate(
                k,
                None,
                &*training_data,
                &*test_data,
                &*test_labels,
                stats,
                mis_writer,
            );
        }
    }
}

fn run_classify(
    cli_args: &CliArgs,
    data: Vec<LabelledSequences>,
    stats: &mut StatsCollector,
    mis_writer: &mut JsonSerializer<impl Write, impl serde_json::ser::Formatter>,
) -> Result<(), Error> {
    if let Some(SubCommand::Classify {
        open_world,
        test_data,
        distance_threshold,
    }) = cli_args.cmd.clone()
    {
        if open_world {
            bail!("Open world not yet implemented");
        }

        info!("Start loading test data dnstap files...");
        let test_data = load_all_dnstap_files(&test_data)?;
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

        for k in (1..=cli_args.k).step_by(2) {
            classify_and_evaluate(
                k,
                distance_threshold,
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
fn classify_and_evaluate(
    // The `k` for k-NN
    k: usize,
    distance_threshold: Option<f32>,
    training_data: &[LabelledSequences],
    test_data: &[Sequence],
    test_labels: &[(Atom, Atom)],
    stats: &mut StatsCollector,
    mis_writer: &mut JsonSerializer<impl Write, impl serde_json::ser::Formatter>,
) {
    info!("Start classification for k={}...", k);
    let classification;
    if let Some(distance_threshold) = distance_threshold {
        classification =
            knn::knn_with_threshold(&*training_data, &*test_data, k as u8, distance_threshold)
    } else {
        classification = knn::knn(&*training_data, &*test_data, k as u8)
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

            if result_quality != ClassificationResultQuality::Exact
                && log_misclassification(
                    mis_writer,
                    k,
                    &sequence,
                    &mapped_domain,
                    &class_result,
                    known_problems.as_ref().map(|x| &**x),
                )
                .is_err()
            {
                error!(
                    "Cannot log misclassification for sequence: {}",
                    sequence.id()
                );
            }
        });
    info!("Done evaluation for k={}", k);
}

fn prepare_confusion_domains<D, P>(data: D) -> Result<(), Error>
where
    D: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    #[derive(Debug, Deserialize)]
    struct Record {
        domain: Atom,
        is_similar_to: Atom,
    };

    let mut conf_domains = HashMap::default();

    for path in data {
        let path = path.as_ref();
        let mut reader = ReaderBuilder::new().has_headers(false).from_reader(
            file_open_read(path)
                .with_context(|_| format!("Opening confusion file '{}' failed", path.display()))?,
        );
        for record in reader.deserialize() {
            let record: Record = record?;
            // skip comment lines
            if record.domain.starts_with('#') {
                continue;
            }
            let existing = conf_domains.insert(record.domain.clone(), record.is_similar_to.clone());
            if let Some(existing) = existing {
                if existing != record.is_similar_to {
                    error!("Duplicate confusion mappings for domain '{}' but with different targets: 1) '{}' 2) '{}'", record.domain, existing, record.is_similar_to);
                }
            }
        }
    }

    let mut lock = CONFUSION_DOMAINS.write().unwrap();
    *lock = Arc::new(conf_domains);

    Ok(())
}

fn prepare_failed_domains(path: impl AsRef<Path>) -> Result<(), Error> {
    #[derive(Debug, Deserialize)]
    struct Record {
        file: PathBuf,
        reason: String,
    }

    let rdr = BufReader::new(file_open_read(path.as_ref()).with_context(|_| {
        format!(
            "Opening failed domains file '{}' failed",
            path.as_ref().display()
        )
    })?);
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(rdr);

    let mut failed_domains = HashMap::default();

    for record in rdr.deserialize() {
        let record: Record = record.context("Failed to read from failed domains file")?;
        let file_name: Atom = (&record.file)
            .file_name()
            .map(|file_name| Atom::from(file_name.to_string_lossy().replace(".json", ".dnstap")))
            .ok_or_else(|| {
                format_err!(
                    "This line does not specify a path with a file name '{}'",
                    record.file.display()
                )
            })?;

        let reason = if record.reason == R008 {
            R008
        } else if record.reason == R009 {
            R009
        } else {
            bail!("Found unknown reason: {}", record.reason)
        };
        failed_domains.insert(file_name, reason);
    }

    replace_loading_failed(failed_domains);
    Ok(())
}

fn make_check_confusion_domains() -> impl Fn(&Atom) -> Atom {
    let lock = CONFUSION_DOMAINS.read().unwrap();
    let conf_domains: Arc<_> = lock.clone();
    move |domain: &Atom| -> Atom {
        let mut curr = domain;
        while let Some(other) = conf_domains.get(curr) {
            curr = other;
        }
        curr.into()
    }
}

fn load_all_dnstap_files(base_dir: &Path) -> Result<Vec<LabelledSequences>, Error> {
    let check_confusion_domains = make_check_confusion_domains();

    Ok(sequences::load_all_dnstap_files_from_dir(base_dir)
        .with_context(|_| {
            format!(
                "Could not load some sequence files from dir: {}",
                base_dir.display()
            )
        })?
        .into_iter()
        .map(|(label, seqs): (String, Vec<Sequence>)| {
            let label = Atom::from(label);
            let mapped_label = check_confusion_domains(&label);

            LabelledSequences {
                true_domain: label,
                mapped_domain: mapped_label,
                sequences: seqs,
            }
        })
        .collect())
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

    out.serialize(writer).map_err(|err| format_err!("{}", err))
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
