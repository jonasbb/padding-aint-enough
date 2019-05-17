use crate::{AbstractQueryResponse, PrecisionSequence, Sequence};
use chrono::NaiveDateTime;
use colored::Colorize;
use failure::{bail, format_err, Error, ResultExt};
use hashbrown::HashMap;
use itertools::Itertools;
use log::debug;
use pcap::{Capture, Linktype, Packet as PcapPacket};
use pnet::packet::{
    ethernet::EthernetPacket, ip::IpNextHeaderProtocols, ipv4::Ipv4Packet, tcp::TcpPacket, Packet,
};
use rustls::internal::msgs::{
    codec::Codec, enums::ContentType as TlsContentType, message::Message as TlsMessage,
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::HashSet,
    mem,
    net::{Ipv4Addr, SocketAddrV4},
    path::Path,
};

/// Identifier for a two-way TCP flow
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct FlowId {
    pub ip0: Ipv4Addr,
    pub port0: u16,
    pub ip1: Ipv4Addr,
    pub port1: u16,
}

impl FlowId {
    pub fn from_pairs(p0: (Ipv4Addr, u16), p1: (Ipv4Addr, u16)) -> Self {
        let ((ip0, port0), (ip1, port1)) = if p0 <= p1 { (p0, p1) } else { (p1, p0) };
        Self {
            ip0,
            port0,
            ip1,
            port1,
        }
    }

    pub fn from_ip_and_tcp(ipv4: &Ipv4Packet, tcp: &TcpPacket) -> Self {
        let p0 = (ipv4.get_source(), tcp.get_source());
        let p1 = (ipv4.get_destination(), tcp.get_destination());
        Self::from_pairs(p0, p1)
    }
}

impl From<&TlsRecord> for FlowId {
    fn from(other: &TlsRecord) -> Self {
        let p0 = (other.sender, other.sender_port);
        let p1 = (other.receiver, other.receiver_port);
        Self::from_pairs(p0, p1)
    }
}

/// Enum representing the different TLS record types.
///
/// Exact representation of [`ContentType`](TlsContentType) from rustls.
/// Rewriting it allows to implement more traits.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize, Serialize)]
pub enum MessageType {
    ChangeCipherSpec,
    Alert,
    Handshake,
    ApplicationData,
    Heartbeat,
    Unknown(u8),
}

impl From<TlsContentType> for MessageType {
    fn from(other: TlsContentType) -> Self {
        match other {
            TlsContentType::ChangeCipherSpec => MessageType::ChangeCipherSpec,
            TlsContentType::Alert => MessageType::Alert,
            TlsContentType::Handshake => MessageType::Handshake,
            TlsContentType::ApplicationData => MessageType::ApplicationData,
            TlsContentType::Heartbeat => MessageType::Heartbeat,
            TlsContentType::Unknown(u) => MessageType::Unknown(u),
        }
    }
}

/// Abstract representation of a TlsRecord within a pcap file
///
/// This contains the necessary information to build a [`Sequence`] and to map them back to the pcap for debugging using wireshark.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct TlsRecord {
    /// ID of the containing packet within the pcap
    ///
    /// Start at 1
    pub packet_in_pcap: u32,
    /// IPv4 Address of the sender
    pub sender: Ipv4Addr,
    /// TCP port of the sender
    pub sender_port: u16,
    /// IPv4 Address of the receiver
    pub receiver: Ipv4Addr,
    /// TCP port of the receiver
    pub receiver_port: u16,
    /// Time in Utc when the packet was captures
    pub time: NaiveDateTime,
    /// The TLS record type
    pub message_type: MessageType,
    /// Payload size of the TLS record
    pub message_length: u32,
}

impl Into<AbstractQueryResponse> for &TlsRecord {
    fn into(self) -> AbstractQueryResponse {
        AbstractQueryResponse {
            time: self.time,
            // Substract some overhead from the TLS encryption
            size: self.message_length - 40,
        }
    }
}

impl PartialOrd for TlsRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl Ord for TlsRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
    }
}

/// Extract a [`Sequence`] from the path to a pcap file.
///
/// Error conditions include unsupported pcaps, e.g., too fragmented records, or pcaps without DNS content.
pub fn pcap_to_sequence(
    file: impl AsRef<Path>,
    server: (Ipv4Addr, u16),
) -> Result<Sequence, Error> {
    let file = file.as_ref();
    let mut records = extract_tls_records(&file)?;
    records.values_mut().for_each(|records| {
        // `filter_tls_records` takes the Vec by value, which is why we first need to move it out
        // of the HashMap and back it afterwards.
        let mut tmp = Vec::new();
        mem::swap(records, &mut tmp);
        tmp = filter_tls_records(tmp, server);
        mem::swap(records, &mut tmp);
    });

    let seq = build_sequence(records, file.to_string_lossy());
    seq.ok_or_else(|| {
        format_err!(
            "No DNS communication found in the PCAP `{}`",
            file.display()
        )
    })
}

/// First step in processing a pcap file, extracting *all* Tls records
///
/// This extracts all Tls records from the pcap file, from both client and server.
pub fn extract_tls_records(
    file: impl AsRef<Path>,
) -> Result<HashMap<FlowId, Vec<TlsRecord>>, Error> {
    let file = file.as_ref();
    let mut capture = Capture::from_file(file)?;
    let datalink_type = capture.get_datalink();
    // ID of the packet with in the pcap file.
    // Makes it easier to map it to the same packet within wireshark
    let mut packet_id = 0;
    // List of all parsed TLS records
    let mut tls_records: HashMap<FlowId, Vec<TlsRecord>> = HashMap::default();
    // Buffer all unprocessed bytes.
    //
    // It needs to be a HashMap, because it needs to be stored per direction.
    let mut buffer_unprocessed: HashMap<FlowId, Vec<u8>> = HashMap::default();
    // The time value we will be using for the next successfully parsed TLS record.
    //
    // It needs to be a HashMap, because it needs to be stored per direction.
    //
    // We need to keep this in an extra variable here, that we can correctly process fragmented records.
    // Assume a TLS record r is split over the two TCP segments s1 and s2 with their arrival time being t1 and t2.
    // s2 contains another record r2.
    // We want the time values to match like:
    // r: t1
    // r2: t2
    // Therefore, we cannot update the time until after we successfully parsed r.
    let mut next_time: HashMap<FlowId, Option<NaiveDateTime>> = HashMap::default();

    (|| {
        'packet: loop {
            packet_id += 1;
            let pkt = capture.next();

            if pkt == Err(pcap::Error::NoMorePackets) {
                break;
            }

            // Start parsing all the layers of the packet
            let PcapPacket { header, data } = pkt?;
            if header.caplen != header.len {
                bail!("Cannot process packets, as they are truncated");
            }

            // Try extracting an IPv4 packet from the raw bytes we have
            // Linktypes are described here: https://www.tcpdump.org/linktypes.html
            let ipv4;
            let eth; // only for extending the lifetime
            match datalink_type {
                Linktype(1) => {
                    // Normal Ethernet
                    eth = EthernetPacket::new(data)
                        .ok_or_else(|| format_err!("Expect Ethernet Packet"))?;
                    ipv4 = Ipv4Packet::new(eth.payload())
                        // ipv4 = Ipv4Packet::new(eth.payload())
                        .ok_or_else(|| format_err!("Expect IPv4 Packet"))?;
                }
                Linktype(113) => {
                    // Linux cooked capture
                    // Used for capturing the `any` device
                    ipv4 = Ipv4Packet::new(&data[16..])
                        .ok_or_else(|| format_err!("Expect IPv4 Packet"))?;
                }
                _ => bail!(
                    "The datalink type is unknown and cannot be process: {} \"{}\"",
                    datalink_type
                        .get_name()
                        .unwrap_or_else(|_| format!("ID: {}", datalink_type.0)),
                    datalink_type
                        .get_description()
                        .unwrap_or_else(|_| "".to_string())
                ),
            }

            // Check for TCP in next layer
            if ipv4.get_next_level_protocol() != IpNextHeaderProtocols::Tcp {
                continue;
            }

            // Bit 0 => Reserved
            // Bit 1 => DF Don't Fragment
            // Bit 2 => MF More Fragments
            if (ipv4.get_flags() & 0b0000_0001) > 0 {
                bail!("Fragmented Packets are not supported")
            }

            let tcp =
                TcpPacket::new(ipv4.payload()).ok_or_else(|| format_err!("Expect TCP Segment"))?;

            let flowid = FlowId::from_ip_and_tcp(&ipv4, &tcp);

            // It seems that pnet 0.22 has a problem determining the TCP payload as it includes padding
            // See issue here: https://github.com/libpnet/libpnet/issues/346
            let tl = ipv4.get_total_length() as usize;
            let ihl = ipv4.get_header_length() as usize;
            let payload_length = tl - (ihl + tcp.get_data_offset() as usize) * 4;
            let payload = &tcp.payload()[0..payload_length];
            // Filter empty acknowledgements
            if payload.is_empty() {
                continue;
            }

            let buffer = buffer_unprocessed.entry(flowid).or_default();
            buffer.extend_from_slice(payload);

            // We only want to keep the next_time of the previous iteration, if we have a partially processed packet
            if buffer.is_empty() {
                next_time.remove(&flowid);
            }
            let time =
                NaiveDateTime::from_timestamp(header.ts.tv_sec, (header.ts.tv_usec * 1000) as u32);
            *next_time.entry(flowid).or_insert_with(|| Some(time)) = Some(time);

            debug!("({:>2}) Processing TCP segment", packet_id);

            while !buffer.is_empty() {
                let tls = match TlsMessage::read_bytes(&buffer) {
                    Some(tls) => tls,
                    // We cannot parse the packet yet, so just skip the processing
                    None => continue 'packet,
                };
                // Remove the bytes we already processed
                // The TLS header is 5 byte long and not included in the payload
                buffer.drain(..5 + tls.payload.length());
                debug!(
                    "{:?} {} - {}B",
                    tls.typ,
                    ipv4.get_source(),
                    tls.payload.length()
                );
                let record = TlsRecord {
                    packet_in_pcap: packet_id,
                    sender: ipv4.get_source(),
                    sender_port: tcp.get_source(),
                    receiver: ipv4.get_destination(),
                    receiver_port: tcp.get_destination(),
                    // next_time is never None here
                    time: next_time[&flowid].unwrap(),
                    message_type: tls.typ.into(),
                    message_length: tls.payload.length() as u32,
                };
                tls_records
                    .entry((&record).into())
                    .or_default()
                    .push(record);

                // Now that we build the TLS record, we can update the time
                next_time.insert(flowid, Some(time));
            }
        }
        Ok(())
    })()
    .with_context(|_| format!("Packet ID: {}", packet_id))?;

    Ok(tls_records)
}

/// Filter a list of TLS records and only return *interesting* ones
///
/// The interesting TLS records are those needed to build the feature set.
/// This means only those containing DNS traffic and maybe only the client or server.
pub fn filter_tls_records(
    records: Vec<TlsRecord>,
    (server, server_port): (Ipv4Addr, u16),
) -> Vec<TlsRecord> {
    let client_marker_query_size = 128 * 3;

    // First we ignore everything until we have seen the ChangeCipherSpec message from sever and client
    // This tells us that the initial unencrypted part of the handshake is done.
    //
    // Then we wait until the first message of the client.
    // The message needs to be at least 128 bytes to count.
    // For example, the client sends a finished message to finish the handshake which is smaller than 128B.
    //
    // From then on, only keep the server traffic.
    // This might need adapting later on.

    let mut has_seen_server_change_cipher_spec = false;
    let mut has_seen_client_change_cipher_spec = false;
    let mut has_seen_first_marker_query = false;
    let mut has_seen_second_marker_query = false;
    let mut records: Vec<_> = records
        .into_iter()
        .skip_while(|rec| {
            if rec.message_type == MessageType::ChangeCipherSpec {
                if rec.sender == server && rec.sender_port == server_port {
                    has_seen_server_change_cipher_spec = true;
                } else {
                    has_seen_client_change_cipher_spec = true;
                }
            }

            !(has_seen_server_change_cipher_spec && has_seen_client_change_cipher_spec)
        })
        .skip_while(|rec| {
            if !(rec.sender == server && rec.sender_port == server_port)
                && rec.message_length >= client_marker_query_size
            {
                has_seen_first_marker_query = true;
            }

            !has_seen_first_marker_query
        })
        // Skip the first marker query
        .skip(1)
        .take_while(|rec| {
            if !(rec.sender == server && rec.sender_port == server_port)
                && rec.message_length >= client_marker_query_size
            {
                has_seen_second_marker_query = true;
            }

            !has_seen_second_marker_query
        })
        .filter(|rec| rec.sender == server && rec.sender_port == server_port)
        // Skip both marker query responses
        .skip(2)
        .collect();
    // Remove the additional marker query to `end.example.`, which is at the very end before we stopped collecting.
    if !records.is_empty() {
        records.truncate(records.len() - 1);
    }
    records
}

/// Given a list of pre-filtered TLS records, build a [`Sequence`] with them
///
/// For filtering the list of [`TlsRecord`]s see the [`filter_tls_records`].
pub fn build_sequence<S, H>(
    records: HashMap<FlowId, Vec<TlsRecord>, H>,
    identifier: S,
) -> Option<Sequence>
where
    S: Into<String>,
{
    let records: Vec<_> = records
        .into_iter()
        .flat_map(|(_id, recs)| recs)
        .sorted()
        .collect();
    crate::convert_to_sequence(&records, identifier.into(), crate::LoadDnstapConfig::Normal)
}

/// Given a list of pre-filtered TLS records, build a [`PrecisionSequence`] with them
///
/// For filtering the list of [`TlsRecord`]s see the [`filter_tls_records`].
pub fn build_precision_sequence<S, H>(
    records: HashMap<FlowId, Vec<TlsRecord>, H>,
    identifier: S,
) -> Option<PrecisionSequence>
where
    S: Into<String>,
{
    let records: Vec<_> = records
        .into_iter()
        .flat_map(|(_id, recs)| recs)
        .sorted()
        .collect();
    crate::load_sequence::convert_to_precision_sequence(
        &records,
        identifier.into(),
    )
}

fn make_error(iter: impl IntoIterator<Item = SocketAddrV4>) -> String {
    let mut error =
        "Multiple server candidates found.\nSelect a server with -f/--filter:".to_string();
    for cand in iter {
        error += &format!("\n  {}", cand);
    }
    error
}

/// Perform all the steps to generate a [`Sequence`] from a pcap-file
pub fn load_pcap_file<P: AsRef<Path>>(
    file: P,
    filter: Option<SocketAddrV4>,
) -> Result<Sequence, Error> {
    load_pcap_file_real(file.as_ref(), filter, false, false)
}

/// Internally used function
#[doc(hidden)]
pub fn load_pcap_file_real(
    file: &Path,
    filter: Option<SocketAddrV4>,
    interative: bool,
    verbose: bool,
) -> Result<Sequence, Error> {
    let records = process_pcap(file, filter, interative, verbose)?;

    // Build final Sequence
    let seq = build_sequence(records, file.to_string_lossy());

    if interative {
        println!("\n{}", "Final DNS Sequence:".underline());
        if let Some(seq) = &seq {
            println!("{:?}", seq);
        }
        println!();
    }

    if let Some(seq) = seq {
        Ok(seq)
    } else {
        bail!("Could not build a sequence from the list of filtered records.")
    }
}

/// Internally used function
#[doc(hidden)]
pub fn process_pcap(
    file: &Path,
    mut filter: Option<SocketAddrV4>,
    interative: bool,
    verbose: bool,
) -> Result<HashMap<FlowId, Vec<TlsRecord>>, Error> {
    if interative {
        println!("{}{}", "Processing file: ".bold(), file.display());
    }

    // let file = "./tests/data/CF-constant-rate-400ms-2packets.pcap";
    let mut records = extract_tls_records(&file)?;

    // If verbose show all records in RON notation
    if verbose {
        println!(
            "{}\n{}\n",
            "List of all TLS records in RON notation:".underline(),
            ron::ser::to_string_pretty(
                &records,
                ron::ser::PrettyConfig {
                    enumerate_arrays: true,
                    ..Default::default()
                }
            )
            .unwrap()
        );
    }

    if filter.is_none() {
        filter = (|| -> Result<Option<SocketAddrV4>, Error> {
            // Try to guess what the sever might have been
            let endpoints: HashSet<_> = records
                .values()
                .flat_map(std::convert::identity)
                .map(|record| SocketAddrV4::new(record.sender, record.sender_port))
                .collect();

            // Check different ports I use for DoT
            let candidates: Vec<_> = endpoints
                .iter()
                .cloned()
                .filter(|sa| sa.port() == 853)
                .collect();
            if candidates.len() == 1 {
                return Ok(Some(candidates[0]));
            } else if candidates.len() > 1 {
                bail!(make_error(candidates))
            }

            let candidates: Vec<_> = endpoints
                .iter()
                .cloned()
                .filter(|sa| sa.port() == 8853)
                .collect();
            if candidates.len() == 1 {
                return Ok(Some(candidates[0]));
            } else if candidates.len() > 1 {
                bail!(make_error(candidates))
            }

            bail!(make_error(endpoints))
        })()?;
    }
    // Filter was set to Some() in the snippet above
    let filter = filter.unwrap();

    // Filter to only those records containing DNS
    records.values_mut().for_each(|records| {
        // `filter_tls_records` takes the Vec by value, which is why we first need to move it out
        // of the HashMap and back it afterwards.
        let mut tmp = Vec::new();
        mem::swap(records, &mut tmp);
        tmp = filter_tls_records(tmp, (*filter.ip(), filter.port()));
        mem::swap(records, &mut tmp);
    });
    if interative {
        println!("{}", "TLS Records with DNS responses:".underline());
        for r in records.values().flat_map(std::convert::identity).sorted() {
            println!("{:?}", r);
        }
    }

    Ok(records)
}
