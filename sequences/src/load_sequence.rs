use crate::{
    precision_sequence::PrecisionSequence, AbstractQueryResponse, LoadDnstapConfig, MatchKey,
    Query, QuerySource, Sequence, SequenceElement, UnmatchedClientQuery,
};
use chrono::Duration;
use dnstap::{
    dnstap::Message_Type,
    process_dnstap,
    protos::{self, DnstapContent},
    sanity_check_dnstap,
};
use failure::{bail, format_err, Error};
use log::{debug, info};
use std::{collections::BTreeMap, path::Path};

pub(crate) enum Padding {
    Q128R468,
}

/// Load a dnstap file and generate a [`Sequence`] from it
pub fn dnstap_to_sequence(dnstap_file: &Path) -> Result<Sequence, Error> {
    dnstap_to_sequence_with_config(dnstap_file, LoadDnstapConfig::Normal)
}

/// Load a dnstap file and generate a [`Sequence`] from it
///
/// `config` allows to alter the loading according to [`LoadDnstapConfig`]
pub fn dnstap_to_sequence_with_config(
    dnstap_file: &Path,
    config: LoadDnstapConfig,
) -> Result<Sequence, Error> {
    let matched = load_matching_query_responses_from_dnstap(dnstap_file)?;
    let seq = convert_to_sequence(&matched, dnstap_file.to_string_lossy().to_string(), config);
    Ok(seq.ok_or_else(|| format_err!("Sequence is empty"))?)
}

/// Load a dnstap file and generate a [`PrecisionSequence`] from it
pub fn dnstap_to_precision_sequence(dnstap_file: &Path) -> Result<PrecisionSequence, Error> {
    let matched = load_matching_query_responses_from_dnstap(dnstap_file)?;
    let seq = convert_to_precision_sequence(&matched, dnstap_file.to_string_lossy().to_string());
    Ok(seq.ok_or_else(|| format_err!("PrecisionSequence is empty"))?)
}

fn load_matching_query_responses_from_dnstap(dnstap_file: &Path) -> Result<Vec<Query>, Error> {
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
    Ok(matched)
}

/// Takes a list of Queries and returns a [`Sequence`]
///
/// The functions abstracts over some details of Queries, such as absolute size and absolute time.
/// The function only returns [`None`], if the input sequence is empty.
pub fn convert_to_sequence<'a, QR>(
    data: &'a [QR],
    identifier: String,
    config: LoadDnstapConfig,
) -> Option<Sequence>
where
    QR: 'a,
    &'a QR: Into<AbstractQueryResponse>,
{
    let base_gap_size = Duration::microseconds(1000);

    if data.is_empty() {
        return None;
    }

    let mut last_time = None;
    Some(Sequence::new(
        data.iter()
            .flat_map(|d| {
                let d: AbstractQueryResponse = d.into();

                let mut gap = None;
                if let Some(last_end) = last_time {
                    gap = gap_size(d.time - last_end, base_gap_size);
                }

                let mut size = Some(pad_size(d.size, false, Padding::Q128R468));

                // The config allows us to remove either Gap or Size
                match config {
                    LoadDnstapConfig::Normal => {}
                    LoadDnstapConfig::PerfectPadding => {
                        // We need to enforce Gap(0) messages to ensure that counting the number of messages still works

                        // If `last_end` is set, then there was a previous message, so we need to add a gap
                        // Only add a gap, if there is not one already
                        if last_time.is_some() && gap.is_none() {
                            gap = Some(SequenceElement::Gap(0));
                        }
                        size = None;
                    }
                    LoadDnstapConfig::PerfectTiming => {
                        gap = None;
                    }
                }

                // Mark this as being not the first iteration anymore
                last_time = Some(d.time);

                gap.into_iter().chain(size)
            })
            .collect(),
        identifier,
    ))
}

/// Takes a list of Queries and returns a [`PrecisionSequence`]
///
/// The functions abstracts over some details of [`Query`]s, such as absolute size and absolute time.
/// The function only returns [`None`], if the input sequence is empty.
pub fn convert_to_precision_sequence<'a, QR>(
    data: &'a [QR],
    identifier: String,
) -> Option<PrecisionSequence>
where
    QR: 'a,
    &'a QR: Into<AbstractQueryResponse>,
{
    if data.is_empty() {
        return None;
    }

    Some(PrecisionSequence::new(
        data.iter().map(|x| x.into()),
        identifier,
    ))
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

pub(crate) fn gap_size(gap: Duration, base: Duration) -> Option<SequenceElement> {
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

pub(crate) fn pad_size(size: u32, is_query: bool, padding: Padding) -> SequenceElement {
    use self::Padding::*;
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

#[test]
fn test_block_padding() {
    assert_eq!(0, block_padding(0, 128));
    assert_eq!(128, block_padding(1, 128));
    assert_eq!(128, block_padding(127, 128));
    assert_eq!(128, block_padding(128, 128));
    assert_eq!(128 * 2, block_padding(129, 128));
}
