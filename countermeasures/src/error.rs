use failure::{Backtrace, Fail};
use std::fmt::Debug;

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
    #[fail(display = "OpenSSL error: {}", _0)]
    OpensslError(#[fail(cause)] openssl::error::ErrorStack, Backtrace),
    #[fail(display = "OpenSSL Handshake error: {}", _0)]
    OpensslHandshakeError(String, Backtrace),
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

impl From<openssl::error::ErrorStack> for Error {
    fn from(error: openssl::error::ErrorStack) -> Self {
        Error::OpensslError(error, Backtrace::default())
    }
}

impl<S> From<openssl::ssl::HandshakeError<S>> for Error
where
    S: Debug,
{
    fn from(error: openssl::ssl::HandshakeError<S>) -> Self {
        use openssl::ssl::HandshakeError::*;
        use Error::*;

        match error {
            SetupFailure(error_stack) => Self::from(error_stack),
            f @ Failure(_) => OpensslHandshakeError(f.to_string(), Backtrace::default()),
            WouldBlock(_) => panic!("The HandshakeError::WouldBlock must always be handled before reaching this function."),
        }
    }
}
