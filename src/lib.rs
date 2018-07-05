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
        .map_err(|err| format_err!("Opening input file '{}' failed: {}", path.display(), err))?;
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
    let mut heap = MinMaxHeap::with_capacity(n);
    let mut iter = iter.into_iter();

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
    let mut heap = MinMaxHeap::with_capacity(n);
    let mut iter = iter.into_iter();

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
