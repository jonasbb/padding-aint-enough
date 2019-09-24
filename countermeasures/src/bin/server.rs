#![deny(rust_2018_compatibility)]
#![warn(rust_2018_idioms)]

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use failure::Fail;
use futures::{future, Stream};
use log::info;
use openssl::{
    pkey::PKey,
    ssl::{SslAcceptor, SslConnector, SslMethod, SslOptions, SslVerifyMode, SslVersion},
    x509::X509,
};
use std::{
    io::Write,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use structopt::StructOpt;
use tlsproxy::{
    print_error, wrap_stream, DnsBytesStream, EnsurePadding, Error, HostnameSocketAddr, MyStream,
    MyTcpStream, Payload, Strategy, TokioOpensslStream, Transport, SERVER_CERT, SERVER_KEY,
};
use tokio::{
    net::{TcpListener, TcpStream},
    prelude::*,
};
use tokio_openssl;
use trust_dns_proto::{
    op::message::Message,
    serialize::binary::{BinEncodable, BinEncoder},
};

const DUMMY_DNS_REPLY: [u8; 468] = [
    /* 0x01, 0xd4, */ 0xb8, 0x97, 0x81, 0x80, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
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

#[derive(Clone, Debug, StructOpt)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
struct CliArgs {
    /// Local TLS port
    #[structopt(
        short = "l",
        long = "listen",
        default_value = "127.0.0.1:1853",
        parse(try_from_str)
    )]
    listen: SocketAddr,

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
    use std::io;

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
        .format_timestamp_nanos()
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

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async_run(config))
}

async fn async_run(config: Config) -> Result<(), Error> {
    // Create a TCP listener which will listen for incoming connections.
    let socket = TcpListener::bind(&config.args.listen).await?;
    println!(
        "Listening on: {}\nProxying to: {}\n",
        config.args.listen, config.args.server
    );

    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
    acceptor.set_verify(SslVerifyMode::NONE);
    acceptor.set_certificate(X509::from_pem(SERVER_CERT)?.as_ref())?;
    acceptor.set_private_key(PKey::private_key_from_pem(SERVER_KEY)?.as_ref())?;
    if let Some(logfile) = &config.args.sslkeylogfile {
        let cb = tlsproxy::keylog_to_file(logfile.clone());
        acceptor.set_keylog_callback(cb);
    }
    let acceptor = acceptor.build();

    let config = Arc::new(config);
    let done = socket
        .incoming()
        // conver the Error to tlsproxy::Error
        .map(|x| Ok(x?))
        .for_each_concurrent(100, move |client| {
            tokio::spawn(print_error(handle_client(
                config.clone(),
                client,
                acceptor.clone(),
            )));
            future::ready(())
        });
    done.await;
    Ok(())
}

async fn handle_client(
    config: Arc<Config>,
    client: Result<TcpStream, Error>,
    acceptor: SslAcceptor,
) -> Result<(), Error> {
    let client = client?;
    // Setup TLS to client
    client.set_nodelay(true)?;
    let client = tokio_openssl::accept(&acceptor, client).await?;

    let (server_reader, server_writer) =
        connect_to_server(config.args.server.clone(), &*config).await?;

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
    let client_reader = DnsBytesStream::new(client_reader);
    let client_reader = EnsurePadding::new(client_reader);
    let client_to_server = copy_client_to_server(client_reader, server_writer);

    let server_reader = DnsBytesStream::new(server_reader).map(|x| Ok(x?));
    let server_reader = wrap_stream(server_reader, &config.args.strategy);
    let server_to_client = copy_server_to_client(server_reader, client_writer);

    let (from_client, from_server) = future::join(client_to_server, server_to_client).await;
    let from_client = from_client?;
    let from_server = from_server?;
    println!(
        "client wrote {} bytes and received {} bytes",
        from_client, from_server
    );

    Ok(())
}

async fn copy_client_to_server<R, W>(mut client: R, mut server: W) -> Result<u64, Error>
where
    R: Stream<Item = Result<Message, Error>> + Send + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(128 * 5);
    while let Some(dns) = client.next().await {
        let dns = dns?;

        out.truncate(0);
        // write placeholder length, replaced later
        out.write_u16::<BigEndian>(0)?;
        {
            let mut encoder = BinEncoder::new(&mut out);
            encoder.set_offset(2);
            dns.emit(&mut encoder)?;
        }
        let len = (out.len() - 2) as u16;
        BigEndian::write_u16(&mut out[..], len);

        info!("C->S {}B", len);

        // Add 2 for the length of the length header
        total_bytes += out.len() as u64;
        server.write_all(&out).await?;
        server.flush().await?;
    }

    // We need to pass the shutdown from client to server, that the server sees that the client shut
    // down the connection. Automatic shutdown does not work in this case, as the reading and
    // writing part access the same underlying TcpStream, thus the drop based shutdown would be too
    // late.
    server.shutdown().await?;
    Ok(total_bytes)
}

async fn copy_server_to_client<R, W>(mut server: R, mut client: W) -> Result<u64, Error>
where
    R: Stream<Item = Payload<Result<Vec<u8>, Error>>> + Send + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(468 * 5);
    while let Some(dns) = server.next().await {
        let dns = match dns.transpose_error()? {
            Payload::Payload(p) => {
                info!("C<-S payload {}B", p.len());
                p
            }
            Payload::Dummy => {
                let res = DUMMY_DNS_REPLY.to_vec();
                info!("C<-S dummy {}B", res.len());
                res
            }
        };

        out.truncate(0);
        out.write_u16::<BigEndian>(dns.len() as u16)?;
        out.extend_from_slice(&*dns);

        // Add 2 for the length of the length header
        total_bytes += out.len() as u64;
        client.write_all(&out).await?;
        client.flush().await?;
    }

    // We need to pass the shutdown from client to server, that the server sees that the client shut
    // down the connection. Automatic shutdown does not work in this case, as the reading and
    // writing part access the same underlying TcpStream, thus the drop based shutdown would be too
    // late.
    client.shutdown().await?;
    Ok(total_bytes)
}

#[allow(clippy::needless_lifetimes)]
async fn connect_to_server(
    server_addr: HostnameSocketAddr,
    config: &Config,
) -> Result<(impl AsyncRead, impl AsyncWrite), Error> {
    // Open a tcp connection. This is always needed
    let server_socket_addr = server_addr.socket_addr();
    let server = TcpStream::connect(&server_socket_addr).await?;
    server.set_nodelay(true)?;

    let server: MyStream<_> = match config.transport {
        Transport::Tcp => MyTcpStream::new(Arc::new(Mutex::new(server))).into(),

        Transport::Tls => {
            let mut connector = SslConnector::builder(SslMethod::tls())?;
            connector.set_min_proto_version(Some(SslVersion::TLS1_2))?;
            connector.set_options(SslOptions::NO_COMPRESSION);
            if let Some(logfile) = &config.args.sslkeylogfile {
                let cb = tlsproxy::keylog_to_file(logfile.clone());
                connector.set_keylog_callback(cb);
            }
            let connector = connector.build();
            let connector_config = connector.configure()?;
            let hostname = server_addr.hostname();
            let server = tokio_openssl::connect(connector_config, &hostname, server).await?;

            TokioOpensslStream::new(Arc::new(Mutex::new(server))).into()
        }
    };

    let server_writer = server.clone();
    Ok((server, server_writer))
}
