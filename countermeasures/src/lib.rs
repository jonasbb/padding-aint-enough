#![deny(rust_2018_compatibility)]
#![warn(rust_2018_idioms)]
// enable async/await support
#![feature(async_await)]
// // only needed to manually implement a std future:
// #![feature(arbitrary_self_types)]
#![feature(duration_float)]

mod adaptive_padding;
mod constant_rate;
mod dns_tcp;
mod ensure_padding;
mod error;
mod pass_through;
mod streams;

pub use crate::{
    adaptive_padding::AdaptivePadding,
    constant_rate::ConstantRate,
    dns_tcp::DnsBytesStream,
    ensure_padding::EnsurePadding,
    error::Error,
    pass_through::PassThrough,
    streams::{MyStream, MyTcpStream, TokioOpensslStream},
};
use failure::Fail;
use futures::Stream;
use log::{error, warn};
use std::{
    fmt::{self, Display},
    fs::OpenOptions,
    io::Write,
    net::{SocketAddr, ToSocketAddrs},
    path::Path,
    str::FromStr,
    sync::Mutex,
    time::Duration,
};
use structopt::StructOpt;
use tokio_timer::throttle::Throttle;

/// Self Signed server certificate in PEM format
pub const SERVER_CERT: &[u8] = include_bytes!("../cert.pem");
/// Private key for the certificate [`SERVER_CERT`]
pub const SERVER_KEY: &[u8] = include_bytes!("../key.pem");

/// Configuration for different sending strategies
#[derive(Clone, Debug, StructOpt)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
pub enum Strategy {
    /// Apply no defense mechanism
    #[structopt(
        name = "pass",
        global_settings(&[
            structopt::clap::AppSettings::ColoredHelp,
            structopt::clap::AppSettings::VersionlessSubcommands
        ])
    )]
    PassThrough,
    /// Use Constant Rate
    #[structopt(global_settings(&[
        structopt::clap::AppSettings::ColoredHelp,
        structopt::clap::AppSettings::VersionlessSubcommands
    ]))]
    Constant {
        /// The rate in which packets are send specified in ms between them
        #[structopt(parse(try_from_str = parse_duration_ms))]
        rate: Duration,
    },
    /// Use AdaptivePadding
    #[structopt(
        name = "ap",
        global_settings(&[
            structopt::clap::AppSettings::ColoredHelp,
            structopt::clap::AppSettings::VersionlessSubcommands
        ])
    )]
    AdaptivePadding {
        /// Throttle the connection to at most 1 real packet every `throttle-in` ms
        #[structopt(long = "tin", parse(try_from_str = parse_duration_ms))]
        throttle_in: Option<Duration>,
        /// Throttle the connection to at most 1 outgoing packet every `throttle-out` ms
        #[structopt(long = "tout", parse(try_from_str = parse_duration_ms))]
        throttle_out: Option<Duration>,
    },
}

/// Parse a string as [`f64`], interpret it as milliseconds, and return a [`Duration`]
pub fn parse_duration_ms(s: &str) -> Result<Duration, std::num::ParseFloatError> {
    let ms: f64 = s.parse()?;
    Ok(Duration::from_micros((ms * 1000.).round() as u64))
}

/// Extension around [`SocketAddr`] and [`ToSocketAddrs`] which additionally stores the hostname
///
/// The hostname is an important feature for TLS (e.g., SNI and cert validity), therefore only a [`SocketAddr`] is often not enough
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum HostnameSocketAddr {
    Hostname {
        full_addr_string: String,
        hostname_length: usize,
        socket_addrs: Vec<SocketAddr>,
    },
    Ip([SocketAddr; 1]),
}

impl HostnameSocketAddr {
    pub fn hostname(&self) -> String {
        use HostnameSocketAddr::*;
        match self {
            Hostname {
                full_addr_string,
                hostname_length,
                ..
            } => full_addr_string[..*hostname_length].to_string(),
            Ip(ip) => ip[0].ip().to_string(),
        }
    }

    pub fn port(&self) -> u16 {
        self.socket_addr().port()
    }

    pub fn socket_addr(&self) -> SocketAddr {
        use HostnameSocketAddr::*;
        match self {
            Hostname { socket_addrs, .. } => socket_addrs[0],
            Ip(ip) => ip[0],
        }
    }

    pub fn socket_addrs(&self) -> &[SocketAddr] {
        use HostnameSocketAddr::*;
        match self {
            Hostname { socket_addrs, .. } => &socket_addrs,
            Ip(ip) => ip,
        }
    }
}

impl Display for HostnameSocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        use HostnameSocketAddr::*;
        match self {
            Hostname {
                full_addr_string,
                socket_addrs,
                ..
            } => {
                write!(f, "{} (", full_addr_string)?;
                let mut first = true;
                for addr in socket_addrs {
                    write!(f, "{}{}", if first { "" } else { ", " }, addr.ip())?;
                    first = false;
                }
                write!(f, ")")
            }
            Ip(ip) => write!(f, "{}", ip[0]),
        }
    }
}

impl FromStr for HostnameSocketAddr {
    // TODO fix error type
    type Err = String;

    fn from_str(addr: &str) -> Result<Self, Self::Err> {
        use HostnameSocketAddr::*;

        // Test if the `addr` is directly convertable to a SocketAddr, then it is an IP address
        if let Ok(addr) = addr.parse() {
            return Ok(Ip([addr]));
        }

        let parts: Vec<_> = addr.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return Err("Missing Port number".into());
        }
        let socket_addrs: Vec<_> = addr
            .to_socket_addrs()
            .map_err(|err| err.to_string())?
            .collect();
        if socket_addrs.is_empty() {
            return Err("The list of SocketAddrs is empty".into());
        }
        Ok(Hostname {
            full_addr_string: addr.to_string(),
            hostname_length: parts[1].len(),
            socket_addrs,
        })
    }
}

#[cfg(test)]
mod test_hostname_socket_add {
    use super::HostnameSocketAddr;
    use std::net::*;

    #[test]
    fn test_ip_address() {
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let addr1_hostname = "127.0.0.1";
        let addr1_str_clean = "127.0.0.1:8080";
        let addr1_str_1 = "127.000.000.001:8080";
        let addr1_str_2 = "127.000.0.01:08080";

        let hsa: HostnameSocketAddr = addr1_str_clean.parse().unwrap();
        assert_eq!(addr1_hostname, hsa.hostname());
        assert_eq!(addr1, hsa.socket_addr());
        assert_eq!(&[addr1], hsa.socket_addrs());
        let hsa: HostnameSocketAddr = addr1_str_1.parse().unwrap();
        assert_eq!(addr1_hostname, hsa.hostname());
        assert_eq!(addr1, hsa.socket_addr());
        assert_eq!(&[addr1], hsa.socket_addrs());
        let hsa: HostnameSocketAddr = addr1_str_2.parse().unwrap();
        assert_eq!(addr1_hostname, hsa.hostname());
        assert_eq!(addr1, hsa.socket_addr());
        assert_eq!(&[addr1], hsa.socket_addrs());

        let addr2 = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(
                0xfe80, 0x0123, 0x4567, 0x89ab, 0xcdef, 0x0, 0x0, 0x53,
            )),
            853,
        );
        let addr2_hostname = "fe80:123:4567:89ab:cdef::53";
        let addr2_str_clean = "[fe80:123:4567:89ab:cdef::53]:853";
        let addr2_str_1 = "[fe80:0123:4567:89ab:cdef::0053]:0853";
        let addr2_str_2 = "[fe80:0123:4567:89ab:cdef:0:0:0053]:0853";

        let hsa: HostnameSocketAddr = addr2_str_clean.parse().unwrap();
        assert_eq!(addr2_hostname, hsa.hostname());
        assert_eq!(addr2, hsa.socket_addr());
        assert_eq!(&[addr2], hsa.socket_addrs());
        let hsa: HostnameSocketAddr = addr2_str_1.parse().unwrap();
        assert_eq!(addr2_hostname, hsa.hostname());
        assert_eq!(addr2, hsa.socket_addr());
        assert_eq!(&[addr2], hsa.socket_addrs());
        let hsa: HostnameSocketAddr = addr2_str_2.parse().unwrap();
        assert_eq!(addr2_hostname, hsa.hostname());
        assert_eq!(addr2, hsa.socket_addr());
        assert_eq!(&[addr2], hsa.socket_addrs());

        // Parsing without port should not work
        assert!(addr1_hostname.parse::<HostnameSocketAddr>().is_err());
        assert!(addr2_hostname.parse::<HostnameSocketAddr>().is_err());
    }

    #[test]
    fn test_simple_network() {
        let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 53);
        let addr1_hostname = "www.1-2-3-4.sslip.io";
        let addr1_str_clean = "www.1-2-3-4.sslip.io:53";
        let addr1_str_1 = "www.1-2-3-4.sslip.io:0053";

        let hsa: HostnameSocketAddr = addr1_str_clean.parse().unwrap();
        assert_eq!(addr1_hostname, hsa.hostname());
        assert_eq!(addr1, hsa.socket_addr());
        assert_eq!(&[addr1], hsa.socket_addrs());
        let hsa: HostnameSocketAddr = addr1_str_1.parse().unwrap();
        assert_eq!(addr1_hostname, hsa.hostname());
        assert_eq!(addr1, hsa.socket_addr());
        assert_eq!(&[addr1], hsa.socket_addrs());

        let addr2 = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(
                0x2001, 0x0123, 0x4567, 0x89ab, 0xcdef, 0x0, 0x0, 0x53,
            )),
            443,
        );
        let addr2_hostname = "m.2001-123-4567-89ab-cdef--53.sslip.io";
        let addr2_str_clean = "m.2001-123-4567-89ab-cdef--53.sslip.io:443";
        let addr2_str_1 = "m.2001-123-4567-89ab-cdef--53.sslip.io:00443";

        let hsa: HostnameSocketAddr = addr2_str_clean.parse().unwrap();
        assert_eq!(addr2_hostname, hsa.hostname());
        assert_eq!(addr2, hsa.socket_addr());
        assert_eq!(&[addr2], hsa.socket_addrs());
        let hsa: HostnameSocketAddr = addr2_str_1.parse().unwrap();
        assert_eq!(addr2_hostname, hsa.hostname());
        assert_eq!(addr2, hsa.socket_addr());
        assert_eq!(&[addr2], hsa.socket_addrs());

        // Parsing without port should not work
        assert!(addr1_hostname.parse::<HostnameSocketAddr>().is_err());
        assert!(addr2_hostname.parse::<HostnameSocketAddr>().is_err());
    }
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
    E: Fail,
{
    use std::fmt::Write;

    if let Err(err) = future.await {
        let mut msg = String::new();
        for fail in Fail::iter_chain(&err) {
            let _ = writeln!(&mut msg, "{}", fail);
        }
        if let Some(backtrace) = err.backtrace() {
            let _ = writeln!(&mut msg, "{}", backtrace);
        };
        error!("{}", msg);
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

impl<T> Payload<Result<T, Error>> {
    pub fn transpose_error(self) -> Result<Payload<T>, Error> {
        match self {
            Payload::Payload(Ok(p)) => Ok(Payload::Payload(p)),
            Payload::Payload(Err(e)) => Err(e),
            Payload::Dummy => Ok(Payload::Dummy),
        }
    }
}

pub fn wrap_stream<S, T>(
    stream: S,
    strategy: &Strategy,
) -> impl Stream<Item = Payload<T>> + Send + Unpin
where
    S: Stream<Item = T> + Send + Unpin + 'static,
    T: Send + Sync + Unpin + 'static,
{
    match strategy {
        Strategy::PassThrough => {
            Box::new(PassThrough::new(stream)) as Box<dyn Stream<Item = _> + Send + Unpin>
        }
        Strategy::Constant { rate, .. } => Box::new(ConstantRate::new(stream, *rate)),
        Strategy::AdaptivePadding {
            throttle_in,
            throttle_out,
            ..
        } => match (*throttle_in, *throttle_out) {
            (Some(tin), Some(tout)) => Box::new(Throttle::new(
                AdaptivePadding::new(Throttle::new(stream, tin)),
                tout,
            )) as Box<dyn Stream<Item = _> + Send + Unpin>,
            (Some(tin), None) => Box::new(AdaptivePadding::new(Throttle::new(stream, tin))),
            (None, Some(tout)) => Box::new(Throttle::new(AdaptivePadding::new(stream), tout)),
            (None, None) => Box::new(AdaptivePadding::new(stream)),
        },
    }
}

/// Specify the transport protocol to be used while connecting to a remote endpoint
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Transport {
    /// Use TCP
    Tcp,
    /// Use TLS
    Tls,
}
