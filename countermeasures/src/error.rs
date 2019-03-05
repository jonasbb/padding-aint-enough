use failure::{Backtrace, Fail};

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "An unknown error occured.")]
    Unknown(Backtrace),
    #[fail(display = "Tokio Timer Error: {}", _0)]
    Timer(#[fail(cause)] tokio_timer::Error, Backtrace),
    #[fail(display = "{}", _0)]
    Io(#[fail(cause)] std::io::Error, Backtrace),
    #[fail(display = "{}", _0)]
    AddrParseError(#[fail(cause)] std::net::AddrParseError, Backtrace),
    #[fail(display = "Invalid DNS message: {}", _0)]
    DnsParseError(#[fail(cause)] trust_dns_proto::error::ProtoError, Backtrace),
    #[fail(display = "TLS error: {}", _0)]
    TlsError(#[fail(cause)] rustls::TLSError, Backtrace),
}

impl From<()> for Error {
    fn from(_error: ()) -> Self {
        Error::Unknown(Backtrace::default())
    }
}

impl From<tokio_timer::Error> for Error {
    fn from(error: tokio_timer::Error) -> Self {
        Error::Timer(error, Backtrace::default())
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error, Backtrace::default())
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(error: std::net::AddrParseError) -> Self {
        Error::AddrParseError(error, Backtrace::default())
    }
}

impl From<trust_dns_proto::error::ProtoError> for Error {
    fn from(error: trust_dns_proto::error::ProtoError) -> Self {
        Error::DnsParseError(error, Backtrace::default())
    }
}

impl From<rustls::TLSError> for Error {
    fn from(error: rustls::TLSError) -> Self {
        Error::TlsError(error, Backtrace::default())
    }
}
