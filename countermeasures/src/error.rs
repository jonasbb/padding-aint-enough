#[derive(Debug)]
pub enum Error {
    Unit,
    Timer(tokio_timer::Error),
    Io(std::io::Error),
    AddrParseError(std::net::AddrParseError),
    DnsParseError(trust_dns_proto::error::ProtoError),
}

impl From<tokio_timer::Error> for Error {
    fn from(error: tokio_timer::Error) -> Self {
        Error::Timer(error)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<()> for Error {
    fn from(_error: ()) -> Self {
        Error::Unit
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(error: std::net::AddrParseError) -> Self {
        Error::AddrParseError(error)
    }
}

impl From<trust_dns_proto::error::ProtoError> for Error {
    fn from(error: trust_dns_proto::error::ProtoError) -> Self {
        Error::DnsParseError(error)
    }
}
