//! Processing DNSTAP files and extracting Sequences from them
//!
//! The module has to entry points for building sequences: [`build_sequence`] and
//! [`build_precision_sequence`]
//!
//! Additionally, the function [`load_matching_query_responses_from_dnstap`] is exported, which
//! returns a list of Query/Response pairs for both the client and forwarder queries. If only part
//! of the data is needed (e.g., only the forwarder messages) additional filtering must be applied.

use crate::{
    load_sequence::{convert_to_precision_sequence, convert_to_sequence, LoadSequenceConfig},
    precision_sequence::PrecisionSequence,
    AbstractQueryResponse, Sequence,
};
use anyhow::{anyhow, bail, Context as _, Error};
use chrono::{DateTime, Utc};
use dnstap::{
    dnstap::Message_Type,
    process_dnstap,
    protos::{self, DnstapContent},
    sanity_check_dnstap,
};
use log::{debug, info};
use serde::Serialize;
use std::{collections::BTreeMap, path::Path};

/// Representation of a single Query/Response pair in dnstap
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

impl From<Query> for AbstractQueryResponse {
    fn from(other: Query) -> Self {
        AbstractQueryResponse {
            time: other.end.naive_utc(),
            size: other.response_size,
        }
    }
}

impl From<&Query> for AbstractQueryResponse {
    fn from(other: &Query) -> Self {
        AbstractQueryResponse {
            time: other.end.naive_utc(),
            size: other.response_size,
        }
    }
}

/// Source for the Query or Response
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize)]
pub enum QuerySource {
    Client,
    Forwarder,
}

/// Lookup key to match queries to their responses
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
struct MatchKey {
    pub qname: String,
    pub qtype: String,
    pub id: u16,
    pub port: u16,
}

/// Query data which is not yet matched with the fitting response pair
///
/// The id and port information are stored in the corresponding [`MatchKey`] struct.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
struct UnmatchedClientQuery {
    pub qname: String,
    pub qtype: String,
    pub start: DateTime<Utc>,
    pub size: u32,
}

/// Load a dnstap file and generate a [`Sequence`] from it
///
/// `config` allows to alter the loading according to [`LoadSequenceConfig`]
pub fn build_sequence(dnstap_file: &Path, config: LoadSequenceConfig) -> Result<Sequence, Error> {
    let matched = load_matching_query_responses_from_dnstap(dnstap_file)?;
    let forwarder_queries = matched
        .into_iter()
        .filter(|q| q.source == QuerySource::Forwarder);
    let seq = convert_to_sequence(
        forwarder_queries,
        dnstap_file.to_string_lossy().to_string(),
        config,
    );
    Ok(seq.ok_or_else(|| anyhow!("Sequence is empty"))?)
}

/// Load a dnstap file and generate a [`PrecisionSequence`] from it
pub fn build_precision_sequence(dnstap_file: &Path) -> Result<PrecisionSequence, Error> {
    let matched = load_matching_query_responses_from_dnstap(dnstap_file)?;
    let forwarder_queries = matched
        .into_iter()
        .filter(|q| q.source == QuerySource::Forwarder);
    let seq =
        convert_to_precision_sequence(forwarder_queries, dnstap_file.to_string_lossy().to_string());
    Ok(seq.ok_or_else(|| anyhow!("PrecisionSequence is empty"))?)
}

/// Load all pairs of client Query/Responses and forwarder Query/Responses
///
/// The output needs to be filtered if only client or forwarder messages should be included
pub fn load_matching_query_responses_from_dnstap(dnstap_file: &Path) -> Result<Vec<Query>, Error> {
    // process dnstap if available
    let mut events: Vec<protos::Dnstap> = process_dnstap(&*dnstap_file)?
        .collect::<Result<_, Error>>()
        .with_context(|| "Failed to read the raw DNSTAP file")?;

    // the dnstap events can be out of order, so sort them by timestamp
    // always take the later timestamp if there are multiple
    events.sort_by_key(|ev| {
        let DnstapContent::Message {
            query_time,
            response_time,
            ..
        } = ev.content;
        if let Some(time) = response_time {
            time
        } else if let Some(time) = query_time {
            time
        } else {
            panic!("The dnstap message must contain either a query or response time.")
        }
    });

    // Place some sanity checks on the dnstap files
    sanity_check_dnstap(&events)?;

    let mut unanswered_client_queries: BTreeMap<MatchKey, UnmatchedClientQuery> = BTreeMap::new();
    let mut unanswered_forwarder_queries: BTreeMap<MatchKey, UnmatchedClientQuery> =
        BTreeMap::new();
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
        })
    {
        let DnstapContent::Message {
            message_type,
            query_message,
            response_message,
            query_time,
            response_time,
            query_port,
            ..
        } = ev.content;
        match message_type {
            Message_Type::CLIENT_QUERY => {
                let (dnsmsg, size) = query_message.expect("Unbound always sets this");
                let qname = dnsmsg.queries()[0].name().to_utf8();
                let qtype = dnsmsg.queries()[0].query_type().to_string();
                let id = dnsmsg.id();
                let start = query_time.expect("Unbound always sets this");
                let port = query_port.expect("Unbound always sets this");

                let key = MatchKey {
                    qname: qname.clone(),
                    qtype: qtype.clone(),
                    id,
                    port,
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
                        "Duplicate Client Query for '{}' ({})",
                        existing_value.qname, existing_value.qtype
                    );
                }
            }

            Message_Type::CLIENT_RESPONSE => {
                let (dnsmsg, size) = response_message.expect("Unbound always sets this: FR r msg");
                let qname = dnsmsg.queries()[0].name().to_utf8();
                let qtype = dnsmsg.queries()[0].query_type().to_string();
                let id = dnsmsg.id();
                let end = response_time.expect("Unbound always sets this: FR r time");
                let port = query_port.expect("Unbound always sets this");

                let key = MatchKey {
                    qname: qname.clone(),
                    qtype: qtype.clone(),
                    id,
                    port,
                };
                if let Some(unmatched) = unanswered_client_queries.remove(&key) {
                    matched.push(Query {
                        source: QuerySource::Client,
                        qname,
                        qtype,
                        start: unmatched.start,
                        end,
                        query_size: unmatched.size,
                        response_size: size as u32,
                    });
                } else {
                    info!("Unmatched Client Response for '{}' ({})", qname, qtype);
                };
            }

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
                let existing_value = unanswered_forwarder_queries.insert(key, value);
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
                if let Some(unmatched) = unanswered_forwarder_queries.remove(&key) {
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
        !(query.qtype == "NULL" && query.qname.starts_with("_ta")) || query.qname == ""
    });
    for msg in unanswered_client_queries {
        debug!("Unanswered client query: {:?}", msg);
    }
    for msg in unanswered_forwarder_queries {
        debug!("Unanswered forwarder query: {:?}", msg);
    }
    // the values are not necessarily in correct order, thus sort them here by end time
    // end time is the time when the response arrives, which is the most interesting field for the attacker
    matched.sort_by_key(|x| x.end);

    sanity_check_matched_queries(&matched)?;
    Ok(matched)
}

/// Run a basic sanity check on the dnstap file to make sure it is not empty and some queries of type A could be found
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
