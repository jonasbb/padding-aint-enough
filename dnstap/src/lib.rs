#![cfg_attr(feature = "cargo-clippy", allow(renamed_and_removed_lints))]

pub mod protos;

pub use crate::protos::dnstap;
use crate::{dnstap::Message_Type, protos::DnstapContent};
use failure::{bail, Error, ResultExt};
use framestream::DecoderReader;
use log::warn;
use misc_utils::fs::file_open_read;
use protobuf;
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
        .filter_map(Result::transpose))
}

pub fn sanity_check_dnstap(events: &[protos::Dnstap]) -> Result<(), Error> {
    let mut client_query_start_count = 0;
    let mut client_response_start_count = 0;
    let mut client_query_end_count = 0;
    let mut client_response_end_count = 0;

    for ev in events {
        match &ev.content {
            DnstapContent::Message {
                message_type: Message_Type::CLIENT_QUERY,
                ref query_message,
                ..
            } => {
                let (dnsmsg, _size) = query_message.as_ref().expect("Unbound always sets this");
                let qname = dnsmsg.queries()[0].name().to_utf8();

                match &*qname {
                    "start.example." => client_query_start_count += 1,
                    "end.example." => client_query_end_count += 1,
                    _ => {}
                }
            }

            DnstapContent::Message {
                message_type: Message_Type::CLIENT_RESPONSE,
                ref response_message,
                ..
            } => {
                let (dnsmsg, _size) = response_message.as_ref().expect("Unbound always sets this");
                let qname = dnsmsg.queries()[0].name().to_utf8();

                match &*qname {
                    "start.example." => client_response_start_count += 1,
                    "end.example." => client_response_end_count += 1,
                    _ => {}
                }
            }

            _ => {}
        }
    }

    if client_query_start_count == 0 {
        bail!("Expected at least 1 CLIENT_QUERY for 'start.example.' but found none");
    } else if client_query_end_count != 1 {
        bail!(
            "Unexpected number of CLIENT_QUERYs for 'end.example.': {}, expected 1",
            client_query_end_count
        );
    } else if client_response_start_count != 1 {
        bail!(
            "Unexpected number of CLIENT_RESPONSEs for 'start.example.': {}, expected 1",
            client_response_start_count
        );
    } else if client_response_end_count != 1 {
        bail!(
            "Unexpected number of CLIENT_RESPONSEs for 'end.example.': {}, expected 1",
            client_response_end_count
        );
    }

    Ok(())
}
