#![deny(rust_2018_compatibility)]
#![warn(rust_2018_idioms)]
// enable the await! macro, async support, and the new std::Futures api.
#![feature(await_macro, async_await, futures_api)]
// // only needed to manually implement a std future:
// #![feature(arbitrary_self_types)]

mod adaptive_padding;
// mod constant_rate;
mod dns_tcp;
mod error;
mod utils;

use crate::{
    adaptive_padding::AdaptivePadding, dns_tcp::DnsBytesStream, error::Error, utils::backward,
};
use byteorder::{BigEndian, WriteBytesExt};
use rustls::{KeyLogFile, Session};
use std::{
    fmt::Debug,
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use structopt::StructOpt;
use tokio::{
    await,
    io::shutdown,
    net::{TcpListener, TcpStream},
    prelude::*,
};
use tokio_rustls::TlsStream;

/// DNS query for `google.com.` with padding
const DUMMY_DNS: [u8; 128] = [
    184, 151, 1, 0, 0, 1, 0, 0, 0, 0, 0, 1, 6, 103, 111, 111, 103, 108, 101, 3, 99, 111, 109, 0, 0,
    1, 0, 1, 0, 0, 41, 16, 0, 0, 0, 0, 0, 0, 89, 0, 12, 0, 85, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/*

client <-> proxy <-> resolver

client -> proxy -> resolver
1. client -> proxy
    * TCP
    * no delay
2. proxy -> resolver
    * TLS
    * send on schedule
3. <-
    * no delays

*/

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    // name = "crossvalidate",
    author = "",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct CliArgs {
    /// Local TCP port
    #[structopt(
        short = "l",
        long = "listen",
        default_value = "127.0.0.1:8853",
        parse(try_from_str)
    )]
    listen: SocketAddr,

    /// Remote DNS over TLS endpoint
    #[structopt(
        short = "s",
        long = "server",
        default_value = "1.1.1.1:853",
        parse(try_from_str)
    )]
    server: SocketAddr,

    /// Log all TLS keys into this file
    #[structopt(long = "sslkeylogfile", env = "SSLKEYLOGFILE")]
    sslkeylogfile: Option<PathBuf>,

    #[structopt(subcommand)]
    strategy: Strategy,
}

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    rename_all = "kebab-case",
    author = "",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
enum Strategy {
    #[structopt(raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
    Constant {
        /// The rate in which packets are send specified in ms between them
        #[structopt(parse(try_from_str = "parse_duration_ms"))]
        rate: Duration,
    },
}

/// Parse a string as [`u64`], interpret it as milliseconds, and return a [`Duration`]
fn parse_duration_ms(s: &str) -> Result<Duration, std::num::ParseIntError> {
    let ms: u64 = s.parse()?;
    Ok(Duration::from_millis(ms))
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Payload<T> {
    Payload(T),
    Dummy,
}

impl<T> Payload<T> {
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

fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    let cli_args = CliArgs::from_args();
    if let Some(file) = cli_args.sslkeylogfile {
        std::env::set_var("SSLKEYLOGFILE", file.to_path_buf());
    }

    // Create a TCP listener which will listen for incoming connections.
    let socket = TcpListener::bind(&cli_args.listen)?;
    println!(
        "Listening on: {}\nProxying to: {}\n",
        cli_args.listen, cli_args.server
    );

    let server = cli_args.server;
    let done = socket
        .incoming()
        .map_err(|e| println!("error accepting socket; error = {:?}", e))
        .for_each(move |client| {
            tokio::spawn_async(print_error(handle_client(client, server)));
            Ok(())
        });

    tokio::run(done);
    Ok(())
}

async fn print_error<F, T, E>(future: F)
where
    F: std::future::Future<Output = Result<T, E>>,
    E: Debug,
{
    if let Err(err) = await!(future) {
        eprintln!("{:?}", err);
    }
}

async fn handle_client(client: TcpStream, server_addr: SocketAddr) -> Result<(), Error> {
    let server = await!(TcpStream::connect(&server_addr))?;

    // // tokio_tls
    // let cx = TlsConnector::builder().build().unwrap();
    // let server = await!(tokio_tls::TlsConnector::from(cx)
    //     .connect("cloudflare-dns.com", server)
    //     .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err)))?;
    // tokio_rustls
    let mut config = rustls::ClientConfig::new();
    config
        .root_store
        .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    config.key_log = Arc::new(KeyLogFile::new());
    let server = await!(tokio_rustls::TlsConnector::from(Arc::new(config))
        .connect(
            webpki::DNSNameRef::try_from_ascii_str("cloudflare-dns.com").unwrap(),
            server
        )
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err)))?;

    // Create separate read/write handles for the TCP clients that we're
    // proxying data between. Note that typically you'd use
    // `AsyncRead::split` for this operation, but we want our writer
    // handles to have a custom implementation of `shutdown` which
    // actually calls `TcpStream::shutdown` to ensure that EOF is
    // transmitted properly across the proxied connection.
    //
    // As a result, we wrap up our client/server manually in arcs and
    // use the impls below on our custom `MyTcpStream` type.
    let client_reader = MyTcpStream(Arc::new(Mutex::new(client)));
    let client_writer = client_reader.clone();
    let server_reader = TokioRustlsStream(Arc::new(Mutex::new(server)));
    let server_writer = server_reader.clone();

    // Copy the data (in parallel) between the client and the server.
    // After the copy is done we indicate to the remote side that we've
    // finished by shutting down the connection.
    let client_to_server = backward(copy_client_to_server(client_reader, server_writer))
        .and_then(|(n, _, server_writer)| shutdown(server_writer).map(move |_| n).from_err());

    let server_to_client = backward(copy_server_to_client(server_reader, client_writer))
        .and_then(|(n, _, client_writer)| shutdown(client_writer).map(move |_| n).from_err());

    let (from_client, from_server) = await!(client_to_server.join(server_to_client))?;
    println!(
        "client wrote {} bytes and received {} bytes",
        from_client, from_server
    );

    Ok(())
}

async fn copy_client_to_server<R, W>(mut client: R, mut server: W) -> Result<(u64, R, W), Error>
where
    R: AsyncRead + Send,
    W: AsyncWrite,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(128 * 5);
    let dnsbytes = DnsBytesStream::new(&mut client);
    // let mut delayed = ConstantRate::new(Duration::from_millis(400), dnsbytes);
    let mut delayed = AdaptivePadding::new(dnsbytes);
    while let Some(dns) = await!(delayed.next()) {
        let dns = dns?.unwrap_or_else(|| DUMMY_DNS.to_vec());
        out.truncate(0);
        out.write_u16::<BigEndian>(dns.len() as u16)?;
        out.extend_from_slice(&*dns);

        // Add 2 for the length of the length header
        total_bytes += out.len() as u64;
        await!(tokio::io::write_all(&mut server, &mut out))?;
        server.flush()?;
    }
    // Ensure client stream is available again
    drop(delayed);
    Ok((total_bytes, client, server))
}

async fn copy_server_to_client<R, W>(mut server: R, mut client: W) -> Result<(u64, R, W), Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(128 * 5);
    let mut dnsbytes = DnsBytesStream::new(&mut server);
    while let Some(dns) = await!(dnsbytes.next()) {
        let dns = dns?;
        let msg = trust_dns_proto::op::message::Message::from_vec(&*dns)?;

        // Remove all dummy messages from the responses
        if msg.id() == 47255 {
            continue;
        }

        out.truncate(0);
        out.write_u16::<BigEndian>(dns.len() as u16)?;
        out.extend_from_slice(&*dns);

        // Add 2 for the length of the length header
        total_bytes += out.len() as u64;
        await!(tokio::io::write_all(&mut client, &mut out))?;
        client.flush()?;
    }

    Ok((total_bytes, server, client))
}

// This is a custom type used to have a custom implementation of the
// `AsyncWrite::shutdown` method which actually calls `TcpStream::shutdown` to
// notify the remote end that we're done writing.
#[derive(Clone)]
struct MyTcpStream(Arc<Mutex<TcpStream>>);

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
struct TokioRustlsStream<IO, S>(Arc<Mutex<TlsStream<IO, S>>>);

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
