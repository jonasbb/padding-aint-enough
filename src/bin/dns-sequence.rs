#![feature(transpose_result)]

extern crate chrono;
extern crate csv;
extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate structopt;
extern crate encrypted_dns;
extern crate misc_utils;
extern crate rayon;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_pickle;

use chrono::Duration;
use csv::{ReaderBuilder, Writer as CsvWriter, WriterBuilder};
use encrypted_dns::{
    dnstap::Message_Type,
    protos::DnstapContent,
    sequences::{knn, split_training_test_data, Sequence, SequenceElement},
    take_largest, MatchKey, Query, QuerySource, UnmatchedClientQuery,
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

/// Load a dnstap file and generate a Sequence from it
fn process_dnstap(dnstap_file: &Path) -> Result<Option<Sequence>, Error> {
    // process dnstap if available
    let mut events: Vec<encrypted_dns::protos::Dnstap> =
        encrypted_dns::process_dnstap(&*dnstap_file)?.collect::<Result<_, Error>>()?;

    // the dnstap events can be out of order, so sort them by timestamp
    // always take the later timestamp if there are multiple
    events.sort_by_key(|ev| {
        let DnstapContent::Message {
            query_time,
            response_time,
            ..
        } = ev.content;
        if let Some(time) = response_time {
            return time;
        } else if let Some(time) = query_time {
            return time;
        } else {
            panic!("The dnstap message must contain either a query or response time.")
        }
    });

    let mut unanswered_client_queries: BTreeMap<MatchKey, UnmatchedClientQuery> = BTreeMap::new();
    let mut matched = Vec::new();

    for ev in events
            .into_iter()
                // search for the CLIENT_RESPONE `start.example.` message as the end of the prefetching events
            .skip_while(|ev| {
                let DnstapContent::Message {
                    message_type,
                    ref response_message,
                    ..
                } = ev.content;
                if message_type == Message_Type::CLIENT_RESPONSE {
                    let (dnsmsg, _size) =
                        response_message.as_ref().expect("Unbound always sets this");
                    let qname = dnsmsg.queries()[0].name().to_utf8();
                    if qname == "start.example." {
                        return false;
                    }
                }
                true
            })
            // the skip while returns the CLIENT_RESPONSE with `start.example.`
            // We want to remove this as well, so skip over the first element here
            .skip(1)
            // Only process messages until the end message is found in form of the first (thus CLIENT_QUERY)
            // message forr domain `end.example.`
            .take_while(|ev| {
                let DnstapContent::Message {
                    message_type,
                    ref query_message,
                    ..
                } = ev.content;
                if message_type == Message_Type::CLIENT_QUERY {
                    let (dnsmsg, _size) =
                        query_message.as_ref().expect("Unbound always sets this");
                    let qname = dnsmsg.queries()[0].name().to_utf8();
                    if qname == "end.example." {
                        return false;
                    }
                }
                true
            }) {
        let DnstapContent::Message {
            message_type,
            query_message,
            response_message,
            query_time,
            response_time,
            ..
        } = ev.content;
        match message_type {
            Message_Type::FORWARDER_QUERY => {
                let (dnsmsg, size) = query_message.expect("Unbound always sets this");
                let qname = dnsmsg.queries()[0].name().to_utf8();
                let qtype = dnsmsg.queries()[0].query_type().to_string();
                let id = dnsmsg.id();
                let start = query_time.expect("Unbound always sets this");

                let key = MatchKey {
                    qname: qname.clone(),
                    qtype: qtype.clone(),
                    id,
                    port: 0,
                };
                let value = UnmatchedClientQuery {
                    qname,
                    qtype,
                    start,
                    size: size as u32,
                };
                let existing_value = unanswered_client_queries.insert(key, value);
                if let Some(existing_value) = existing_value {
                    info!(
                        "Duplicate Forwarder Query for '{}' ({})",
                        existing_value.qname, existing_value.qtype
                    );
                }
            }

            Message_Type::FORWARDER_RESPONSE => {
                let (dnsmsg, size) = response_message.expect("Unbound always sets this: FR r msg");
                let qname = dnsmsg.queries()[0].name().to_utf8();
                let qtype = dnsmsg.queries()[0].query_type().to_string();
                let start = query_time.expect("Unbound always sets this: FR q time");
                let id = dnsmsg.id();
                let end = response_time.expect("Unbound always sets this: FR r time");

                let key = MatchKey {
                    qname: qname.clone(),
                    qtype: qtype.clone(),
                    id,
                    port: 0,
                };
                if let Some(unmatched) = unanswered_client_queries.remove(&key) {
                    matched.push(Query {
                        source: QuerySource::Forwarder,
                        qname,
                        qtype,
                        start,
                        end,
                        query_size: unmatched.size,
                        response_size: size as u32,
                    });
                } else {
                    info!("Unmatched Forwarder Response for '{}' ({})", qname, qtype);
                };
            }

            _ => {}
        }
    }

    // cleanup some messages
    // filter out all the queries which are just noise
    matched.retain(|query| {
        !(query.qtype == "NULL" && query.qname.starts_with("_ta"))
            && query.qname != "fedoraproject.org."
    });
    for msg in unanswered_client_queries {
        debug!("Unanswered forwarder query: {:?}", msg);
    }
    // the values are not necessarily in correct order, thus sort them here by end time
    // end time is the time when the response arrives, which is the most interesting field for the attacker
    matched.sort_by_key(|x| x.end);

    let seq = convert_to_sequence(&matched, dnstap_file.to_string_lossy().to_string());

    Ok(seq)
}

/// Takes a list of Queries and returns a Sequence
///
/// The functions abstracts over some details of Queries, such as absolute size and absolute time.
fn convert_to_sequence(data: &[Query], identifier: String) -> Option<Sequence> {
    let base_gap_size = Duration::microseconds(1000);

    if data.is_empty() {
        return None;
    }

    let mut last_end = None;
    Some(Sequence::new(
        data.into_iter()
            .flat_map(|d| {
                let mut gap = None;
                if let Some(last_end) = last_end {
                    gap = gap_size(d.end - last_end, base_gap_size);
                }
                last_end = Some(d.end);

                let size = pad_size(d.response_size, false, Padding::Q128R468);
                gap.into_iter().chain(Some(size))
            })
            .collect(),
        identifier,
    ))
}

fn gap_size(gap: Duration, base: Duration) -> Option<SequenceElement> {
    if gap <= base {
        return None;
    }
    let mut gap = gap;
    let mut out = 0;
    while gap > base {
        gap = gap - base;
        out += 1;
    }
    let dist = f64::from(out).log2() as u8;
    if dist == 0 {
        None
    } else {
        SequenceElement::Gap(dist).into()
    }
}

fn pad_size(size: u32, is_query: bool, padding: Padding) -> SequenceElement {
    use Padding::*;
    SequenceElement::Size(match (padding, is_query) {
        (Q128R468, true) => block_padding(size, 128) / 128,
        (Q128R468, false) => block_padding(size, 468) / 468,
    } as u8)
}

fn block_padding(size: u32, block_size: u32) -> u32 {
    if size % block_size == 0 {
        size
    } else {
        size / block_size * block_size + block_size
    }
}

enum Padding {
    Q128R468,
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
                    Ok(process_dnstap(&*dnstap_file).with_context(|_| {
                        format!("Processing dnstap file '{}'", dnstap_file.display())
                    })?)
                })
                .filter_map(|seq| seq.transpose())
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

    let reason = match &*packets {
        [] => {
            error!(
                "Empty sequence for ID {}. Should never occur",
                sequence.id()
            );
            None
        }
        [SequenceElement::Size(n)] => Some(match n {
            0 => unreachable!("Packets of size 0 may never occur."),
            1 => "R004 Single packet of size 1.",
            2 => "R004 Single packet of size 2.",
            3 => "R004 Single packet of size 3.",
            4 => "R004 Single packet of size 4.",
            5 => "R004 Single packet of size 5.",
            6 => "R004 Single packet of size 6.",
            _ => "R004 A single packet of unknown size.",
        }),
        [SequenceElement::Size(1), SequenceElement::Size(2)] => {
            Some("R001 Single Domain. A + DNSKEY")
        }
        [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1)] => {
            Some("R002 Single Domain with www redirect. A + DNSKEY + A (for www)")
        }
        [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1), SequenceElement::Size(2)] => {
            Some("R003 Two domains for website. (A + DNSKEY) * 2")
        }
        [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(2)] => {
            Some("R005 Two domains for website, second is CNAME.")
        }
        [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(2)] => {
            Some("R006 www redirect + Akamai")
        }
        [SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(2)] => {
            Some("R006 www redirect + Akamai on 3rd-LVL domain without DNSSEC")
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
                Some("R007 Unreachable Name Server")
            } else {
                None
            }
        }
    };

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
