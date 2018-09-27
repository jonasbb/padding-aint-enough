#![feature(try_from)]

extern crate chrono;
#[macro_use]
extern crate failure;
extern crate protobuf;
extern crate trust_dns;

pub mod protos;

use dnstap::Message_Type;
use failure::Error;
pub use protos::dnstap;
use protos::DnstapContent;

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
