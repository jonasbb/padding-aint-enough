#![allow(dead_code)]
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
use csv::ReaderBuilder;
use encrypted_dns::{
    dnstap::Message_Type,
    protos::DnstapContent,
    sequences::{knn, split_training_test_data, Sequence, SequenceElement},
    MatchKey, Query, QuerySource, UnmatchedClientQuery,
};
use failure::{Error, ResultExt};
use misc_utils::fs::{file_open_read, file_open_write, WriteOptions};
use rayon::prelude::*;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use structopt::StructOpt;

lazy_static! {
    static ref CONFUSION_DOMAINS: RwLock<Arc<BTreeMap<String, String>>> =
        RwLock::new(Arc::new(BTreeMap::new()));
}

#[derive(StructOpt, Debug)]
#[structopt(author = "", raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
struct CliArgs {
    #[structopt(parse(from_os_str))]
    base_dir: PathBuf,
    #[structopt(short = "d", long = "confusion_domains", parse(from_os_str))]
    confusion_domains: Vec<PathBuf>,
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

    prepare_confusion_domains(&cli_args.confusion_domains)?;

    let directories: Vec<PathBuf> = fs::read_dir(cli_args.base_dir)?
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

    let check_confusion_domains = make_check_confusion_domains();

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

            let sequences: Vec<Sequence> = filenames
                .into_iter()
                .map(|dnstap_file| {
                    debug!("Processing dnstap file '{}'", dnstap_file.display());
                    Ok(process_dnstap(&*dnstap_file).with_context(|_| {
                        format_err!("Processing dnstap file '{}'", dnstap_file.display())
                    })?)
                })
                .collect::<Result<_, Error>>()?;

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

    let most_k = 5;
    let mut res = vec![(0, 0, 0); most_k];
    for fold in 0..10 {
        info!("Testing for fold {}", fold);
        let (training_data, test) = split_training_test_data(&*data, fold);
        let len = test.len();
        let (test_labels, test_data) = test.into_iter().fold(
            (Vec::with_capacity(len), Vec::with_capacity(len)),
            |(mut labels, mut data), elem| {
                labels.push(elem.0);
                data.push(elem.1);
                (labels, data)
            },
        );

        for k in 1..=most_k {
            let classification = knn(&*training_data, &*test_data, k as u8);
            assert_eq!(classification.len(), test_labels.len());
            let (correct, undecided) = classification.into_iter().zip(&test_labels).fold(
                (0, 0),
                |(mut corr, mut und), (class, label)| {
                    if class == *label {
                        corr += 1;
                    }
                    if class.contains(&*label) {
                        und += 1;
                    }
                    (corr, und)
                },
            );
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

fn process_dnstap(dnstap_file: &Path) -> Result<Sequence, Error> {
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
    matched.retain(|query| !(query.qtype == "NULL" && query.qname.starts_with("_ta")));
    for msg in unanswered_client_queries {
        debug!("Unanswered forwarder query: {:?}", msg);
    }
    // the values are not necessarily in correct order, thus sort them here by end time
    // end time is the time when the response arrives, which is the most interesting field for the attacker
    matched.sort_by_key(|x| x.end);

    let seq = convert_to_sequence(&matched);

    // let fname = dnstap_file.with_extension("sequence.pickle");
    // save_as_pickle(&fname, &seq)?;

    Ok(seq)
}

fn convert_to_sequence(data: &[Query]) -> Sequence {
    let base_gap_size = Duration::microseconds(1000);

    let mut last_end = None;
    Sequence::new(
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
    )
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

fn save_as_pickle<D: Serialize>(path: &Path, data: &D) -> Result<(), Error> {
    let mut wtr = file_open_write(
        path,
        WriteOptions::default().set_open_options(OpenOptions::new().create(true).truncate(true)),
    ).map_err(|err| {
        format_err!("Opening output file '{}' failed: {}", path.display(), err)
    })?;
    serde_pickle::to_writer(&mut wtr, data, true)?;
    Ok(())
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
            file_open_read(path).map_err(|err| {
                format_err!(
                    "Opening confusion file '{}' failed: {}",
                    path.display(),
                    err
                )
            })?,
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
