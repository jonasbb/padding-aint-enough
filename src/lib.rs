#![cfg_attr(feature = "cargo-clippy", allow(renamed_and_removed_lints))]

pub extern crate chrome;
extern crate chrono;
extern crate dnstap as _dnstap;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
extern crate min_max_heap;
extern crate sequences;

pub use _dnstap::protos::{self, dnstap};
use _dnstap::{process_dnstap, sanity_check_dnstap};
use chrono::Duration;
use dnstap::Message_Type;
use failure::{Error, Fail};
use min_max_heap::MinMaxHeap;
use protos::DnstapContent;
use sequences::{MatchKey, Query, QuerySource, Sequence, SequenceElement, UnmatchedClientQuery};
use std::{
    collections::BTreeMap,
    fmt::{self, Display},
    path::Path,
};

pub fn take_largest<I, T>(iter: I, n: usize) -> Vec<T>
where
    I: IntoIterator<Item = T>,
    T: Ord,
{
    let mut iter = iter.into_iter();
    if n == 1 {
        // simply take the largest value and return it
        return iter.max().into_iter().collect();
    }

    let mut heap = MinMaxHeap::with_capacity(n);
    // fill the heap with n elements
    for _ in 0..n {
        match iter.next() {
            Some(v) => heap.push(v),
            None => break,
        }
    }

    // replace exisiting elements keeping the heap size
    for v in iter {
        heap.push_pop_min(v);
    }

    let res = heap.into_vec_desc();
    assert!(
        res.len() <= n,
        "Output vector only contains more than n elements."
    );
    res
}

pub mod common_sequence_classifications {
    pub const R001: &str = "R001 Single Domain. A + DNSKEY";
    pub const R002: &str = "R002 Single Domain with www redirect. A + DNSKEY + A (for www)";
    pub const R003: &str = "R003 Two domains for website. (A + DNSKEY) * 2";
    pub const R004_SIZE1: &str = "R004 Single packet of size 1.";
    pub const R004_SIZE2: &str = "R004 Single packet of size 2.";
    pub const R004_SIZE3: &str = "R004 Single packet of size 3.";
    pub const R004_SIZE4: &str = "R004 Single packet of size 4.";
    pub const R004_SIZE5: &str = "R004 Single packet of size 5.";
    pub const R004_SIZE6: &str = "R004 Single packet of size 6.";
    pub const R004_UNKNOWN: &str = "R004 A single packet of unknown size.";
    pub const R005: &str = "R005 Two domains for website second is CNAME.";
    pub const R006: &str = "R006 www redirect + Akamai";
    pub const R006_3RD_LVL_DOM: &str =
        "R006 www redirect + Akamai on 3rd-LVL domain without DNSSEC";
    pub const R007: &str = "R007 Unreachable Name Server";
    pub const R008: &str =
        "R008 Domain did not load properly and Chrome performed a Google search on the error page.";
}

/// Load a dnstap file and generate a Sequence from it
pub fn dnstap_to_sequence(dnstap_file: &Path) -> Result<Sequence, Error> {
    // process dnstap if available
    let mut events: Vec<protos::Dnstap> =
        process_dnstap(&*dnstap_file)?.collect::<Result<_, Error>>()?;

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

    // Place some sanity checks on the dnstap files
    sanity_check_dnstap(&events)?;

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
                let (dnsmsg, _size) = response_message.as_ref().expect("Unbound always sets this");
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
                let (dnsmsg, _size) = query_message.as_ref().expect("Unbound always sets this");
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
        // _ta queries are queries sent to the root servers to indicate which root DNSSEC key is trusted.
        // fedoraproject.org are artifacts due to the use of Fedora for the VMs, e.g., update queries and captive portal detection
        !(query.qtype == "NULL" && query.qname.starts_with("_ta")
            || query.qname.ends_with("fedoraproject.org."))
            || query.qname == ""
    });
    for msg in unanswered_client_queries {
        debug!("Unanswered forwarder query: {:?}", msg);
    }
    // the values are not necessarily in correct order, thus sort them here by end time
    // end time is the time when the response arrives, which is the most interesting field for the attacker
    matched.sort_by_key(|x| x.end);

    sanity_check_matched_queries(&matched)?;

    let seq = convert_to_sequence(&matched, dnstap_file.to_string_lossy().to_string());
    Ok(seq.ok_or_else(|| format_err!("Sequence is empty"))?)
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

/// A short-lived wrapper for some `Fail` type that displays it and all its
/// causes delimited by the string ": ".
pub struct DisplayCauses<'a>(&'a Fail);

impl<'a> Display for DisplayCauses<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, f)?;
        let mut x: &Fail = self.0;
        while let Some(cause) = x.cause() {
            f.write_str(": ")?;
            Display::fmt(&cause, f)?;
            x = cause;
        }
        Ok(())
    }
}

pub trait FailExt {
    fn display_causes(&self) -> DisplayCauses;
}

impl<T> FailExt for T
where
    T: Fail,
{
    fn display_causes(&self) -> DisplayCauses {
        DisplayCauses(self)
    }
}

pub trait ErrorExt {
    fn display_causes(&self) -> DisplayCauses;
}

impl ErrorExt for Error {
    fn display_causes(&self) -> DisplayCauses {
        DisplayCauses(self.as_fail())
    }
}

fn sanity_check_matched_queries(matched: &[Query]) -> Result<(), Error> {
    if matched.is_empty() {
        bail!("No DNS query/response pairs could be matched.");
    }

    let mut found_resolver_query = false;

    for query in matched {
        if query.source == QuerySource::Forwarder && query.qtype == "A" {
            found_resolver_query = true;
            break;
        }
    }
    if !found_resolver_query {
        bail!("There must be at least one resolver query for type A.")
    }

    Ok(())
}
