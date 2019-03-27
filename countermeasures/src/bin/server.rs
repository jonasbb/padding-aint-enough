#![deny(rust_2018_compatibility)]
#![warn(rust_2018_idioms)]
// enable the await! macro, async support, and the new std::Futures api.
#![feature(await_macro, async_await, futures_api)]
// // only needed to manually implement a std future:
// #![feature(arbitrary_self_types)]

use byteorder::{BigEndian, WriteBytesExt};
use failure::Fail;
use log::info;
use openssl::{
    pkey::PKey,
    ssl::{SslAcceptor, SslConnector, SslMethod, SslOptions, SslVerifyMode, SslVersion},
    x509::X509,
};
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use structopt::StructOpt;
use tlsproxy::{
    print_error, utils::backward, wrap_stream, DnsBytesStream, Error, HostnameSocketAddr, MyStream,
    MyTcpStream, Payload, Strategy, TokioOpensslStream, SERVER_CERT, SERVER_KEY,
};
use tokio::{
    await,
    io::shutdown,
    net::{TcpListener, TcpStream},
    prelude::*,
};
use tokio_openssl::{SslAcceptorExt, SslConnectorExt};

const DUMMY_DNS_REPLY: [u8; 468] = [
    /*0x01, 0xd4,*/ 0xb8, 0x97, 0x81, 0x80, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
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

#[derive(Clone, Debug)]
struct Config {
    args: CliArgs,
    transport: Transport,
}

/// Specify the transport protocol to be used while connecting to a remote endpoint
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
enum Transport {
    /// Use TCP
    Tcp,
    /// Use TLS
    Tls,
}

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
    server: HostnameSocketAddr,

    /// Force the connection to use TCP. Conflicts with `--tls`.
    ///
    /// If unspecified infer transport from `server` port.
    #[structopt(long = "tcp", conflicts_with = "tls")]
    tcp: bool,

    /// Force the connection to use TLS. Conflicts with `--tcp`.
    ///
    /// If unspecified infer transport from `server` port.
    #[structopt(long = "tls", conflicts_with = "tcp")]
    tls: bool,

    /// Log all TLS keys into this file
    #[structopt(long = "sslkeylogfile", env = "SSLKEYLOGFILE")]
    sslkeylogfile: Option<PathBuf>,

    #[structopt(subcommand)]
    strategy: Strategy,
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
        .default_format_timestamp_nanos(true)
        .init();
    let mut config = Config {
        args: CliArgs::from_args(),
        // This value will be overwritten later
        transport: Transport::Tcp,
    };
    if let Some(file) = &config.args.sslkeylogfile {
        std::env::set_var("SSLKEYLOGFILE", file.to_path_buf());
    }

    match (config.args.tcp, config.args.tls, config.args.server.port()) {
        (true, false, _) => config.transport = Transport::Tcp,
        (false, true, _) => config.transport = Transport::Tls,
        (false, false, 53) => config.transport = Transport::Tcp,
        (false, false, 853) => config.transport = Transport::Tls,
        (false, false, port) => return Err(Error::TransportNotInferable(port, Default::default())),

        (true, true, _) => unreachable!(
            "This case is already checked in Clap by having those flags be mutually exclusive."
        ),
    }

    // Create a TCP listener which will listen for incoming connections.
    let socket = TcpListener::bind(&config.args.listen)?;
    println!(
        "Listening on: {}\nProxying to: {}\n",
        config.args.listen, config.args.server
    );

    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
    acceptor.set_verify(SslVerifyMode::NONE);
    acceptor.set_certificate(X509::from_pem(SERVER_CERT)?.as_ref())?;
    acceptor.set_private_key(PKey::private_key_from_pem(SERVER_KEY)?.as_ref())?;
    let acceptor = acceptor.build();

    let config = Arc::new(config);
    let done = socket
        .incoming()
        .map_err(|e| println!("error accepting socket; error = {:?}", e))
        .for_each(move |client| {
            tokio::spawn_async(print_error(handle_client(
                config.clone(),
                client,
                acceptor.clone(),
            )));
            Ok(())
        });

    tokio::run(done);
    Ok(())
}

async fn handle_client(
    config: Arc<Config>,
    client: TcpStream,
    acceptor: SslAcceptor,
) -> Result<(), Error> {
    // Setup TLS to client
    client.set_nodelay(true)?;
    let client = await!(acceptor.accept_async(client))?;

    let (server_reader, server_writer) = await!(connect_to_server(
        config.args.server.clone(),
        config.transport
    ))?;

    // Create separate read/write handles for the TCP clients that we're
    // proxying data between. Note that typically you'd use
    // `AsyncRead::split` for this operation, but we want our writer
    // handles to have a custom implementation of `shutdown` which
    // actually calls `TcpStream::shutdown` to ensure that EOF is
    // transmitted properly across the proxied connection.
    //
    // As a result, we wrap up our client/server manually in arcs and
    // use the impls below on our custom `MyTcpStream` type.
    let client_reader = TokioOpensslStream::new(Arc::new(Mutex::new(client)));
    let client_writer = client_reader.clone();

    // Copy the data (in parallel) between the client and the server.
    // After the copy is done we indicate to the remote side that we've
    // finished by shutting down the connection.
    let client_reader = DnsBytesStream::new(client_reader).from_err();
    let client_to_server = backward(copy_client_to_server(client_reader, server_writer));

    let server_reader = DnsBytesStream::new(server_reader).from_err();
    let server_reader = wrap_stream(server_reader, &config.args.strategy);
    let server_to_client = backward(copy_server_to_client(server_reader, client_writer));

    let (from_client, from_server) = await!(client_to_server.join(server_to_client))?;
    println!(
        "client wrote {} bytes and received {} bytes",
        from_client, from_server
    );

    Ok(())
}

async fn copy_client_to_server<R, W>(mut client: R, mut server: W) -> Result<u64, Error>
where
    R: Stream<Item = Vec<u8>, Error = Error> + Send + Unpin,
    W: AsyncWrite,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(128 * 5);
    while let Some(dns) = await!(client.next()) {
        let dns = dns?;

        out.truncate(0);
        out.write_u16::<BigEndian>(dns.len() as u16)?;
        out.extend_from_slice(&*dns);

        // Add 2 for the length of the length header
        total_bytes += out.len() as u64;
        await!(tokio::io::write_all(&mut server, &mut out))?;
        server.flush()?;
    }

    // We need to shutdown the endpoint before they are closed due to dropping one of the endpoints
    // hint: server and client access the same underlying TcpStream
    await!(shutdown(server))?;
    Ok(total_bytes)
}

async fn copy_server_to_client<R, W>(mut server: R, mut client: W) -> Result<u64, Error>
where
    R: Stream<Item = Payload<Vec<u8>>, Error = Error> + Send + Unpin,
    W: AsyncWrite,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(468 * 5);
    while let Some(dns) = await!(server.next()) {
        let dns = match dns? {
            Payload::Payload(p) => {
                info!("Send payload");
                p
            }
            Payload::Dummy => {
                info!("Send dummy");
                DUMMY_DNS_REPLY.to_vec()
            }
        };

        out.truncate(0);
        out.write_u16::<BigEndian>(dns.len() as u16)?;
        out.extend_from_slice(&*dns);

        // Add 2 for the length of the length header
        total_bytes += out.len() as u64;
        await!(tokio::io::write_all(&mut client, &mut out))?;
        client.flush()?;
    }

    // We need to shutdown the endpoint before they are closed due to dropping one of the endpoints
    // hint: server and client access the same underlying TcpStream
    await!(shutdown(client))?;
    Ok(total_bytes)
}

async fn connect_to_server(
    server_addr: HostnameSocketAddr,
    transport: Transport,
) -> Result<(impl AsyncRead, impl AsyncWrite), Error> {
    // Open a tcp connection. This is always needed
    let server = await!(TcpStream::connect(&server_addr.socket_addr()))?;
    server.set_nodelay(true)?;

    let server: MyStream<_> = match transport {
        Transport::Tcp => MyTcpStream::new(Arc::new(Mutex::new(server))).into(),

        Transport::Tls => {
            let mut connector = SslConnector::builder(SslMethod::tls())?;
            connector.set_min_proto_version(Some(SslVersion::TLS1_2))?;
            connector.set_options(SslOptions::NO_COMPRESSION);
            if let Some(logfile) = std::env::var_os("SSLKEYLOGFILE") {
                let cb = tlsproxy::keylog_to_file(logfile);
                connector.set_keylog_callback(cb);
            }
            let connector = connector.build();
            let server = await!(connector.connect_async(&server_addr.hostname(), server))?;

            TokioOpensslStream::new(Arc::new(Mutex::new(server))).into()
        }
    };

    let server_writer = server.clone();
    Ok((server, server_writer))
}
