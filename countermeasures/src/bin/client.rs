#![deny(rust_2018_compatibility)]
#![warn(rust_2018_idioms)]

use byteorder::{BigEndian, ByteOrder, WriteBytesExt};
use chrono::{SecondsFormat, Utc};
use failure::Fail;
use futures::{future, Stream, StreamExt};
use log::{info, trace, warn};
use openssl::{
    pkey::PKey,
    ssl::{SslAcceptor, SslConnector, SslMethod, SslOptions, SslVerifyMode, SslVersion},
    x509::X509,
};
use sequences::{load_sequence::convert_to_sequence, AbstractQueryResponse, LoadSequenceConfig};
use std::{
    io::Write,
    mem,
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
    fs::File,
    io::{AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
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
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
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

    #[structopt(subcommand)]
    strategy: Strategy,
}

// #[derive(Debug)]
struct Config {
    args: CliArgs,
    message: Mutex<Vec<AbstractQueryResponse>>,
    transport: Transport,
    acceptor: Option<SslAcceptor>,
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
    let log_settings = "client=debug,tlsproxy=debug";
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_settings))
        .format_timestamp_nanos()
        .init();
    openssl_probe::init_ssl_cert_env_vars();
    let cli_args = CliArgs::from_args();
    eprintln!("{:?}", cli_args);
    if let Some(file) = &cli_args.sslkeylogfile {
        std::env::set_var("SSLKEYLOGFILE", file.to_path_buf());
    }

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async_run(cli_args))
}

async fn async_run(cli_args: CliArgs) -> Result<(), Error> {
    // Create a TCP listener which will listen for incoming connections.
    let socket = TcpListener::bind(&cli_args.listen).await?;
    println!(
        "Listening on: {}\nProxying to: {}\n",
        cli_args.listen, cli_args.server
    );

    let transport = if cli_args.tcp {
        Transport::Tcp
    } else if cli_args.tls {
        Transport::Tls
    } else {
        Transport::Tcp
    };

    let acceptor = if transport == Transport::Tls {
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls())?;
        acceptor.set_verify(SslVerifyMode::NONE);
        acceptor.set_certificate(X509::from_pem(SERVER_CERT)?.as_ref())?;
        acceptor.set_private_key(PKey::private_key_from_pem(SERVER_KEY)?.as_ref())?;
        if let Some(logfile) = &cli_args.sslkeylogfile {
            let cb = tlsproxy::keylog_to_file(logfile.clone());
            acceptor.set_keylog_callback(cb);
        }
        Some(acceptor.build())
    } else {
        None
    };

    let config: Arc<Config> = Arc::new(Config {
        args: cli_args,
        message: Mutex::default(),
        transport,
        acceptor,
    });
    let done = socket
        .incoming()
        // conver the Error to tlsproxy::Error
        .map(|x| Ok(x?))
        .for_each_concurrent(100, move |client| {
            tokio::spawn(print_error(handle_client(config.clone(), client)));
            future::ready(())
        });
    done.await;
    Ok(())
}

async fn handle_client(config: Arc<Config>, client: Result<TcpStream, Error>) -> Result<(), Error> {
    let client = client?;
    client.set_nodelay(true)?;

    let server_socket_addr = config.args.server.socket_addr();
    let server = TcpStream::connect(&server_socket_addr).await?;
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
    let connector_config = connector.configure()?;
    let hostname = &config.args.server.hostname();
    let server = tokio_openssl::connect(connector_config, hostname, server).await?;

    // Create separate read/write handles for the TCP clients that we're
    // proxying data between. Note that typically you'd use
    // `AsyncRead::split` for this operation, but we want our writer
    // handles to have a custom implementation of `shutdown` which
    // actually calls `TcpStream::shutdown` to ensure that EOF is
    // transmitted properly across the proxied connection.
    //
    // As a result, we wrap up our client/server manually in arcs and
    // use the impls below on our custom `MyTcpStream` type.
    let client_reader: MyStream<_> = match config.transport {
        Transport::Tcp => MyTcpStream::new(Arc::new(Mutex::new(client))).into(),
        Transport::Tls => TokioOpensslStream::new(Arc::new(Mutex::new({
            let acceptor = &config.acceptor.clone().unwrap();
            tokio_openssl::accept(acceptor, client).await?
        })))
        .into(),
    };
    let client_writer = client_reader.clone();
    let server_reader = TokioOpensslStream::new(Arc::new(Mutex::new(server)));
    let server_writer = server_reader.clone();

    // Copy the data (in parallel) between the client and the server.
    // After the copy is done we indicate to the remote side that we've
    // finished by shutting down the connection.
    let client_reader = DnsBytesStream::new(client_reader);
    let client_reader = EnsurePadding::new(client_reader);
    let client_reader = wrap_stream(client_reader, &config.args.strategy);
    let client_to_server = copy_client_to_server(client_reader, server_writer);

    let server_reader = DnsBytesStream::new(server_reader)
        .map(|dns| {
            let dns = dns?;
            let msg = trust_dns_proto::op::message::Message::from_vec(&*dns).unwrap();
            Ok((dns, msg))
        })
        .inspect(|x| {
            if let Ok((dns, msg)) = x {
                let qname = msg.queries()[0].name().to_utf8();
                let mut msgs = config.message.lock().unwrap();
                match &*qname {
                    "start.example." => {
                        msgs.truncate(0);
                    }
                    "end.example." => {
                        let mut tmp = Vec::default();
                        mem::swap(&mut tmp, &mut msgs);
                        tokio::spawn(print_error(write_sequence(
                            config.args.dump_sequences.clone(),
                            tmp,
                        )));
                    }
                    _ => {
                        msgs.push(AbstractQueryResponse {
                            time: Utc::now().naive_utc(),
                            size: dns.len() as _,
                        });
                    }
                }
            }
        });
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
    R: Stream<Item = Payload<Result<Message, Error>>> + Send + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(128 * 5);
    while let Some(dns) = client.next().await {
        out.truncate(0);
        // write placeholder length, replaced later
        out.write_u16::<BigEndian>(0)?;
        match dns.transpose_error()? {
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
    R: Stream<Item = Result<(Vec<u8>, Message), Error>> + Send + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut total_bytes = 0;

    let mut out = Vec::with_capacity(468 * 5);
    while let Some(x) = server.next().await {
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

async fn write_sequence(
    dir: Option<PathBuf>,
    mut sequence_raw: Vec<AbstractQueryResponse>,
) -> Result<(), Error> {
    if let Some(dir) = dir {
        let filepath = dir.join(format!(
            "sequence-{}.json",
            Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
        ));
        let mut file = File::create(filepath.clone()).await?;
        sequence_raw.sort_unstable_by_key(|x| x.time);
        let seq = convert_to_sequence(
            &*sequence_raw,
            filepath.to_string_lossy().to_string(),
            LoadSequenceConfig::default(),
        )
        .unwrap();
        let content = serde_json::to_string(&seq).unwrap();
        AsyncWriteExt::write_all(&mut file, content.as_ref()).await?;
        file.flush().await?;
    }
    Ok(())
}
