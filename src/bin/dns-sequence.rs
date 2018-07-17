#![feature(transpose_result)]

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
extern crate rayon;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate structopt;

use csv::{ReaderBuilder, Writer as CsvWriter, WriterBuilder};
use encrypted_dns::{
    common_sequence_classifications::{
        R001, R002, R003, R004_SIZE1, R004_SIZE2, R004_SIZE3, R004_SIZE4, R004_SIZE5, R004_SIZE6,
        R004_UNKNOWN, R005, R006, R006_3RD_LVL_DOM, R007,
    },
    dnstap_to_sequence,
    sequences::{knn, split_training_test_data, Sequence, SequenceElement},
    take_largest,
};
use failure::{Error, ResultExt};
use misc_utils::fs::{file_open_read, file_open_write, WriteOptions};
use rayon::prelude::*;
use std::{
    collections::BTreeMap,
    fs,
    io::{stdout, Write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use structopt::StructOpt;

lazy_static! {
    static ref CONFUSION_DOMAINS: RwLock<Arc<BTreeMap<String, String>>> =
        RwLock::new(Arc::new(BTreeMap::new()));
}

#[derive(StructOpt, Debug)]
#[structopt(
    author = "",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct CliArgs {
    #[structopt(parse(from_os_str))]
    base_dir: PathBuf,
    #[structopt(short = "d", long = "confusion_domains", parse(from_os_str))]
    confusion_domains: Vec<PathBuf>,
    #[structopt(long = "misclassifications", parse(from_os_str))]
    misclassifications: Option<PathBuf>,
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

    // Controls how many folds there are
    let at_most_sequences_per_label = 5;
    // Controls the maximum k for knn
    let most_k = 1;

    let writer: Box<Write> = cli_args
        .misclassifications
        .map(|path| file_open_write(path, WriteOptions::new()))
        .unwrap_or_else(|| Ok(Box::new(stdout())))
        .context("Cannot open writer for misclassifications.")?;
    let mut mis_writer = WriterBuilder::new().has_headers(true).from_writer(writer);

    info!("Start loading confusion domains...");
    prepare_confusion_domains(&cli_args.confusion_domains)?;
    info!("Done loading confusion domains.");
    info!("Start loading dnstap files...");
    let data = load_all_dnstap_files(&cli_args.base_dir, at_most_sequences_per_label)?;
    info!("Done loading dnstap files.");

    let mut res = vec![(0, 0, 0); most_k];
    for fold in 0..at_most_sequences_per_label {
        info!("Testing for fold {}", fold);
        info!("Start splitting trainings and test data...");
        let (training_data, test) = split_training_test_data(&*data, fold as u8);
        let len = test.len();
        let (test_labels, test_data) = test.into_iter().fold(
            (Vec::with_capacity(len), Vec::with_capacity(len)),
            |(mut labels, mut data), elem| {
                labels.push(elem.0);
                data.push(elem.1);
                (labels, data)
            },
        );
        info!("Done splitting trainings and test data.");

        for k in (1..=most_k).step_by(2) {
            let classification = knn(&*training_data, &*test_data, k as u8);
            assert_eq!(classification.len(), test_labels.len());
            let (correct, undecided) = classification
                .into_iter()
                .zip(&test_labels)
                .zip(&test_data)
                .fold((0, 0), |(mut corr, mut und), ((class, label), sequence)| {
                    if class == *label {
                        corr += 1;
                    } else if log_misclassification(&mut mis_writer, k, &sequence, &label, &class)
                        .is_err()
                    {
                        error!(
                            "Cannot log misclassification for sequence: {}",
                            sequence.id()
                        );
                    }
                    if class.contains(&*label) {
                        und += 1;
                    }
                    (corr, und)
                });
            info!(
                "Fold: {} k: {} - {} / {} correct (In list of choices: {})",
                fold,
                k,
                correct,
                test_labels.len(),
                undecided
            );
            // update stats
            res[k - 1].0 += correct;
            res[k - 1].1 += undecided;
            res[k - 1].2 += test_labels.len();
        }
    }

    // print final results
    for (k, (correct, undecided, total)) in res.iter().enumerate() {
        println!(
            r"Results for k={k}:
Correct: {}/{total}
Multiple Options: {}/{total}
",
            correct,
            undecided,
            k = k + 1,
            total = total
        )
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
        domain: String,
        is_similar_to: String,
    };

    let mut conf_domains = BTreeMap::new();

    for path in data {
        let path = path.as_ref();
        let mut reader = ReaderBuilder::new().has_headers(false).from_reader(
            file_open_read(path)
                .with_context(|_| format!("Opening confusion file '{}' failed", path.display()))?,
        );
        for record in reader.deserialize() {
            let record: Record = record?;
            let existing =
                conf_domains.insert(record.domain.to_string(), record.is_similar_to.to_string());
            if let Some(existing) = existing {
                if existing != record.is_similar_to {
                    error!("Duplicate confusion mappings for domain '{}' but with different targets: 1) '{}' 2) '{}", record.domain, existing, record.is_similar_to);
                }
            }
        }
    }

    let mut lock = CONFUSION_DOMAINS.write().unwrap();
    *lock = Arc::new(conf_domains);

    Ok(())
}

fn make_check_confusion_domains() -> impl Fn(&str) -> String {
    let lock = CONFUSION_DOMAINS.read().unwrap();
    let conf_domains: Arc<_> = lock.clone();
    move |domain: &str| -> String {
        let mut curr = domain;
        while let Some(other) = conf_domains.get(curr) {
            curr = other;
        }
        curr.to_string()
    }
}

fn load_all_dnstap_files(
    base_dir: &Path,
    at_most_sequences_per_label: usize,
) -> Result<Vec<(String, Vec<Sequence>)>, Error> {
    let check_confusion_domains = make_check_confusion_domains();

    // Get a list of directories
    // Each directory corresponds to a label
    let directories: Vec<PathBuf> = fs::read_dir(base_dir)?
        .into_iter()
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
            }).transpose()
        })
        .collect::<Result<_, _>>()?;

    // Pairs of Label with Data (the Sequences)
    let data: Vec<(String, Vec<Sequence>)> = directories
        .into_par_iter()
        .map(|dir| {
            let label = dir
                .file_name()
                .expect("Each directory has a name")
                .to_string_lossy()
                .to_string();

            let mut filenames: Vec<PathBuf> = fs::read_dir(&dir)?
                .into_iter()
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
                    }).transpose()
                })
                .collect::<Result<_, _>>()?;
            // sort filenames for predictable results
            filenames.sort();

            let mut sequences: Vec<Sequence> = filenames
                .into_iter()
                .map(|dnstap_file| {
                    debug!("Processing dnstap file '{}'", dnstap_file.display());
                    Ok(dnstap_to_sequence(&*dnstap_file).with_context(|_| {
                        format!("Processing dnstap file '{}'", dnstap_file.display())
                    })?)
                })
                .collect::<Result<_, Error>>()?;

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
                let label = check_confusion_domains(&label);
                Ok(Some((label, sequences)))
            }
        })
        // Remove all the empty directories from the previous step
        .filter_map(|x| x.transpose())
        .collect::<Result<_, Error>>()?;

    // return all loaded data
    Ok(data)
}

fn log_misclassification<W>(
    csv_writer: &mut CsvWriter<W>,
    k: usize,
    sequence: &Sequence,
    label: &str,
    class: &str,
) -> Result<(), Error>
where
    W: Write,
{
    #[derive(Serialize)]
    struct Out<'a> {
        id: &'a str,
        k: usize,
        label: &'a str,
        class: &'a str,
        reason: Option<&'a str>,
    };

    let reason = classify_sequence(sequence);

    let out = Out {
        id: sequence.id(),
        k,
        label,
        class,
        reason,
    };

    csv_writer
        .serialize(&out)
        .map_err(|err| format_err!("{}", err))
}

fn classify_sequence(sequence: &Sequence) -> Option<&'static str> {
    // Test if sequence only contains two responses of size 1 and then 2
    let packets: Vec<_> = sequence
        .as_elements()
        .iter()
        .filter(|elem| {
            if let SequenceElement::Size(_) = elem {
                true
            } else {
                false
            }
        })
        .cloned()
        .collect();

    match &*packets {
        [] => {
            error!(
                "Empty sequence for ID {}. Should never occur",
                sequence.id()
            );
            None
        }
        [SequenceElement::Size(n)] => Some(match n {
            0 => unreachable!("Packets of size 0 may never occur."),
            1 => R004_SIZE1,
            2 => R004_SIZE2,
            3 => R004_SIZE3,
            4 => R004_SIZE4,
            5 => R004_SIZE5,
            6 => R004_SIZE6,
            _ => R004_UNKNOWN,
        }),
        [SequenceElement::Size(1), SequenceElement::Size(2)] => Some(R001),
        [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1)] => {
            Some(R002)
        }
        [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1), SequenceElement::Size(2)] => {
            Some(R003)
        }
        [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(2)] => {
            Some(R005)
        }
        [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(2)] => {
            Some(R006)
        }
        [SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(2)] => {
            Some(R006_3RD_LVL_DOM)
        }
        _ => {
            let mut is_unreachable_domain = true;
            {
                // Unreachable domains have many requests of Size 1 but never a DNSKEY
                let mut iter = sequence.as_elements().iter().fuse();
                // Sequence looks like for Size and Gap
                // S G S G S G S G S
                // we only need to loop until we find a counter proof
                while is_unreachable_domain {
                    match (iter.next(), iter.next()) {
                        // This is the end of the sequence
                        (Some(SequenceElement::Size(1)), None) => break,
                        // this is the normal, good case
                        (Some(SequenceElement::Size(1)), Some(SequenceElement::Gap(_))) => {}

                        // This can never happen with the above pattern
                        (None, None) => is_unreachable_domain = false,
                        // Sequence may not end on a Gap
                        (Some(SequenceElement::Gap(_)), None) => is_unreachable_domain = false,
                        // all other patterns, e.g., different Sizes do not match
                        _ => is_unreachable_domain = false,
                    }
                }
            }

            if is_unreachable_domain {
                Some(R007)
            } else {
                None
            }
        }
    }
}

#[test]
fn test_classify_sequence_r001() {
    use SequenceElement::{Gap, Size};

    let sequence = Sequence::new(vec![Size(1), Size(2)], "".to_string());
    assert_eq!(classify_sequence(&sequence), Some(R001));

    let sequence = Sequence::new(vec![Size(1), Gap(3), Size(2)], "".to_string());
    assert_eq!(classify_sequence(&sequence), Some(R001));

    let sequence = Sequence::new(vec![Size(1), Gap(10), Size(2)], "".to_string());
    assert_eq!(classify_sequence(&sequence), Some(R001));

    let sequence = Sequence::new(vec![Gap(9), Size(1), Size(2), Gap(12)], "".to_string());
    assert_eq!(classify_sequence(&sequence), Some(R001));

    let sequence = Sequence::new(
        vec![Gap(9), Size(1), Gap(5), Size(2), Gap(12)],
        "".to_string(),
    );
    assert_eq!(classify_sequence(&sequence), Some(R001));
}

#[test]
fn test_classify_sequence_r002() {
    use SequenceElement::{Gap, Size};

    let sequence = Sequence::new(vec![Size(1), Size(2), Size(1)], "".to_string());
    assert_eq!(classify_sequence(&sequence), Some(R002));

    let sequence = Sequence::new(vec![Size(1), Gap(3), Size(2), Size(1)], "".to_string());
    assert_eq!(classify_sequence(&sequence), Some(R002));

    let sequence = Sequence::new(
        vec![Size(1), Gap(5), Size(2), Gap(10), Size(1)],
        "".to_string(),
    );
    assert_eq!(classify_sequence(&sequence), Some(R002));

    let sequence = Sequence::new(
        vec![Gap(2), Size(1), Gap(15), Size(2), Gap(2), Size(1), Gap(3)],
        "".to_string(),
    );
    assert_eq!(classify_sequence(&sequence), Some(R002));

    // These must fail

    let sequence = Sequence::new(vec![Size(1), Size(1), Size(1)], "".to_string());
    assert_ne!(classify_sequence(&sequence), Some(R002));

    let sequence = Sequence::new(vec![Size(1), Size(1), Size(2)], "".to_string());
    assert_ne!(classify_sequence(&sequence), Some(R002));

    let sequence = Sequence::new(vec![Size(2), Size(1), Size(1)], "".to_string());
    assert_ne!(classify_sequence(&sequence), Some(R002));
}

#[test]
fn test_classify_sequence_r007() {
    use SequenceElement::{Gap, Size};

    let sequence = Sequence::new(vec![Size(1), Gap(3), Size(1)], "".to_string());
    assert_eq!(classify_sequence(&sequence), Some(R007));

    let sequence = Sequence::new(
        vec![Size(1), Gap(3), Size(1), Gap(3), Size(1), Gap(3), Size(1)],
        "".to_string(),
    );
    assert_eq!(classify_sequence(&sequence), Some(R007));

    // These must fail

    let sequence = Sequence::new(
        vec![
            Size(1),
            Gap(2),
            Size(2),
            Gap(9),
            Size(1),
            Gap(2),
            Size(1),
            Gap(9),
            Size(1),
            Gap(2),
            Size(1),
            Gap(1),
            Size(1),
            Gap(2),
            Size(1),
            Gap(8),
            Size(1),
            Gap(2),
            Size(1),
            Gap(2),
            Size(2),
            Gap(2),
            Size(2),
        ],
        "".to_string(),
    );

    assert_ne!(classify_sequence(&sequence), Some(R007));
}
