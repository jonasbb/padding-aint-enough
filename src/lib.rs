#![feature(transpose_result)]
#![feature(try_from)]

extern crate chrono;
#[macro_use]
extern crate failure;
extern crate framestream;
extern crate misc_utils;
extern crate protobuf;
extern crate trust_dns;

mod protos;

use failure::Error;
use framestream::DecoderReader;
use misc_utils::fs::*;
pub use protos::dnstap;
use std::convert::TryFrom;
use std::path::Path;

pub fn get_dns_name<P: AsRef<Path>>(path: P) -> Result<(), Error> {
    let path = path.as_ref();

    let rdr = file_open_read(path)
        .map_err(|err| format_err!("Opening input file '{}' failed: {}", path.display(), err))?;
    let fstrm = DecoderReader::new(rdr, None);

    fstrm
        .into_iter()
        .map(|msg| -> Result<protos::Dnstap, Error> {
            Ok(protos::Dnstap::try_from(protobuf::parse_from_bytes::<
                dnstap::Dnstap,
            >(&*(msg?))?)?)
        })
        .for_each(|msg| println!("{:?}", msg));

    Ok(())
}

pub fn x() -> Result<(), Error> {
    get_dns_name("./test/data/exp/dnstap-1.log")
}
