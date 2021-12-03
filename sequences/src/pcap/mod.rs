//! Parsing PCAPs and extracting DNS Sequences from them
//!
//! The module has two entry point which does the pcap parsing and returns a sequence.
//! These are the [`build_sequence`] and [`build_precision_sequence`] functions.
//!
//! Internally three main steps are performed:
//!
//! 1. Extract all TLS records from the pcap file: [`extract_tls_records`].
//! 2. Then filter out all records which are not interesting in our case: [`filter_tls_records`].
//!
//!     This are the records containing the TLS certificates or other meta-information which is not DNS traffic.
//!     This relies on either a manually specified filter (IP + Port) to identify which flow contains the DNS traffic,
//!     or it uses the [`guess_dns_flow_identifier`] function to guess based on IP or port information.
//! 3. The extracted size and time information are converted into a sequence using [`crate::convert_to_sequence`].
//!
//! Steps 1 and 2 are combined in a single [`extract_and_filter_tls_records_from_file`], such that it can be shared
//! for both [`build_sequence`]/[`build_precision_sequence`] functions.

mod bounded_buffer;
mod tcp_buffer;

use self::{bounded_buffer::BoundedBuffer, tcp_buffer::TcpBuffer};
use crate::{AbstractQueryResponse, LoadSequenceConfig, PrecisionSequence, Sequence};
use anyhow::{anyhow, bail, Context as _, Error};
use chrono::NaiveDateTime;
use etherparse::{InternetSlice, Ipv4HeaderSlice, SlicedPacket, TcpHeaderSlice, TransportSlice};
use itertools::Itertools;
use log::{debug, trace};
use misc_utils::fs;
use pcap_parser::{data::PacketData, PcapCapture, PcapError};
use rustls::{
    internal::msgs::{
        codec::Reader,
        enums::ContentType as TlsContentType,
        handshake::{
            HandshakePayload as TlsHandshakePayload, ServerExtension as TlsServerExtensions,
        },
        message::{MessagePayload as TlsMessagePayload, OpaqueMessage as OpaqueTlsMessage},
    },
    ProtocolVersion,
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    mem,
    net::{Ipv4Addr, SocketAddrV4},
    path::Path,
};

/// Identifier for a one-way TCP flow
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct FlowIdentifier {
    pub source_ip: Ipv4Addr,
    pub source_port: u16,
    pub destination_ip: Ipv4Addr,
    pub destination_port: u16,
}

impl FlowIdentifier {
    pub fn from_pairs(source: (Ipv4Addr, u16), destination: (Ipv4Addr, u16)) -> Self {
        Self {
            source_ip: source.0,
            source_port: source.1,
            destination_ip: destination.0,
            destination_port: destination.1,
        }
    }

    pub fn from_ip_and_tcp<'a, 'b>(ipv4: &Ipv4HeaderSlice<'a>, tcp: &TcpHeaderSlice<'b>) -> Self {
        let source = (ipv4.source_addr(), tcp.source_port());
        let destination = (ipv4.destination_addr(), tcp.destination_port());
        Self::from_pairs(source, destination)
    }
}

impl From<&TlsRecord> for FlowIdentifier {
    fn from(other: &TlsRecord) -> Self {
        let source = (other.sender, other.sender_port);
        let destination = (other.receiver, other.receiver_port);
        Self::from_pairs(source, destination)
    }
}

/// Identifier for a two-way TCP flow
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct TwoWayFlowIdentifier(FlowIdentifier);

impl From<FlowIdentifier> for TwoWayFlowIdentifier {
    fn from(other: FlowIdentifier) -> Self {
        let p0 = (other.source_ip, other.source_port);
        let p1 = (other.destination_ip, other.destination_port);
        let ((source_ip, source_port), (destination_ip, destination_port)) =
            if p0 <= p1 { (p0, p1) } else { (p1, p0) };
        Self(FlowIdentifier {
            source_ip,
            source_port,
            destination_ip,
            destination_port,
        })
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

/// Different TLS versions we support
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize, Serialize)]
pub enum TlsVersion {
    Tls1_2,
    Tls1_3,
    Unknown,
}

impl From<ProtocolVersion> for TlsVersion {
    fn from(version: ProtocolVersion) -> Self {
        match version {
            ProtocolVersion::TLSv1_2 => Self::Tls1_2,
            ProtocolVersion::TLSv1_3 => Self::Tls1_3,
            _ => Self::Unknown,
        }
    }
}

impl From<&ProtocolVersion> for TlsVersion {
    fn from(version: &ProtocolVersion) -> Self {
        Self::from(*version)
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
    /// TLS version choosen by the server, if this is the ServerHello handshake message
    pub tls_version: Option<TlsVersion>,
}

impl From<&TlsRecord> for AbstractQueryResponse {
    fn from(tls: &TlsRecord) -> Self {
        Self {
            time: tls.time,
            // Substract some overhead from the TLS encryption
            size: tls.message_length - 40,
        }
    }
}

impl PartialOrd for TlsRecord {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TlsRecord {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time)
    }
}

/// First step in processing a pcap file, extracting *all* Tls records
///
/// This extracts all Tls records from the pcap file, from both client and server.
fn extract_tls_records(
    file: impl AsRef<Path>,
) -> Result<HashMap<TwoWayFlowIdentifier, Vec<TlsRecord>>, Error> {
    let file_content = fs::read(file)?;
    let capture = PcapCapture::from_file(&file_content).map_err(|err| match err {
        PcapError::Eof => anyhow!("Failed reading pcap: EOF"),
        PcapError::ReadError => anyhow!("Failed reading pcap: Read error"),
        PcapError::Incomplete => anyhow!("Failed reading pcap: Incomplete"),
        PcapError::HeaderNotRecognized => anyhow!("Failed reading pcap: Header not recognized"),
        PcapError::NomError(_, kind) | PcapError::OwnedNomError(_, kind) => {
            anyhow!("Failed reading pcap: Nom Error: {:?}", kind)
        }
    })?;
    let datalink_type = capture.header.network;
    // ID of the packet with in the pcap file.
    // Makes it easier to map it to the same packet within wireshark
    let mut packet_id = 0;
    // List of all parsed TLS records
    let mut tls_records: HashMap<TwoWayFlowIdentifier, Vec<TlsRecord>> = HashMap::default();
    // Buffer all unprocessed bytes.
    //
    // It needs to be a HashMap, because it needs to be stored per direction.
    let mut buffer_unprocessed: HashMap<FlowIdentifier, TcpBuffer> = HashMap::default();
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
    let mut next_time: HashMap<FlowIdentifier, Option<NaiveDateTime>> = HashMap::default();
    // Keep a list of flowids and processed sequence numbers to be able to detect retransmissions
    let mut seen_sequences: BoundedBuffer<(FlowIdentifier, u32)> = BoundedBuffer::new(30);

    (|| {
        'packet: for (id, pkt) in capture.blocks.into_iter().enumerate() {
            packet_id = id as u32 + 1;
            if pkt.caplen != pkt.origlen {
                bail!("Cannot process packets, as they are truncated");
            }

            // Try extracting an IPv4 packet from the raw bytes we have
            // Linktypes are described here: https://www.tcpdump.org/linktypes.html
            let parsed_packet;
            let ipv4;
            let tcp;
            match pcap_parser::data::get_packetdata(pkt.data, datalink_type, pkt.caplen as usize) {
                None => bail!("Could not parse the packet data of packet_id {}", packet_id),
                Some(PacketData::Unsupported(_)) | Some(PacketData::L4(_, _)) => {
                    bail!("Unsupported linktype {}", datalink_type)
                }
                Some(PacketData::L2(data)) => {
                    // Normal Ethernet captures
                    parsed_packet =
                        SlicedPacket::from_ethernet(data).map_err(|err| anyhow!("{:?}", err))?;
                }
                Some(PacketData::L3(_, data)) => {
                    // Linux cooked capture
                    // Used for capturing the `any` device
                    parsed_packet =
                        SlicedPacket::from_ip(data).map_err(|err| anyhow!("{:?}", err))?;
                }
            };
            if let Some(InternetSlice::Ipv4(inner, _)) = parsed_packet.ip {
                ipv4 = inner;
            } else {
                bail!("Could not find an IPv4 packet for packet_id: {}", packet_id);
            }

            // Only process TCP packets, skip rest
            if let Some(TransportSlice::Tcp(inner)) = parsed_packet.transport {
                tcp = inner;
            } else {
                continue;
            }

            // Filter empty acknowledgements
            if parsed_packet.payload.is_empty() {
                continue;
            }

            // Bit 0 => Reserved
            // Bit 1 => DF Don't Fragment
            // Bit 2 => MF More Fragments
            if ipv4.more_fragments() {
                bail!("Fragmented Packets are not supported")
            }

            let flowid = FlowIdentifier::from_ip_and_tcp(&ipv4, &tcp);

            // We only want to keep unique entries and filter out all retransmissions
            if !seen_sequences.add((flowid, tcp.sequence_number())) {
                // This is a retransmission, so do not process it
                continue;
            }

            let buffer = buffer_unprocessed.entry(flowid).or_default();
            buffer.add_data(tcp.sequence_number(), parsed_packet.payload);

            // We only want to keep the next_time of the previous iteration, if we have a partially processed packet
            if buffer.is_empty() {
                next_time.remove(&flowid);
            }
            let time =
                NaiveDateTime::from_timestamp(i64::from(pkt.ts_sec), (pkt.ts_usec * 1000) as u32);
            *next_time.entry(flowid).or_insert_with(|| Some(time)) = Some(time);

            debug!("({:>2}) Processing TCP segment", packet_id);

            while !buffer.is_empty() {
                let tls = match OpaqueTlsMessage::read(&mut Reader::init(buffer.view_data())) {
                    Ok(tls) => tls,
                    // We cannot parse the packet yet, so just skip the processing
                    Err(_) => continue 'packet,
                };
                // Remove the bytes we already processed
                // The TLS header is 5 byte long and not included in the payload
                buffer.consume(5 + tls.payload.0.len())?;
                debug!(
                    "{:?} {} - {}B",
                    tls.typ,
                    ipv4.source_addr(),
                    tls.payload.0.len()
                );

                let mut tls_version = None;

                // See if this is a server send ServerHello with a version
                if let Ok(TlsMessagePayload::Handshake(handshake_payload)) =
                    TlsMessagePayload::new(tls.typ, tls.version, tls.payload.clone())
                {
                    if let TlsHandshakePayload::ServerHello(server_hello) =
                        handshake_payload.payload
                    {
                        let mut min_version = server_hello.legacy_version.into();
                        for ext in &server_hello.extensions {
                            if let TlsServerExtensions::SupportedVersions(vers) = ext {
                                let vers = vers.into();
                                if vers > min_version {
                                    min_version = vers;
                                }
                            }
                        }
                        tls_version = Some(min_version);
                    }
                };
                let record = TlsRecord {
                    packet_in_pcap: packet_id,
                    sender: ipv4.source_addr(),
                    sender_port: tcp.source_port(),
                    receiver: ipv4.destination_addr(),
                    receiver_port: tcp.destination_port(),
                    // next_time is never None here
                    time: next_time[&flowid].unwrap(),
                    message_type: tls.typ.into(),
                    message_length: tls.payload.0.len() as u32,
                    tls_version,
                };
                tls_records.entry(flowid.into()).or_default().push(record);

                // Now that we build the TLS record, we can update the time
                next_time.insert(flowid, Some(time));
            }
        }
        Ok(())
    })()
    .with_context(|| format!("Packet ID: {}", packet_id))?;

    Ok(tls_records)
}

/// Filter a list of TLS records and only return *interesting* ones
///
/// The interesting TLS records are those needed to build the feature set.
/// This means only those containing DNS traffic and maybe only the client or server.
fn filter_tls_records(
    records: Vec<TlsRecord>,
    (server, server_port): (Ipv4Addr, u16),
) -> Vec<TlsRecord> {
    let base_message_size = 128;
    let client_marker_query_size = 128 * 3;

    // First we ignore everything until we have seen the ChangeCipherSpec message from sever and client
    // This tells us that the initial unencrypted part of the handshake is done.
    //
    // Then we wait for the transmission of the aaa.aaa.aaa.aaa query and the corresponding response.
    // They can both be recognized by their large size, of at least 3*128 bytes (255 Qname + header overhead + block padding).
    // Afterwards, we check for a small query, the `start.example` one.
    // The response to this query we want to keep in the data.
    //
    // From then on, only keep the server traffic.
    // This might need adapting later on.

    trace!("Filter TLS Server: {} {}", server, server_port);
    let mut tls_version = None;
    let mut has_seen_server_change_cipher_spec = false;
    let mut has_seen_client_change_cipher_spec = false;
    let mut has_seen_large_marker_query = false;
    let mut has_seen_start_marker_query = false;
    let mut has_seen_end_marker_query = false;
    let mut records: Vec<_> = records
        .into_iter()
        .inspect(|rec| {
            if rec.tls_version.is_some() {
                tls_version = rec.tls_version;
            }
        })
        .skip_while(|rec| {
            if rec.message_type == MessageType::ChangeCipherSpec {
                if rec.sender == server && rec.sender_port == server_port {
                    has_seen_server_change_cipher_spec = true;
                } else {
                    has_seen_client_change_cipher_spec = true;
                }
            }

            if has_seen_server_change_cipher_spec && has_seen_client_change_cipher_spec {
                trace!("Second ChangeCipherSpec seen in ID: {}", rec.packet_in_pcap);
            }
            !(has_seen_server_change_cipher_spec && has_seen_client_change_cipher_spec)
        })
        // Filter for the large marker query aaa.aaa.aaa.aaa
        .skip_while(|rec| {
            if rec.sender == server
                && rec.sender_port == server_port
                && rec.message_length >= client_marker_query_size
            {
                has_seen_large_marker_query = true;
            }

            if has_seen_large_marker_query {
                trace!("Marker Query (large) seen in ID: {}", rec.packet_in_pcap);
            }
            !has_seen_large_marker_query
        })
        // Now we wait for the next message from the client, which is a small `start.example.`
        .skip_while(|rec| {
            if rec.receiver == server
                && rec.receiver_port == server_port
                && rec.message_length >= base_message_size
                && rec.message_length <= 2 * base_message_size
            {
                has_seen_start_marker_query = true;
            }

            if has_seen_start_marker_query {
                trace!("Marker Query (start) seen in ID: {}", rec.packet_in_pcap);
            }
            !has_seen_start_marker_query
        })
        .take_while(|rec| {
            if !(rec.sender == server && rec.sender_port == server_port)
                && rec.message_length >= client_marker_query_size
            {
                has_seen_end_marker_query = true;
            }

            !has_seen_end_marker_query
        })
        // Only keep the server replies
        .filter(|rec| rec.sender == server && rec.sender_port == server_port)
        // Only keep `Application Data` entries
        .filter(|rec| rec.message_type == MessageType::ApplicationData)
        // Skip the start marker query responses
        .skip(1)
        .collect();

    // if the connection is build using TLSv1.2 the messages are not necessarily padded to 128 bytes
    // Instead they are unpadded.
    // We need to keep this in mind while filtering for message sizes here
    // Only keep messages which are large enough to contain DNS
    if tls_version == Some(TlsVersion::Tls1_3) {
        records.retain(|rec| rec.message_length >= base_message_size);
    }

    if has_seen_end_marker_query {
        // Remove the additional marker query to `end.example.`,
        // which is at the very end before we stopped collecting,
        // but only if we are sure we observed it.
        // The last part is important as sometimes the `end.example.` and
        // zzz.zzz.zzz.zzz queries are part of a new TCP session due to timeouts.
        records.truncate(records.len().saturating_sub(1));
    }
    records
}

/// Perform all the steps to generate a [`Sequence`] from a pcap-file
pub fn build_sequence(
    file: &Path,
    filter: Option<SocketAddrV4>,
    verbose: bool,
    config: LoadSequenceConfig,
) -> Result<Sequence, Error> {
    let records = extract_and_filter_tls_records_from_file(file, filter, verbose)?;
    let records: Vec<_> = records
        .into_iter()
        .flat_map(|(_id, recs)| recs)
        .sorted()
        .collect();
    crate::convert_to_sequence(&records, file.to_string_lossy().to_string(), config).ok_or_else(
        || {
            anyhow!(
                "Could not build Sequence from extracted TLS records for file {}",
                file.display()
            )
        },
    )
}

/// Perform all the steps to generate a [`PrecisionSequence`] from a pcap-file
pub fn build_precision_sequence(
    file: &Path,
    filter: Option<SocketAddrV4>,
    verbose: bool,
) -> Result<PrecisionSequence, Error> {
    let records = extract_and_filter_tls_records_from_file(file, filter, verbose)?;
    let records: Vec<_> = records
        .into_iter()
        .flat_map(|(_id, recs)| recs)
        .sorted()
        .collect();
    crate::load_sequence::convert_to_precision_sequence(
        &records,
        file.to_string_lossy().to_string(),
    )
    .ok_or_else(|| {
        anyhow!(
            "Could not build PrecisionSequence from extracted TLS records for file {}",
            file.display()
        )
    })
}

/// Extract TLS records from a file and filter them to only contain DNS entries
fn extract_and_filter_tls_records_from_file(
    file: &Path,
    mut filter: Option<SocketAddrV4>,
    verbose: bool,
) -> Result<HashMap<TwoWayFlowIdentifier, Vec<TlsRecord>>, Error> {
    // Extract TLS records
    let mut records = extract_tls_records(&file)?;
    trace!("Extracted TLS Recrods:\n{:#?}", records);

    // Guess which connection contains the DNS flow if not manually specified
    if filter.is_none() {
        filter = Some(guess_dns_flow_identifier(&records)?);
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

    trace!("Extracted Flows:\n{:#?}", records);
    if verbose {
        let records: Vec<_> = records.values().flatten().sorted().collect();
        eprintln!("{}", serde_json::to_string_pretty(&records).unwrap());
    }

    Ok(records)

    // // Build final Sequence
    // let seq = build_sequence(records, file.to_string_lossy(), config);

    // if let Some(seq) = seq {
    //     Ok(seq)
    // } else {
    //     bail!("Could not build a sequence from the list of filtered records.")
    // }
}

/// Guess which of the flows contains DNS data
///
/// Returns a result if a single flow could be identified.
/// Returns an error if either no endpoints exist or multiple candidates exist.
fn guess_dns_flow_identifier(
    records: &HashMap<TwoWayFlowIdentifier, Vec<TlsRecord>>,
) -> Result<SocketAddrV4, Error> {
    /// Create a error description if multiple filter candidates are found
    fn make_error(iter: impl IntoIterator<Item = SocketAddrV4>) -> String {
        let mut error =
            "Multiple server candidates found.\nSelect a server with -f/--filter:".to_string();
        for cand in iter {
            error += &format!("\n  {}", cand);
        }
        error
    }

    // Try to guess what the sever might have been
    let endpoints: HashSet<_> = records
        .values()
        .flatten()
        .map(|record| SocketAddrV4::new(record.sender, record.sender_port))
        .collect();

    // Check different ports I use for DoT
    let candidates: Vec<_> = endpoints
        .iter()
        .cloned()
        .filter(|sa| sa.port() == 853)
        .collect();
    match candidates.len() {
        0 => {}
        1 => return Ok(candidates[0]),
        _ => bail!(make_error(candidates)),
    }

    let candidates: Vec<_> = endpoints
        .iter()
        .cloned()
        .filter(|sa| sa.port() == 8853)
        .collect();
    match candidates.len() {
        0 => {}
        1 => return Ok(candidates[0]),
        _ => bail!(make_error(candidates)),
    }

    bail!(make_error(endpoints))
}
