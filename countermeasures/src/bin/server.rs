#![deny(rust_2018_compatibility)]
#![warn(rust_2018_idioms)]
// enable the await! macro, async support, and the new std::Futures api.
#![feature(await_macro, async_await, futures_api)]
// // only needed to manually implement a std future:
// #![feature(arbitrary_self_types)]

use byteorder::{BigEndian, WriteBytesExt};
use failure::Fail;
use rustls::KeyLogFile;
use std::{
    io::Cursor,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use structopt::StructOpt;
use tlsproxy::{
    parse_duration_ms, print_error, utils::backward, ConstantRate, DnsBytesStream, Error,
    TokioRustlsStream,
};
use tokio::{
    await,
    io::shutdown,
    net::{TcpListener, TcpStream},
    prelude::*,
};
use tokio_rustls::TlsAcceptor;

const SERVER_CERT: &[u8] = include_bytes!("../../cert.pem");
const SERVER_KEY: &[u8] = include_bytes!("../../key.pem");
const DUMMY_DNS_REPLY: [u8; 468] = [
    /*0x01, 0xd4,*/ 0x0a, 0xa6, 0x81, 0x80, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x06, 0x67, 0x6f, 0x6f, 0x67, 0x6c, 0x65, 0x03, 0x63, 0x6f, 0x6d, 0x00, 0x00, 0x01, 0x00, 0x01,
    0xc0, 0x0c, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x3e, 0x00, 0x04, 0xac, 0xd9, 0x16, 0x4e,
    0x00, 0x00, 0x29, 0x05, 0xac, 0x00, 0x00, 0x00, 0x00, 0x01, 0x9d, 0x00, 0x0c, 0x01, 0x99, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[derive(Clone, Debug, StructOpt)]
#[structopt(
    // name = "crossvalidate",
    author = "",
    raw(setting = "structopt::clap::AppSettings::ColoredHelp")
)]
struct CliArgs {
    /// Local TLS port
    #[structopt(
        short = "l",
        long = "listen",
        default_value = "127.0.0.1:1853",
        parse(try_from_str)
    )]
    listen: SocketAddr,

    // FIXME add --tcp and --tls options to force the remote endpoint to those protocols
    // Otherwise guess from port number 53/853
    // Otherwise error out
    /// Remote DNS over TCP / DNS over TLS endpoint
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

fn main() {
    use std::io::{self, Write};

    if let Err(err) = run() {
        let stderr = io::stderr();
        let mut out = stderr.lock();
        // cannot handle a write error here, we are already in the outermost layer
        let _ = writeln!(out, "An error occured:");
        for fail in Fail::iter_chain(&err) {
            let _ = writeln!(out, "  {}", fail);
        }
        if let Some(backtrace) = err.backtrace() {
            let _ = writeln!(out, "{}", backtrace);
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    // generic setup
    let log_settings = "server=debug,tlsproxy=debug";
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_settings))
        .init();
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

    let mut config = rustls::ServerConfig::new(rustls::NoClientAuth::new());
    let certs = rustls::internal::pemfile::certs(&mut Cursor::new(SERVER_CERT)).unwrap();
    let mut keys =
        rustls::internal::pemfile::pkcs8_private_keys(&mut Cursor::new(SERVER_KEY)).unwrap();
    assert_eq!(1, keys.len(), "Only one key should be specified");
    config.set_single_cert(certs, keys.pop().unwrap())?;
    let config = TlsAcceptor::from(Arc::new(config));

    let done = socket
        .incoming()
        .map_err(|e| println!("error accepting socket; error = {:?}", e))
        .for_each(move |client| {
            tokio::spawn_async(print_error(handle_client(client, config.clone(), server)));
            Ok(())
        });

    tokio::run(done);
    Ok(())
}

async fn handle_client(
    client: TcpStream,
    acceptor: TlsAcceptor,
    server_addr: SocketAddr,
) -> Result<(), Error> {
    // Setup TLS to client
    let client = await!(acceptor.accept(client))?;

    let server = await!(TcpStream::connect(&server_addr))?;
    let mut config = rustls::ClientConfig::new();
    config
        .root_store
        .add_server_trust_anchors(&webpki_roots::TLS_SERVER_ROOTS);
    config.key_log = Arc::new(KeyLogFile::new());
    let server = await!(tokio_rustls::TlsConnector::from(Arc::new(config))
        .connect(
            // FIXME we need a hostname here, so instead of a SocketAddr, we need something which carries a hostname
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
    let client_reader = TokioRustlsStream::new(Arc::new(Mutex::new(client)));
    let client_writer = client_reader.clone();
    let server_reader = TokioRustlsStream::new(Arc::new(Mutex::new(server)));
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
    let mut dnsbytes = DnsBytesStream::new(&mut client);
    while let Some(dns) = await!(dnsbytes.next()) {
        let dns = dns?;

        out.truncate(0);
        out.write_u16::<BigEndian>(dns.len() as u16)?;
        out.extend_from_slice(&*dns);

        // Add 2 for the length of the length header
        total_bytes += out.len() as u64;
        await!(tokio::io::write_all(&mut server, &mut out))?;
        server.flush()?;
    }

    Ok((total_bytes, client, server))
}

async fn copy_server_to_client<R, W>(mut server: R, mut client: W) -> Result<(u64, R, W), Error>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(468 * 5);
    let dnsbytes = DnsBytesStream::new(&mut server);
    let mut delayed = ConstantRate::new(Duration::from_millis(400), dnsbytes);
    // let mut delayed = AdaptivePadding::new(dnsbytes);
    while let Some(dns) = await!(delayed.next()) {
        let dns = dns?.unwrap_or_else(|| DUMMY_DNS_REPLY.to_vec());

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
