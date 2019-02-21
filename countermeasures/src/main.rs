// enable the await! macro, async support, and the new std::Futures api.
#![feature(await_macro, async_await, futures_api)]
// only needed to manually implement a std future:
#![feature(arbitrary_self_types)]

mod utils;

use crate::utils::backward;
use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use log::debug;
use native_tls::TlsConnector;
use std::{
    env,
    fmt::Debug,
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr},
    sync::{Arc, Mutex},
};
use tokio::{
    await,
    io::{copy, shutdown},
    net::{TcpListener, TcpStream},
    prelude::*,
};
// use tokio_tls::TlsStream;
use rustls::KeyLogFile;
use tokio_rustls::TlsStream;

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

fn main() -> Result<(), Box<std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8081".to_string());
    let listen_addr = listen_addr.parse::<SocketAddr>()?;

    let server_addr = "1.1.1.1:853".parse::<SocketAddr>()?;

    // Create a TCP listener which will listen for incoming connections.
    let socket = TcpListener::bind(&listen_addr)?;
    println!("Listening on: {}", listen_addr);
    println!("Proxying to: {}", server_addr);

    let done = socket
        .incoming()
        .map_err(|e| println!("error accepting socket; error = {:?}", e))
        .for_each(|client| {
            tokio::spawn_async(print_error(handle_client(client)));
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

async fn handle_client(client: TcpStream) -> io::Result<()> {
    let server_addr = "1.1.1.1:853".parse::<SocketAddr>().unwrap();
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
    // let client_to_server = copy(client_reader, server_writer)
    //     .and_then(|(n, _, server_writer)| shutdown(server_writer).map(move |_| n));
    let client_to_server = backward(copy_client_to_server(client_reader, server_writer))
        .and_then(|(n, _, server_writer)| shutdown(server_writer).map(move |_| n));

    let server_to_client = copy(server_reader, client_writer)
        .and_then(|(n, _, client_writer)| shutdown(client_writer).map(move |_| n));

    let (from_client, from_server) = await!(client_to_server.join(server_to_client))?;
    println!(
        "client wrote {} bytes and received {} bytes",
        from_client, from_server
    );

    Ok(())
}

async fn copy_client_to_server<R, W>(mut client: R, mut server: W) -> io::Result<(u64, R, W)>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut total_bytes = 0;

    let mut dnsbytes = DnsBytesStream::new(&mut client);
    while let Some(dns) = await!(dnsbytes.next()) {
        let dns = dns?;
        // Add 2 for the length of the length header
        total_bytes += 2 + dns.len() as u64;
        // TODO write them in one go as otherwise they end up in two TLS segments
        server.write_u16::<BigEndian>(dns.len() as u16)?;
        server.write_all(&*dns)?;
        server.flush()?;
    }
    Ok((total_bytes, client, server))
}

enum DnsBytesReadState {
    Length,
    DnsMessage,
}

struct DnsBytesStream<R> {
    read: R,
    buf: [u8; 128 * 5],
    bytes_read: usize,
    expected_bytes: usize,
    read_state: DnsBytesReadState,
}

impl<R> DnsBytesStream<R> {
    fn new(read: R) -> Self {
        Self {
            read,
            buf: [0; 128 * 5],
            bytes_read: 0,
            expected_bytes: 2,
            read_state: DnsBytesReadState::Length,
        }
    }
}

impl<R: AsyncRead> Stream for DnsBytesStream<R> {
    // The same as our future above:
    type Item = Vec<u8>;
    type Error = io::Error;

    // poll is very similar to our Future implementation, except that
    // it returns an `Option<u8>` instead of a `u8`. This is so that the
    // Stream can signal that it's finished by returning `None`:
    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, io::Error> {
        if self.bytes_read < self.expected_bytes {
            debug!(
                "Read {} bytes, expects {} bytes, missing {} bytes",
                self.bytes_read,
                self.expected_bytes,
                self.expected_bytes - self.bytes_read
            );
            match self
                .read
                .poll_read(&mut self.buf[self.bytes_read..self.expected_bytes])
            {
                Ok(Async::Ready(n)) => {
                    // By convention, if an AsyncRead says that it read 0 bytes,
                    // we should assume that it has got to the end, so we signal that
                    // the Stream is done in this case by returning None:
                    if n == 0 {
                        return Ok(Async::Ready(None));
                    } else {
                        self.bytes_read += n;
                    }
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => return Err(e),
            }
        }

        // now that we read more, we may be able to process it
        if self.bytes_read == self.expected_bytes {
            match self.read_state {
                DnsBytesReadState::Length => {
                    let len = BigEndian::read_u16(&self.buf[0..self.expected_bytes]) as usize;
                    debug!("Read length field: {}", len);

                    // init next state
                    self.bytes_read = 0;
                    self.expected_bytes = len;
                    self.read_state = DnsBytesReadState::DnsMessage;
                    // poll again, since we might be able to make progress
                    self.poll()
                }
                DnsBytesReadState::DnsMessage => {
                    let ret = self.buf[0..self.expected_bytes].to_vec();
                    debug!("Read DNS message");

                    // init next state
                    self.bytes_read = 0;
                    self.expected_bytes = 2;
                    self.read_state = DnsBytesReadState::Length;
                    Ok(Async::Ready(Some(ret)))
                }
            }
        } else {
            Ok(Async::NotReady)
        }
    }
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
// `AsyncWrite::shutdown` method which actually calls `TcpStream::shutdown` to
// notify the remote end that we're done writing.
#[derive(Clone)]
struct TokioTlsStream(Arc<Mutex<tokio_tls::TlsStream<TcpStream>>>);

impl Read for TokioTlsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl Write for TokioTlsStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl AsyncRead for TokioTlsStream {}

impl AsyncWrite for TokioTlsStream {
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.0.lock().unwrap().shutdown()?;
        Ok(().into())
    }
}

// This is a custom type used to have a custom implementation of the
// `AsyncWrite::shutdown` method which actually calls `TcpStream::shutdown` to
// notify the remote end that we're done writing.
struct TokioRustlsStream<IO, S>(Arc<Mutex<tokio_rustls::TlsStream<IO, S>>>);

impl<IO, S> Clone for TokioRustlsStream<IO, S> {
    fn clone(&self) -> Self {
        TokioRustlsStream(self.0.clone())
    }
}

impl<IO, S> Read for TokioRustlsStream<IO, S>
where
    IO: Read + Write,
    S: rustls::Session,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.lock().unwrap().read(buf)
    }
}

impl<IO, S> Write for TokioRustlsStream<IO, S>
where
    IO: Read + Write,
    S: rustls::Session,
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
    S: rustls::Session,
{
}

impl<IO, S> AsyncWrite for TokioRustlsStream<IO, S>
where
    IO: AsyncRead + AsyncWrite,
    S: rustls::Session,
{
    fn shutdown(&mut self) -> Poll<(), io::Error> {
        self.0.lock().unwrap().shutdown()?;
        Ok(().into())
    }
}
