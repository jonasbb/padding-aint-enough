#![feature(transpose_result)]
#![cfg_attr(feature = "cargo-clippy", allow(renamed_and_removed_lints))]

extern crate csv;
extern crate encrypted_dns;
extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate misc_utils;
#[cfg(feature = "plot")]
extern crate plot;
extern crate prettytable;
extern crate rayon;
extern crate sequences;
#[macro_use]
extern crate serde;
#[cfg(not(feature = "plot"))]
extern crate serde_pickle;
extern crate serde_with;
extern crate string_cache;
extern crate structopt;

mod stats;

use csv::{ReaderBuilder, Writer as CsvWriter, WriterBuilder};
use encrypted_dns::{dnstap_to_sequence, take_largest, FailExt};
use failure::{Error, ResultExt};
use misc_utils::{
    fs::{file_open_read, file_open_write, WriteOptions},
    Max, Min,
};
use rayon::prelude::*;
use sequences::{
    common_sequence_classifications::*, knn, replace_loading_failed, LabelledSequences, Sequence,
};
use serde::Serialize;
use stats::StatsCollector;
use std::{
    collections::HashMap,
    fmt::{self, Display},
    fs::{self, OpenOptions},
    io::{stdout, BufReader, Write},
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
    #[structopt(short = "k", default_value = "3")]
    k: usize,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
enum ClassificationResult {
    Correct,
    Undetermined,
    Wrong,
}

impl Display for ClassificationResult {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ClassificationResult::Correct => write!(f, "Correct"),
            ClassificationResult::Undetermined => write!(f, "Undetermined"),
            ClassificationResult::Wrong => write!(f, "Wrong"),
        }
    }
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

    // Controls how many folds there are
    let at_most_sequences_per_label = 10;

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
        .unwrap_or_else(|| Ok(Box::new(stdout())))
        .context("Cannot open writer for misclassifications.")?;
    let mut mis_writer = WriterBuilder::new().has_headers(true).from_writer(writer);

    let (res1, res2) = rayon::join(
        || {
            info!("Start loading confusion domains...");
            let res = prepare_confusion_domains(&cli_args.confusion_domains);
            info!("Done loading confusion domains.");
            res
        },
        || {
            if let Some(ref path) = &cli_args.loading_failed {
                info!("Start loading of failed domains...");
                let res = prepare_failed_domains(path);
                info!("Done loading of failed domains.");
                res
            } else {
                Ok(())
            }
        },
    );
    res1?;
    res2?;

    info!("Start loading dnstap files...");
    let data = load_all_dnstap_files(&cli_args.base_dir, at_most_sequences_per_label)?;
    info!("Done loading dnstap files.");
    {
        // delete non-permanent memory
        let mut lock = CONFUSION_DOMAINS
            .write()
            .expect("CONFUSION_DOMAINS should still be accessible");
        *lock = Arc::default();
    }

    // Collect the stats during the execution and print them at the end
    let mut stats = StatsCollector::new();

    for fold in 0..at_most_sequences_per_label {
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
            info!("Start classification for k={}...", k);
            let classification = knn::knn(&*training_data, &*test_data, k as u8);
            assert_eq!(classification.len(), test_labels.len());
            info!("Done classification for k={}, start evaluation...", k);
            classification
                .into_iter()
                .zip(&test_labels)
                .zip(&test_data)
                .for_each(
                    |(
                        ((ref class, min_dist, max_dist), (true_domain, mapped_domain)),
                        sequence,
                    )| {
                        let class_res = if **class == *mapped_domain {
                            ClassificationResult::Correct
                        } else if class.contains(&**mapped_domain) {
                            ClassificationResult::Undetermined
                        } else {
                            ClassificationResult::Wrong
                        };
                        let known_problems = sequence.classify().map(Atom::from);
                        stats.update(
                            k as u8,
                            true_domain.clone(),
                            mapped_domain.clone(),
                            class_res,
                            known_problems.clone(),
                        );

                        if class_res != ClassificationResult::Correct && log_misclassification(
                            &mut mis_writer,
                            k,
                            &sequence,
                            &mapped_domain,
                            &class,
                            min_dist,
                            max_dist,
                            known_problems.as_ref().map(|x| &**x),
                        )
                        .is_err()
                        {
                            error!(
                                "Cannot log misclassification for sequence: {}",
                                sequence.id()
                            );
                        }
                    },
                );
            info!("Done evaluation for k={}", k);
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

fn load_all_dnstap_files(
    base_dir: &Path,
    at_most_sequences_per_label: usize,
) -> Result<Vec<LabelledSequences>, Error> {
    let check_confusion_domains = make_check_confusion_domains();

    // Get a list of directories
    // Each directory corresponds to a label
    let directories: Vec<PathBuf> = fs::read_dir(base_dir)?
        .flat_map(|x| {
            x.and_then(|entry| {
                // Result<Option<PathBuf>>
                entry.file_type().map(|ft| {
                    if ft.is_dir() {
                        Some(entry.path())
                    } else {
                        None
                    }
                })
            })
            .transpose()
        })
        .collect::<Result<_, _>>()?;

    // Pairs of Label with Data (the Sequences)
    let data: Vec<LabelledSequences> = directories
        .into_par_iter()
        .map(|dir| {
            let label = dir
                .file_name()
                .expect("Each directory has a name")
                .to_string_lossy()
                .into();

            let mut filenames: Vec<PathBuf> = fs::read_dir(&dir)?
                .flat_map(|x| {
                    x.and_then(|entry| {
                        // Result<Option<PathBuf>>
                        entry.file_type().map(|ft| {
                            if ft.is_file()
                                && entry.file_name().to_string_lossy().contains(".dnstap")
                            {
                                Some(entry.path())
                            } else {
                                None
                            }
                        })
                    })
                    .transpose()
                })
                .collect::<Result<_, _>>()?;
            // sort filenames for predictable results
            filenames.sort();

            let mut sequences: Vec<Sequence> = filenames
                .into_iter()
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
                .collect();

            // TODO this is sooo ugly
            // Only retain 5 of the possibilities which have the highest number of diversity
            // Sequences are sorted by complexity
            sequences = take_largest(sequences, at_most_sequences_per_label);

            // Some directories do not contain data, e.g., because the site didn't exists
            // Skip all directories with 0 results
            if sequences.is_empty() {
                warn!("Directory contains no data: {}", dir.display());
                Ok(None)
            } else {
                let mapped_label = check_confusion_domains(&label);
                Ok(Some(LabelledSequences {
                    true_domain: label,
                    mapped_domain: mapped_label,
                    sequences,
                }))
            }
        })
        // Remove all the empty directories from the previous step
        .filter_map(|x| x.transpose())
        .collect::<Result<_, Error>>()?;

    // return all loaded data
    Ok(data)
}

#[cfg_attr(feature = "cargo-clippy", allow(too_many_arguments))]
fn log_misclassification<W>(
    csv_writer: &mut CsvWriter<W>,
    k: usize,
    sequence: &Sequence,
    label: &str,
    class: &str,
    min_dist: Min<usize>,
    max_dist: Max<usize>,
    reason: Option<&str>,
) -> Result<(), Error>
where
    W: Write,
{
    #[derive(Serialize)]
    struct Out<'a> {
        id: &'a str,
        k: usize,
        label: &'a str,
        #[serde(with = "serde_with::rust::display_fromstr")]
        min_dist: Min<usize>,
        #[serde(with = "serde_with::rust::display_fromstr")]
        max_dist: Max<usize>,
        class: &'a str,
        reason: Option<&'a str>,
    };

    let out = Out {
        id: sequence.id(),
        k,
        label,
        min_dist,
        max_dist,
        class,
        reason,
    };

    csv_writer
        .serialize(&out)
        .map_err(|err| format_err!("{}", err))
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
