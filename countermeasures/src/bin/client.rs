#![deny(rust_2018_compatibility)]
#![warn(rust_2018_idioms)]
// enable the await! macro, async support, and the new std::Futures api.
#![feature(await_macro, async_await, futures_api)]
// // only needed to manually implement a std future:
// #![feature(arbitrary_self_types)]

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use chrono::{SecondsFormat, Utc};
use failure::Fail;
use log::{info, trace, warn};
use openssl::{
    ssl::{SslConnector, SslMethod, SslOptions, SslVerifyMode, SslVersion},
    x509::X509,
};
use sequences::Sequence;
use std::{
    mem,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};
use structopt::StructOpt;
use tlsproxy::{
    print_error, utils::backward, wrap_stream, DnsBytesStream, EnsurePadding, Error,
    HostnameSocketAddr, MyTcpStream, Payload, Strategy, TokioOpensslStream, SERVER_CERT,
};
use tokio::{
    await,
    fs::File,
    io::{self, shutdown},
    net::{TcpListener, TcpStream},
    prelude::*,
};
use tokio_openssl::SslConnectorExt;
use trust_dns_proto::{
    op::message::Message,
    serialize::binary::{BinEncodable, BinEncoder},
};

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
    #[structopt(long = "sslkeylogfile", env = "SSLKEYLOGFILE", value_name = "FILE")]
    sslkeylogfile: Option<PathBuf>,

    /// Dump sequence files off all connections of this client.
    #[structopt(long = "dump-sequences", value_name = "DIR")]
    dump_sequences: Option<PathBuf>,

    #[structopt(subcommand)]
    strategy: Strategy,
}

#[derive(Debug)]
struct Config {
    args: CliArgs,
    message: Mutex<Vec<(u16, Instant)>>,
}

impl From<CliArgs> for Config {
    fn from(args: CliArgs) -> Self {
        Self {
            args,
            message: Mutex::default(),
        }
    }
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
    eprintln!("{:?}", cli_args);
    if let Some(file) = &cli_args.sslkeylogfile {
        std::env::set_var("SSLKEYLOGFILE", file.to_path_buf());
    }

    // Create a TCP listener which will listen for incoming connections.
    let socket = TcpListener::bind(&cli_args.listen)?;
    println!(
        "Listening on: {}\nProxying to: {}\n",
        cli_args.listen, cli_args.server
    );

    let config: Arc<Config> = Arc::new(cli_args.into());
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

async fn handle_client(config: Arc<Config>, client: TcpStream) -> Result<(), Error> {
    client.set_nodelay(true)?;

    let server = await!(TcpStream::connect(&config.args.server.socket_addr()))?;
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
    let server = await!(connector.connect_async(&config.args.server.hostname(), server))?;

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
    let client_reader = DnsBytesStream::new(client_reader).from_err();
    let client_reader = EnsurePadding::new(client_reader);
    let client_reader = wrap_stream(client_reader, &config.args.strategy);
    let client_to_server = backward(copy_client_to_server(client_reader, server_writer));

    let server_reader = DnsBytesStream::new(server_reader)
        .from_err()
        .map(|dns| {
            let msg = trust_dns_proto::op::message::Message::from_vec(&*dns).unwrap();
            (dns, msg)
        })
        .inspect(|(dns, msg)| {
            let qname = msg.queries()[0].name().to_utf8();
            let mut msgs = config.message.lock().unwrap();
            match &*qname {
                "start.example." => {
                    msgs.truncate(0);
                }
                "end.example." => {
                    let mut tmp = Vec::default();
                    mem::swap(&mut tmp, &mut msgs);
                    tokio::spawn_async(print_error(write_sequence(
                        config.args.dump_sequences.clone(),
                        tmp,
                    )));
                }
                _ => {
                    msgs.push((dns.len() as u16, Instant::now()));
                }
            }
        });
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
    R: Stream<Item = Payload<Message>, Error = Error> + Send + Unpin,
    W: AsyncWrite,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(128 * 5);
    while let Some(dns) = await!(client.next()) {
        out.truncate(0);
        // write placeholder length, replaced later
        out.write_u16::<BigEndian>(0)?;
        match dns? {
            Payload::Payload(p) => {
                info!("Send payload");
                let mut encoder = BinEncoder::new(&mut out);
                encoder.set_offset(2);
                p.emit(&mut encoder)?;
            }
            Payload::Dummy => {
                info!("Send dummy");
                out.extend_from_slice(&DUMMY_DNS);
            }
        };
        let len = (out.len() - 2) as u16;
        BigEndian::write_u16(&mut out[..], len);

        total_bytes += out.len() as u64;
        await!(tokio::io::write_all(&mut server, &mut out))?;
        server.flush()?;
    }

    // We need to shutdown the endpoint before they are closed due to dropping one of the endpoints
    // hint: server and client access the same underlying TcpStream
    await!(shutdown(server))?;
    Ok(total_bytes)
}

async fn copy_server_to_client<R, W>(mut server: R, mut client: W) -> Result<(u64), Error>
where
    R: Stream<Item = (Vec<u8>, Message), Error = Error> + Send + Unpin,
    W: AsyncWrite,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(468 * 5);
    while let Some(x) = await!(server.next()) {
        let (dns, msg) = x?;

        // Remove all dummy messages from the responses
        if msg.id() == 47255 {
            info!("Received dummy");
            continue;
        }
        info!("Received payload");

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

async fn write_sequence(
    dir: Option<PathBuf>,
    mut sequence_raw: Vec<(u16, Instant)>,
) -> Result<(), Error> {
    if let Some(dir) = dir {
        let filepath = dir.join(format!(
            "sequence-{}.json",
            Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
        ));
        let mut file = await!(File::create(filepath.clone()))?;
        sequence_raw.sort_unstable_by_key(|x| x.1);
        let seq =
            Sequence::from_sizes_and_times(filepath.to_string_lossy().to_string(), &*sequence_raw)
                .unwrap();
        await!(io::write_all(
            &mut file,
            serde_json::to_string(&seq).unwrap()
        ))?;
        await!(io::flush(&mut file))?;
    }
    Ok(())
}
