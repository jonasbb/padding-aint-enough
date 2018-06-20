#![allow(dead_code)]

extern crate chrono;
extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
#[macro_use]
extern crate structopt;
extern crate encrypted_dns;
extern crate misc_utils;
extern crate pylib;
extern crate serde;
extern crate serde_pickle;

use chrono::Duration;
use encrypted_dns::{
    dnstap::Message_Type, protos::DnstapContent, MatchKey, Query, QuerySource, UnmatchedClientQuery,
};
use failure::{Error, ResultExt};
use misc_utils::fs::{file_open_write, WriteOptions};
use pylib::*;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(author = "", raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
struct CliArgs {
    #[structopt(parse(from_os_str))]
    dnstap_files: Vec<PathBuf>,
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

    for dnstap_file in cli_args.dnstap_files {
        process_dnstap(&*dnstap_file)
            .with_context(|_| format_err!("Processing dnstap file '{}'", dnstap_file.display()))?;
    }

    Ok(())
}

fn process_dnstap(dnstap_file: &Path) -> Result<(), Error> {
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

    convert_to_sequence(&matched);

    // let fname = dnstap_file.with_extension("sequence.pickle");
    // save_as_pickle(&fname, &matched)?;

    Ok(())
}

fn convert_to_sequence(data: &[Query]) -> Vec<SequenceElement> {
    let base_gap_size = Duration::microseconds(1000);

    let mut last_end = None;
    data.into_iter()
        .flat_map(|d| {
            let mut gap = None;
            if let Some(last_end) = last_end {
                gap = gap_size(d.end - last_end, base_gap_size);
                println!("Gap: {:?}", gap);
            }
            last_end = Some(d.end);

            let size = pad_size(d.response_size, false, Padding::Q128R468);
            println!("Size: {:?} {} {} -- {}", size, d.start, d.end, d.qname);
            gap.into_iter().chain(Some(size))
        })
        .for_each(|x| println!("{:?}", x));
    // for d in data {
    //     if let Some(last_end) = last_end {
    //         let gap: Duration = d.end - last_end;
    //         println!("{:?}", gap_size(gap, base_gap_size));
    //     }
    //     last_end = Some(d.end);
    //     println!(
    //         "{:?} {} {} -- {}",
    //         pad_size(d.response_size, false, Padding::Q128R468),
    //         d.start,
    //         d.end,
    //         d.qname
    //     );
    // }
    unimplemented!()
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
