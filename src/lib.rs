#![feature(nll)]
#![feature(transpose_result)]
#![feature(try_from)]

extern crate chrono;
#[macro_use]
extern crate failure;
extern crate framestream;
#[macro_use]
extern crate log;
extern crate min_max_heap;
extern crate misc_utils;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate protobuf;
extern crate rayon;
extern crate trust_dns;

pub mod protos;
pub mod sequences;

use chrono::{DateTime, Utc};
use failure::{Error, ResultExt};
use framestream::DecoderReader;
use min_max_heap::MinMaxHeap;
use misc_utils::fs::file_open_read;
pub use protos::dnstap;
use std::{convert::TryFrom, path::Path};

pub fn process_dnstap<P: AsRef<Path>>(
    path: P,
) -> Result<impl Iterator<Item = Result<protos::Dnstap, Error>>, Error> {
    let path = path.as_ref();
    let path_str = path.to_string_lossy().to_string();

    let rdr = file_open_read(path)
        .with_context(|_| format!("Opening input file '{}' failed", path.display()))?;
    let fstrm = DecoderReader::with_content_type(rdr, "protobuf:dnstap.Dnstap".into());

    Ok(fstrm
        .into_iter()
        .map(move |msg| -> Result<Option<protos::Dnstap>, Error> {
            let raw_dnstap = protobuf::parse_from_bytes::<dnstap::Dnstap>(&msg?)
                .context("Parsing protobuf failed.")?;
            match protos::Dnstap::try_from(raw_dnstap) {
                Ok(dnstap) => Ok(Some(dnstap)),
                Err(err) => {
                    warn!(
                        "Skipping DNS event due to conversion errror in file '{}': {}",
                        path_str, err
                    );
                    Ok(None)
                }
            }
        })
        .filter_map(|x| x.transpose()))
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize)]
pub struct Query {
    pub source: QuerySource,
    pub qname: String,
    pub qtype: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub query_size: u32,
    pub response_size: u32,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize)]
pub enum QuerySource {
    Client,
    Forwarder,
    ForwarderLostQuery,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct MatchKey {
    pub qname: String,
    pub qtype: String,
    pub id: u16,
    pub port: u16,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct UnmatchedClientQuery {
    pub qname: String,
    pub qtype: String,
    pub start: DateTime<Utc>,
    pub size: u32,
}

pub fn take_smallest<I, T>(iter: I, n: usize) -> Vec<T>
where
    I: IntoIterator<Item = T>,
    T: Ord,
{
    let mut iter = iter.into_iter();
    if n == 1 {
        // simply take the largest value and return it
        return iter.min().into_iter().collect();
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
        heap.push_pop_max(v);
    }

    let res = heap.into_vec_asc();
    assert!(
        res.len() <= n,
        "Output vector only contains more than n elements."
    );
    res
}

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
}
