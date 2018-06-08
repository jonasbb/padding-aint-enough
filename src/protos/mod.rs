pub mod dnstap;

use chrono::{DateTime, NaiveDateTime, Utc};
use failure::Error;
use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use trust_dns::op::Message as DnsMessage;
use trust_dns::rr::Name as DnsName;
use trust_dns::serialize::binary::BinDecodable;

#[derive(Clone, Debug)]
pub struct Dnstap {
    pub identity: Option<String>,
    pub version: Option<String>,
    pub extra: Option<Vec<u8>>,
    pub content: DnstapContent,
}

#[derive(Clone, Debug)]
pub enum DnstapContent {
    Message {
        message_type: dnstap::Message_Type,
        query_address: Option<IpAddr>,
        response_address: Option<IpAddr>,
        query_port: Option<u16>,
        response_port: Option<u16>,
        query_time: Option<DateTime<Utc>>,
        response_time: Option<DateTime<Utc>>,
        query_message: Option<(DnsMessage, usize)>,
        response_message: Option<(DnsMessage, usize)>,
        query_zone: Option<DnsName>,
    },
}

impl DnstapContent {
    fn convert_message(mut from: dnstap::Message) -> Result<DnstapContent, Error> {
        let message_type = from.get_field_type();
        let (query_address, response_address) = if !from.has_socket_family() {
            if from.has_query_address() || from.has_response_address() {
                bail!(
                    "Specifying a query or response address requires to specify the socket family."
                )
            }
            // nothing exists, so its fine
            (None, None)
        } else {
            // has socket family
            match from.get_socket_family() {
                dnstap::SocketFamily::INET => {
                    let q = if from.has_query_address() {
                        let q_bytes = from.take_query_address();
                        if q_bytes.len() != 4 {
                            bail!("An IPv4 address has to consists of exactly four bytes!")
                        }
                        Some(Ipv4Addr::from(*<&[u8; 4]>::try_from(&*q_bytes).unwrap()).into())
                    } else {
                        None
                    };
                    let r = if from.has_response_address() {
                        let r_bytes = from.take_response_address();
                        if r_bytes.len() != 4 {
                            bail!("An IPv4 address has to consists of exactly four bytes!")
                        }
                        Some(Ipv4Addr::from(*<&[u8; 4]>::try_from(&*r_bytes).unwrap()).into())
                    } else {
                        None
                    };
                    (q, r)
                }
                dnstap::SocketFamily::INET6 => {
                    let q = if from.has_query_address() {
                        let q_bytes = from.take_query_address();
                        if q_bytes.len() != 16 {
                            bail!("An IPv6 address has to consists of exactly 16 bytes!")
                        }
                        Some(Ipv6Addr::from(*<&[u8; 16]>::try_from(&*q_bytes).unwrap()).into())
                    } else {
                        None
                    };
                    let r = if from.has_response_address() {
                        let r_bytes = from.take_query_address();
                        if r_bytes.len() != 16 {
                            bail!("An IPv6 address has to consists of exactly 16 bytes!")
                        }
                        Some(Ipv6Addr::from(*<&[u8; 16]>::try_from(&*r_bytes).unwrap()).into())
                    } else {
                        None
                    };
                    (q, r)
                }
            }
        };
        let query_port = if from.has_query_port() {
            Some(from.get_query_port() as u16)
        } else {
            None
        };
        let response_port = if from.has_response_port() {
            Some(from.get_response_port() as u16)
        } else {
            None
        };
        let query_time = if from.has_query_time_sec() {
            let q_sec = from.get_query_time_sec();
            let q_nsec = if from.has_query_time_nsec() {
                from.get_query_time_nsec()
            } else {
                0
            };
            let ndt = NaiveDateTime::from_timestamp_opt(q_sec as i64, q_nsec);
            if let Some(ndt) = ndt {
                Some(DateTime::<Utc>::from_utc(ndt, Utc))
            } else {
                bail!(
                    "Query Time: Invalid or out of range value for DateTime: {}.{} s",
                    q_sec,
                    q_nsec
                )
            }
        } else {
            None
        };
        let response_time = if from.has_response_time_sec() {
            let r_sec = from.get_response_time_sec();
            let r_nsec = if from.has_response_time_nsec() {
                from.get_response_time_nsec()
            } else {
                0
            };
            let ndt = NaiveDateTime::from_timestamp_opt(r_sec as i64, r_nsec);
            if let Some(ndt) = ndt {
                Some(DateTime::<Utc>::from_utc(ndt, Utc))
            } else {
                bail!(
                    "Response Time: Invalid or out of range value for DateTime: {}.{} s",
                    r_sec,
                    r_nsec
                )
            }
        } else {
            None
        };
        let query_zone = if from.has_query_zone() {
            Some(
                DnsName::from_bytes(&*from.take_query_zone())
                    .map_err(|err| format_err!("Processing the query zone failed: {}", err))?,
            )
        } else {
            None
        };
        let query_message = if from.has_query_message() {
            let buf = from.take_query_message();
            Some((
                DnsMessage::from_vec(&*buf)
                    .map_err(|err| format_err!("Processing the query message failed: {}", err))?,
                buf.len(),
            ))
        } else {
            None
        };
        let response_message = if from.has_response_message() {
            let buf = from.take_response_message();
            Some((
                DnsMessage::from_vec(&*buf)
                    .map_err(|err| format_err!("Processing the response message failed: {}", err))?,
                buf.len(),
            ))
        } else {
            None
        };

        Ok(DnstapContent::Message {
            message_type,
            query_address,
            response_address,
            query_port,
            response_port,
            query_time,
            response_time,
            query_message,
            response_message,
            query_zone,
        })
    }
}

impl TryFrom<dnstap::Dnstap> for Dnstap {
    type Error = Error;

    fn try_from(mut from: dnstap::Dnstap) -> Result<Self, Error> {
        let identity = if from.has_identity() {
            Some(String::from_utf8(from.take_identity())?)
        } else {
            None
        };
        let version = if from.has_version() {
            Some(String::from_utf8(from.take_version())?)
        } else {
            None
        };
        let extra = if from.has_extra() {
            Some(from.take_extra())
        } else {
            None
        };

        if !from.has_field_type() {
            panic!("Need to have a field type!")
        }

        let content = match from.get_field_type() {
            dnstap::Dnstap_Type::MESSAGE => DnstapContent::convert_message(from.take_message())?,
        };

        Ok(Dnstap {
            identity,
            version,
            extra,
            content,
        })
    }
}
