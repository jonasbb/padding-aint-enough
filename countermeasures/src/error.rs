use failure::{Backtrace, Fail};
use std::fmt::Debug;

#[derive(Debug, Fail)]
pub enum Error {
    /// Unknown error condition
    #[fail(display = "An unknown error occured.")]
    Unknown(Backtrace),
    /// Errors related to [`tokio_timer`]
    #[fail(display = "Tokio Timer Error: {}", _0)]
    Timer(#[fail(cause)] tokio_timer::Error, Backtrace),
    /// Errors based on [`std::io`]
    #[fail(display = "{}", _0)]
    Io(#[fail(cause)] std::io::Error, Backtrace),
    /// Errors for parsing `ip:port` strings
    #[fail(display = "{}", _0)]
    AddrParseError(#[fail(cause)] std::net::AddrParseError, Backtrace),
    /// Errors from parsing malformed DNS messages
    #[fail(display = "Invalid DNS message: {}", _0)]
    DnsParseError(#[fail(cause)] trust_dns_proto::error::ProtoError, Backtrace),
    /*
    #[fail(display = "TLS error: {}", _0)]
    TlsError(#[fail(cause)] rustls::TLSError, Backtrace),
    */
    /// General [OpenSSL](openssl) errors
    #[fail(display = "OpenSSL error: {}", _0)]
    OpensslError(#[fail(cause)] openssl::error::ErrorStack, Backtrace),
    /// [OpenSSL](openssl) errors specific to the handshake. Converted from [`openssl::ssl::HandshakeError`]
    ///
    /// This error does not contain the [`HandshakeError`] as this would introduce a generic type parameter
    ///
    /// [`HandshakeError`]: openssl::ssl::HandshakeError
    #[fail(display = "OpenSSL Handshake error: {}", _0)]
    OpensslHandshakeError(String, Backtrace),
    /// Error specific to the `server` binary in how the remote endpoint is choosen.
    #[fail(
        display = r#"No transport protocol can be inferred for port {}. The only recognized options are TCP on port 53 and TLS on port 853.

Please, specify the choice explicitly by using either --tcp or --tls."#,
        _0
    )]
    TransportNotInferable(u16, Backtrace),
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

/*
impl From<rustls::TLSError> for Error {
    fn from(error: rustls::TLSError) -> Self {
        Error::TlsError(error, Backtrace::default())
    }
}
*/

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
