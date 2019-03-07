#![deny(rust_2018_compatibility)]
#![warn(rust_2018_idioms)]
// enable the await! macro, async support, and the new std::Futures api.
#![feature(await_macro, async_await, futures_api)]
// // only needed to manually implement a std future:
// #![feature(arbitrary_self_types)]

use byteorder::{BigEndian, WriteBytesExt};
use failure::Fail;
use log::{trace, warn};
use openssl::{
    ssl::{SslConnector, SslMethod, SslOptions, SslVerifyMode, SslVersion},
    x509::X509,
};
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use structopt::StructOpt;
use tlsproxy::{
    parse_duration_ms, print_error, utils::backward, AdaptivePadding, DnsBytesStream, Error,
    HostnameSocketAddr, MyTcpStream, TokioOpensslStream, SERVER_CERT,
};
use tokio::{
    await,
    io::shutdown,
    net::{TcpListener, TcpStream},
    prelude::*,
};
use tokio_openssl::SslConnectorExt;

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
    server: HostnameSocketAddr,

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
    let log_settings = "client=debug,tlsproxy=debug";
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_settings))
        .default_format_timestamp_nanos(true)
        .init();
    openssl_probe::init_ssl_cert_env_vars();
    let cli_args = CliArgs::from_args();
    if let Some(file) = &cli_args.sslkeylogfile {
        std::env::set_var("SSLKEYLOGFILE", file.to_path_buf());
    }

    // Create a TCP listener which will listen for incoming connections.
    let socket = TcpListener::bind(&cli_args.listen)?;
    println!(
        "Listening on: {}\nProxying to: {}\n",
        cli_args.listen, cli_args.server
    );

    let config = Arc::new(cli_args);
    let done = socket
        .incoming()
        .map_err(|e| println!("error accepting socket; error = {:?}", e))
        .for_each(move |client| {
            tokio::spawn_async(print_error(handle_client(config.clone(), client)));
            Ok(())
        });

    tokio::run(done);
    Ok(())
}

async fn handle_client(config: Arc<CliArgs>, client: TcpStream) -> Result<(), Error> {
    client.set_nodelay(true)?;

    let server = await!(TcpStream::connect(&config.server.socket_addr()))?;
    server.set_nodelay(true)?;
    let mut connector = SslConnector::builder(SslMethod::tls())?;
    connector.set_min_proto_version(Some(SslVersion::TLS1_2))?;
    connector.set_options(SslOptions::NO_COMPRESSION);
    // make the connector always accept my cert
    connector.set_verify_callback(
        SslVerifyMode::PEER,
        |passed_openssl_cert_check, cert_context| {
            // Extract the signature of our known good cert
            let cert = X509::from_pem(SERVER_CERT).ok();
            let good_cert_signature = cert.as_ref().map(|cert| cert.signature().as_slice());

            // get the signature of the certificate from the server
            let cert_signature = cert_context
                .current_cert()
                .map(|cert| cert.signature().as_slice());

            // Log the signatures
            trace!("{:?}\n\n{:?}", cert_signature, good_cert_signature);

            // allow certificate if either openssl accepts it or if the signature matches our known good
            passed_openssl_cert_check || (cert_signature == good_cert_signature)
        },
    );
    if let Some(logfile) = std::env::var_os("SSLKEYLOGFILE") {
        let cb = tlsproxy::keylog_to_file(logfile);
        connector.set_keylog_callback(cb);
    }
    let connector = connector.build();
    let server = await!(connector.connect_async(&config.server.hostname(), server))?;

    // Create separate read/write handles for the TCP clients that we're
    // proxying data between. Note that typically you'd use
    // `AsyncRead::split` for this operation, but we want our writer
    // handles to have a custom implementation of `shutdown` which
    // actually calls `TcpStream::shutdown` to ensure that EOF is
    // transmitted properly across the proxied connection.
    //
    // As a result, we wrap up our client/server manually in arcs and
    // use the impls below on our custom `MyTcpStream` type.
    let client_reader = MyTcpStream::new(Arc::new(Mutex::new(client)));
    let client_writer = client_reader.clone();
    let server_reader = TokioOpensslStream::new(Arc::new(Mutex::new(server)));
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

    let mut out = Vec::with_capacity(468 * 5);
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
