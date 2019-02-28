use chrono::NaiveDateTime;
use failure::{bail, format_err, Error, ResultExt};
use log::debug;
use pcap::{Capture, Packet as PcapPacket};
use pnet::packet::{ethernet::EthernetPacket, ipv4::Ipv4Packet, tcp::TcpPacket, Packet};
use rustls::internal::msgs::{
    codec::Codec, enums::ContentType as TlsContentType, message::Message as TlsMessage,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::Ipv4Addr, path::Path};

type FlowId = (Ipv4Addr, u16);

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

#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct TlsRecord {
    pub packet_in_pcap: u32,
    pub sender: Ipv4Addr,
    pub port: u16,
    pub time: NaiveDateTime,
    pub message_type: MessageType,
    pub message_length: u32,
}

pub fn extract_tls_records(file: impl AsRef<Path>) -> Result<Vec<TlsRecord>, Error> {
    let file = file.as_ref();
    let mut capture = Capture::from_file(file)?;
    // ID of the packet with in the pcap file.
    // Makes it easier to map it to the same packet within wireshark
    let mut packet_id = 0;
    // List of all parsed TLS records
    let mut tls_records = Vec::default();
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

            let eth =
                EthernetPacket::new(data).ok_or_else(|| format_err!("Expect Ethernet Packet"))?;

            let ipv4 =
                Ipv4Packet::new(eth.payload()).ok_or_else(|| format_err!("Expect IPv4 Packet"))?;
            // Bit 0 => Reserved
            // Bit 1 => DF Don't Fragment
            // Bit 2 => MF More Fragments
            if (ipv4.get_flags() & 0b0000_0001) > 0 {
                bail!("Fragmented Packets are not supported")
            }

            let tcp =
                TcpPacket::new(ipv4.payload()).ok_or_else(|| format_err!("Expect TCP Segment"))?;

            let flowid = (ipv4.get_source(), tcp.get_source());

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
                tls_records.push(TlsRecord {
                    packet_in_pcap: packet_id,
                    sender: ipv4.get_source(),
                    port: tcp.get_source(),
                    // next_time is never None here
                    time: next_time[&flowid].unwrap(),
                    message_type: tls.typ.into(),
                    message_length: tls.payload.length() as u32,
                });

                // Now that we build the TLS record, we can update the time
                next_time.insert(flowid, Some(time));
            }
        }
        Ok(())
    })()
    .with_context(|_| format!("Packet ID: {}", packet_id))?;

    Ok(tls_records)
}

pub fn filter_tls_records(records: Vec<TlsRecord>) -> Vec<TlsRecord> {
    let server = Ipv4Addr::new(1, 1, 1, 1);
    let server_port = 853;
    let min_client_query_size = 128;

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
    let mut has_seen_first_client_query = false;
    records
        .into_iter()
        .skip_while(|rec| {
            if rec.message_type == MessageType::ChangeCipherSpec {
                if rec.sender == server && rec.port == server_port {
                    has_seen_server_change_cipher_spec = true;
                } else {
                    has_seen_client_change_cipher_spec = true;
                }
            }

            !(has_seen_server_change_cipher_spec && has_seen_client_change_cipher_spec)
        })
        .skip_while(|rec| {
            if !(rec.sender == server && rec.port == server_port)
                && rec.message_length >= min_client_query_size
            {
                has_seen_first_client_query = true;
            }

            !has_seen_first_client_query
        })
        .filter(|rec| rec.sender == server && rec.port == server_port)
        .collect()
}
