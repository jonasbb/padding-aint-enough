use std::fmt::Debug;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Unknown error condition
    #[error("An unknown error occured.")]
    Unknown,
    /// Errors related to [`tokio::time`]
    #[error("Tokio Timer Error: {}", _0)]
    Timer(#[source] tokio::time::Error),
    /// Errors based on [`std::io`]
    #[error("{}: Kind: {:?}", _0, _1)]
    Io(#[source] std::io::Error, std::io::ErrorKind),
    /// Errors for parsing `ip:port` strings
    #[error("{}", _0)]
    AddrParseError(#[source] std::net::AddrParseError),
    /// Errors from parsing malformed DNS messages
    #[error("Invalid DNS message: {}", _0)]
    DnsParseError(#[source] trust_dns_proto::error::ProtoError),
    /// General [OpenSSL](openssl) errors
    #[error("OpenSSL error: {}", _0)]
    OpensslError(#[source] openssl::error::ErrorStack),
    /// Error specific to the `server` binary in how the remote endpoint is choosen.
    #[error(
         r#"No transport protocol can be inferred for port {}. The only recognized options are TCP on port 53 and TLS on port 853.

Please, specify the choice explicitly by using either --tcp or --tls."#,
        _0
    )]
    TransportNotInferable(u16),
    #[error("Tokio OpenSSL Handshake error: {}", _0)]
    TokioOpensslHandshakeError(String),
}

impl From<()> for Error {
    fn from(_error: ()) -> Self {
        Error::Unknown
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        let kind = error.kind();
        Error::Io(error, kind)
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

impl From<openssl::error::ErrorStack> for Error {
    fn from(error: openssl::error::ErrorStack) -> Self {
        Error::OpensslError(error)
    }
}

impl<S> From<tokio_openssl::HandshakeError<S>> for Error
where
    S: Debug,
{
    fn from(error: tokio_openssl::HandshakeError<S>) -> Self {
        Error::TokioOpensslHandshakeError(error.to_string())
    }
}
