#![deny(rust_2018_compatibility)]
#![warn(rust_2018_idioms)]
// enable the await! macro, async support, and the new std::Futures api.
#![feature(await_macro, async_await, futures_api)]
// // only needed to manually implement a std future:
// #![feature(arbitrary_self_types)]

mod adaptive_padding;
mod constant_rate;
mod dns_tcp;
mod error;
pub mod utils;

pub use crate::{
    adaptive_padding::AdaptivePadding, constant_rate::ConstantRate, dns_tcp::DnsBytesStream,
    error::Error,
};
use log::{error, warn};
use rustls::Session;
use std::{
    fmt::Debug,
    fs::OpenOptions,
    io::{self, Read, Write},
    net::Shutdown,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{await, net::TcpStream, prelude::*};
use tokio_rustls::TlsStream;

/// Parse a string as [`u64`], interpret it as milliseconds, and return a [`Duration`]
pub fn parse_duration_ms(s: &str) -> Result<Duration, std::num::ParseIntError> {
    let ms: u64 = s.parse()?;
    Ok(Duration::from_millis(ms))
}

#[allow(dead_code)]
type OpensslKeylogCallback = dyn Fn(&openssl::ssl::SslRef, &str) + 'static + Sync + Send;

pub fn keylog_to_stderr(_ssl: &openssl::ssl::SslRef, line: &str) {
    eprintln!("{}", line);
}

pub fn keylog_to_file<P>(file: P) -> impl Fn(&openssl::ssl::SslRef, &str) + 'static + Sync + Send
where
    P: AsRef<Path>,
{
    let path = file.as_ref().to_path_buf();
    let file = match OpenOptions::new().append(true).create(true).open(&path) {
        Ok(f) => Some(f),
        Err(e) => {
            warn!("unable to create key log file '{:?}': {}", path, e);
            None
        }
    }
    // TODO replace with better error handling
    .unwrap_or_else(|| panic!("Could not open SSLKEYLOGILE {}", path.display()));

    // Allow the closure to be Fn instead of only FnMut
    let file = Mutex::from(file);
    move |_ssl, line| {
        let mut file = file.lock().unwrap();
        if let Err(err) = writeln!(file, "{}", line) {
            error!(
                "Could not write to SSLKEYLOGFILE {}: {}",
                path.display(),
                err
            );
        }
    }
}

#[test]
fn test_function_has_correct_type() {
    fn require_type<T: ?Sized>(_: &T) {};

    require_type::<OpensslKeylogCallback>(&keylog_to_stderr);
    require_type::<OpensslKeylogCallback>(&keylog_to_file("/dev/null"));
}

/// Log all errors produces by the future and discard the ok-value
pub async fn print_error<F, T, E>(future: F)
where
    F: std::future::Future<Output = Result<T, E>>,
    E: Debug,
{
    if let Err(err) = await!(future) {
        error!("{:?}", err);
    }
}

/// Stream item type which support payload and dummy values
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Payload<T> {
    /// Indicates a real payload element, which will be transferred like this over the wire
    Payload(T),
    /// Indicates a dummy element, which needs to be replaced with something to transmit over the wire
    Dummy,
}

impl<T> Payload<T> {
    /// Convert this instance of [`Payload`] into a `T`
    ///
    /// The function takes the payload value, if the variant is [`PAYLOAD`].
    /// Otherwise, this removes the [`DUMMY`] entries by executing `f` and returning the output.
    ///
    /// [`DUMMY`]: self::Payload::DUMMY
    /// [`PAYLOAD`]: self::Payload::PAYLOAD
    pub fn unwrap_or_else<F>(self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        match self {
            Payload::Payload(p) => p,
            Payload::Dummy => f(),
        }
    }
}

impl<T> Payload<Payload<T>> {
    /// Flatten two layers of [`Payload`] into one
    pub fn flatten(self) -> Payload<T> {
        match self {
            Payload::Payload(Payload::Payload(p)) => Payload::Payload(p),
            _ => Payload::Dummy,
        }
    }
}

// This is a custom type used to have a custom implementation of the
// `AsyncWrite::shutdown` method which actually calls `TcpStream::shutdown` to
// notify the remote end that we're done writing.
#[derive(Clone)]
pub struct MyTcpStream(Arc<Mutex<TcpStream>>);

impl MyTcpStream {
    pub fn new(stream: Arc<Mutex<TcpStream>>) -> Self {
        Self(stream)
    }
}

impl Read for MyTcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl Write for MyTcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl AsyncRead for MyTcpStream {}

impl AsyncWrite for MyTcpStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.0.lock().unwrap().shutdown(Shutdown::Write)?;
        Ok(().into())
    }
}

// This is a custom type used to have a custom implementation of the
// `AsyncWrite::shutdown` method which actually calls `TlsStream::shutdown` to
// notify the remote end that we're done writing.
pub struct TokioRustlsStream<IO, S>(Arc<Mutex<TlsStream<IO, S>>>);

impl<IO, S> TokioRustlsStream<IO, S> {
    pub fn new(stream: Arc<Mutex<TlsStream<IO, S>>>) -> Self {
        Self(stream)
    }
}

impl<IO, S> Clone for TokioRustlsStream<IO, S> {
    fn clone(&self) -> Self {
        TokioRustlsStream(self.0.clone())
    }
}

impl<IO, S> Read for TokioRustlsStream<IO, S>
where
    IO: Read + Write,
    S: Session,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl<IO, S> Write for TokioRustlsStream<IO, S>
where
    IO: Read + Write,
    S: Session,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl<IO, S> AsyncRead for TokioRustlsStream<IO, S>
where
    IO: AsyncRead + AsyncWrite,
    S: Session,
{
}

impl<IO, S> AsyncWrite for TokioRustlsStream<IO, S>
where
    IO: AsyncRead + AsyncWrite,
    S: Session,
{
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.0.lock().unwrap().shutdown()?;
        Ok(().into())
    }
}

// This is a custom type used to have a custom implementation of the
// `AsyncWrite::shutdown` method which actually calls `TlsStream::shutdown` to
// notify the remote end that we're done writing.
pub struct TokioOpensslStream<S>(Arc<Mutex<tokio_openssl::SslStream<S>>>);

impl<S> TokioOpensslStream<S> {
    pub fn new(stream: Arc<Mutex<tokio_openssl::SslStream<S>>>) -> Self {
        Self(stream)
    }
}

impl<S> Clone for TokioOpensslStream<S> {
    fn clone(&self) -> Self {
        TokioOpensslStream(self.0.clone())
    }
}

impl<S> Read for TokioOpensslStream<S>
where
    S: Read + Write,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl<S> Write for TokioOpensslStream<S>
where
    S: Read + Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl<S> AsyncRead for TokioOpensslStream<S> where S: AsyncRead + AsyncWrite {}

impl<S> AsyncWrite for TokioOpensslStream<S>
where
    S: AsyncRead + AsyncWrite,
{
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.0.lock().unwrap().shutdown()?;
        Ok(().into())
    }
}
