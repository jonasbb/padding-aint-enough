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
extern crate minmax;
extern crate misc_utils;
extern crate rayon;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_with;
extern crate string_cache;
extern crate structopt;

use csv::{ReaderBuilder, Writer as CsvWriter, WriterBuilder};
use encrypted_dns::{
    common_sequence_classifications::{
        R001, R002, R003, R004_SIZE1, R004_SIZE2, R004_SIZE3, R004_SIZE4, R004_SIZE5, R004_SIZE6,
        R004_UNKNOWN, R005, R006, R006_3RD_LVL_DOM, R007,
    },
    dnstap_to_sequence,
    sequences::{knn, split_training_test_data, LabelledSequences, Sequence, SequenceElement},
    take_largest, FailExt,
};
use failure::{Error, ResultExt};
use minmax::{Max, Min};
use misc_utils::fs::{file_open_read, file_open_write, WriteOptions};
use rayon::prelude::*;
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
    fmt::{self, Display},
    fs::{self, OpenOptions},
    hash::Hash,
    io::{stdout, Write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use string_cache::DefaultAtom as Atom;
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
    #[structopt(long = "statistics", parse(from_os_str))]
    statistics: Option<PathBuf>,
}

#[derive(Debug)]
struct StatsCollector<S: Eq + Hash = Atom> {
    data: HashMap<u8, StatsInternal<S>>,
}

impl<S: Eq + Hash> StatsCollector<S> {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    fn update(
        &mut self,
        k: u8,
        true_domain: S,
        mapped_domain: S,
        result: ClassificationResult,
        known_problems: Option<S>,
    ) where
        S: Clone,
    {
        let k_stats = self.data.entry(k).or_default();
        k_stats
            .true_domain
            .entry(true_domain)
            .or_default()
            .update(result, known_problems.clone());
        k_stats
            .mapped_domain
            .entry(mapped_domain)
            .or_default()
            .update(result, known_problems.clone());
        k_stats.global.update(result, known_problems);
    }

    fn dump_stats_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Error>
    where
        S: Serialize,
    {
        let wtr = file_open_write(
            path.as_ref(),
            WriteOptions::new().set_open_options(OpenOptions::new().create(true).truncate(true)),
        ).context("Cannot open writer for statistics.")?;
        let mut writer = WriterBuilder::new().has_headers(true).from_writer(wtr);

        #[derive(Serialize)]
        struct Out<'a, S> {
            k: u8,
            label: &'a S,
            corr: usize,
            corr_w_reason: usize,
            und: usize,
            und_w_reason: usize,
            wrong: usize,
            wrong_w_reason: usize,
            reasons: usize,
        };

        let mut ks: Vec<_> = self.data.keys().collect();
        ks.sort();
        for &k in ks {
            for (domain, stats) in &self.data[&k].true_domain {
                let out = Out {
                    k,
                    label: domain,
                    corr: stats
                        .results
                        .get(&(ClassificationResult::Correct, false))
                        .cloned()
                        .unwrap_or_default(),
                    corr_w_reason: stats
                        .results
                        .get(&(ClassificationResult::Correct, true))
                        .cloned()
                        .unwrap_or_default(),
                    und: stats
                        .results
                        .get(&(ClassificationResult::Undetermined, false))
                        .cloned()
                        .unwrap_or_default(),
                    und_w_reason: stats
                        .results
                        .get(&(ClassificationResult::Undetermined, true))
                        .cloned()
                        .unwrap_or_default(),
                    wrong: stats
                        .results
                        .get(&(ClassificationResult::Wrong, false))
                        .cloned()
                        .unwrap_or_default(),
                    wrong_w_reason: stats
                        .results
                        .get(&(ClassificationResult::Wrong, true))
                        .cloned()
                        .unwrap_or_default(),
                    reasons: stats.reasons.iter().map(|(_reason, count)| count).sum(),
                };

                writer
                    .serialize(&out)
                    .map_err(|err| format_err!("{}", err))?;
            }
        }

        Ok(())
    }
}

impl<S> Display for StatsCollector<S>
where
    S: Display + Eq + Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut keys: Vec<_> = self.data.keys().collect();
        keys.sort();

        for k in keys {
            // key must exist, because we just got it from the HashMap
            let k_stats = &self.data[k];
            writeln!(f, "knn with k={}:", k)?;
            writeln!(f, "{}", k_stats.global)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct StatsInternal<S: Eq + Hash = Atom> {
    true_domain: HashMap<S, StatsCounter<S>>,
    mapped_domain: HashMap<S, StatsCounter<S>>,
    global: StatsCounter<S>,
}

impl<S: Eq + Hash> Default for StatsInternal<S> {
    fn default() -> Self {
        Self {
            true_domain: HashMap::default(),
            mapped_domain: HashMap::default(),
            global: StatsCounter::default(),
        }
    }
}

#[derive(Debug)]
struct StatsCounter<S: Eq + Hash = Atom> {
    /// Counts pairs of `ClassificationResult` and if it is known problematic (bool).
    results: HashMap<(ClassificationResult, bool), usize>,
    /// Counts the problematic reasons
    reasons: HashMap<S, usize>,
}

impl<S> Display for StatsCounter<S>
where
    S: Display + Eq + Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{:>12}: {:>10}/{:>10}", "Success?", "", "Known Prob")?;
        for &res in &[
            (ClassificationResult::Correct),
            (ClassificationResult::Undetermined),
            (ClassificationResult::Wrong),
        ] {
            let wo_problems_count = self.results.get(&(res, false)).cloned().unwrap_or_default();
            let with_problems_count = self.results.get(&(res, true)).cloned().unwrap_or_default();
            writeln!(
                f,
                "{:>12}: {:>10}/{:>10}",
                res.to_string(),
                wo_problems_count,
                with_problems_count
            )?;
        }
        Ok(())
    }
}

impl<S: Eq + Hash> Default for StatsCounter<S> {
    fn default() -> Self {
        Self {
            results: HashMap::default(),
            reasons: HashMap::default(),
        }
    }
}

impl<S: Eq + Hash> StatsCounter<S> {
    fn update(&mut self, result: ClassificationResult, known_problems: Option<S>) {
        *self
            .results
            .entry((result, known_problems.is_some()))
            .or_default() += 1;
        if let Some(reason) = known_problems {
            *self.reasons.entry(reason).or_default() += 1;
        }
    }
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
    // Controls the maximum k for knn
    let most_k = 5;

    let writer: Box<Write> = cli_args
        .misclassifications
        .map(|path| {
            file_open_write(
                path,
                WriteOptions::new()
                    .set_open_options(OpenOptions::new().create(true).truncate(true)),
            )
        }).unwrap_or_else(|| Ok(Box::new(stdout())))
        .context("Cannot open writer for misclassifications.")?;
    let mut mis_writer = WriterBuilder::new().has_headers(true).from_writer(writer);

    info!("Start loading confusion domains...");
    prepare_confusion_domains(&cli_args.confusion_domains)?;
    info!("Done loading confusion domains.");
    info!("Start loading dnstap files...");
    let data = load_all_dnstap_files(&cli_args.base_dir, at_most_sequences_per_label)?;
    info!("Done loading dnstap files.");

    // Collect the stats during the execution and print them at the end
    let mut stats = StatsCollector::new();

    for fold in 0..at_most_sequences_per_label {
        info!("Testing for fold {}", fold);
        info!("Start splitting trainings and test data...");
        let (training_data, test) = split_training_test_data(&*data, fold as u8);
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

        for k in (1..=most_k).step_by(2) {
            let classification = knn(&*training_data, &*test_data, k as u8);
            assert_eq!(classification.len(), test_labels.len());
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
                        let known_problems = classify_sequence(sequence).map(Atom::from);
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
                        ).is_err()
                        {
                            error!(
                                "Cannot log misclassification for sequence: {}",
                                sequence.id()
                            );
                        }
                    },
                );
        }
    }

    // TODO print final stats
    println!("{}", stats);
    if let Some(path) = &cli_args.statistics {
        stats.dump_stats_to_file(path)?;
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
            }).transpose()
        }).collect::<Result<_, _>>()?;

    // Pairs of Label with Data (the Sequences)
    let data: Vec<LabelledSequences> = directories
        .into_par_iter()
        .map(|dir| {
            let label = dir
                .file_name()
                .expect("Each directory has a name")
                .to_string_lossy()
                .to_string();

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
                    }).transpose()
                }).collect::<Result<_, _>>()?;
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
                }).collect();

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
                    true_domain: label.into(),
                    mapped_domain: mapped_label.into(),
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
        }).cloned()
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
