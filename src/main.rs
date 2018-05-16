#![feature(try_from)]

extern crate chrono;
extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate log;
#[macro_use]
extern crate structopt;
extern crate misc_utils;
extern crate petgraph;
extern crate petgraph_graphml;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate serde_pickle;
extern crate url;

mod depgraph;

use chrome::*;
use chrono::{DateTime, Utc};
use depgraph::DepGraph;
use failure::Error;
use failure::ResultExt;
use misc_utils::fs::{file_open_read, file_open_write, WriteOptions};
use petgraph::prelude::*;
use petgraph_graphml::GraphMl;
use std::borrow::Cow;
use std::cmp;
use std::convert::TryFrom;
use std::fs::OpenOptions;
use std::path::PathBuf;
use structopt::StructOpt;
use url::Url;

#[derive(StructOpt, Debug)]
#[structopt(author = "", raw(setting = "structopt::clap::AppSettings::ColoredHelp"))]
struct CliArgs {
    #[structopt(parse(from_os_str))]
    webpage_log: PathBuf,
}

fn main() {
    use std::io::{self, Write};

    if let Err(err) = run() {
        let stderr = io::stderr();
        let mut out = stderr.lock();
        // cannot handle a write error here, we are already in the outermost layer
        let _ = writeln!(out, "An error occured:");
        for fail in err.causes() {
            let _ = writeln!(out, "  {}", fail);
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    // generic setup
    env_logger::init();
    let cli_args = CliArgs::from_args();

    let rdr = file_open_read(&cli_args.webpage_log).map_err(|err| {
        format_err!(
            "Opening input file '{}' failed: {}",
            cli_args.webpage_log.display(),
            err
        )
    })?;
    let messages: Vec<ChromeDebuggerMessage> = serde_json::from_reader(rdr)?;
    process_messages(&messages)?;

    Ok(())
}

fn process_messages(messages: &[ChromeDebuggerMessage]) -> Result<(), Error> {
    let mut depgraph = DepGraph::new(messages).context("Failure to build the graph.")?;
    depgraph.simplify_graph();
    depgraph.duplicate_domains();
    let domain_names = depgraph.get_domain_names();
    let graph = depgraph.into_graph();
    export_as_graphml(&graph)?;
    export_as_pickle(&graph)?;

    for domain in domain_names {
        println!("{}", domain);
    }

    Ok(())
}

fn export_as_graphml(graph: &Graph<RequestInfo, ()>) -> Result<(), Error> {
    let graphml = GraphMl::new(&graph).export_node_weights(Box::new(RequestInfo::graphml_support));
    let fname = PathBuf::from("res.graphml");
    let wtr = file_open_write(
        &fname,
        WriteOptions::default().set_open_options(OpenOptions::new().create(true).truncate(true)),
    ).map_err(|err| {
        format_err!("Opening input file '{}' failed: {}", &fname.display(), err)
    })?;
    graphml.to_writer(wtr)?;

    Ok(())
}

fn export_as_pickle(graph: &Graph<RequestInfo, ()>) -> Result<(), Error> {
    let fname = PathBuf::from("res.pickle");
    let mut wtr = file_open_write(
        &fname,
        WriteOptions::default().set_open_options(OpenOptions::new().create(true).truncate(true)),
    ).map_err(|err| {
        format_err!("Opening input file '{}' failed: {}", &fname.display(), err)
    })?;
    serde_pickle::to_writer(&mut wtr, graph, true)?;

    Ok(())
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct RequestInfo {
    normalized_domain_name: String,
    earliest_wall_time: DateTime<Utc>,
    requests: Vec<IndividualRequest>,
}

impl RequestInfo {
    /// Panics if `normalized_domain_name` is not equal
    fn merge_with(&mut self, other: &Self) {
        assert_eq!(self.normalized_domain_name, other.normalized_domain_name);

        self.requests.extend(other.requests.iter().cloned());
        self.earliest_wall_time = cmp::min(self.earliest_wall_time, other.earliest_wall_time);
    }

    pub fn graphml_support(&self) -> Vec<(Cow<'static, str>, Cow<str>)> {
        vec![
            ("domain_name".into(), (&*self.normalized_domain_name).into()),
            (
                "earliest_wall_time".into(),
                self.earliest_wall_time.to_string().into(),
            ),
            (
                "request_ids".into(),
                (format!(
                    "{:#?}",
                    self.requests
                        .iter()
                        .map(|r| &r.request_id)
                        .collect::<Vec<_>>()
                ).into()),
            ),
            (
                "urls".into(),
                (format!(
                    "{:#?}",
                    self.requests.iter().map(|r| &r.url).collect::<Vec<_>>()
                ).into()),
            ),
            (
                "wall_times".into(),
                (format!(
                    "{:#?}",
                    self.requests
                        .iter()
                        .map(|r| &r.wall_time)
                        .collect::<Vec<_>>()
                ).into()),
            ),
        ]
    }
}

impl<'a> TryFrom<&'a ChromeDebuggerMessage> for RequestInfo {
    type Error = Error;

    fn try_from(from: &'a ChromeDebuggerMessage) -> Result<Self, Error> {
        match *from {
            ChromeDebuggerMessage::NetworkRequestWillBeSent{
                request: Request { ref url, .. },
                ..
            } => {
                let parsed_url = Url::parse(&url).context("RequestInfo needs a domain name, but URL is not a valid URL.")?;
                let ndn = parsed_url
                    .host_str()
                    .ok_or_else(|| format_err!("The URL must have a domain part, but does not. URL: '{}'", parsed_url))?;
                let ind_req = IndividualRequest::try_from(from)?;
                Ok(RequestInfo{
                    normalized_domain_name: ndn.to_string(),
                    earliest_wall_time: ind_req.wall_time,
                    requests: vec![ind_req],
                })
           },
            _ => bail!("IndividualRequest can only be created from ChromeDebuggerMessage::NetworkRequestWillBeSent")
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
struct IndividualRequest {
    request_id: String,
    url: String,
    wall_time: DateTime<Utc>,
}

impl<'a> TryFrom<&'a ChromeDebuggerMessage> for IndividualRequest {
    type Error = Error;

    fn try_from(from: &'a ChromeDebuggerMessage) -> Result<Self, Error> {
        match *from {
            ChromeDebuggerMessage::NetworkRequestWillBeSent{
                request: Request { ref url, .. },
                ref request_id,
                wall_time,
                ..
            } => {
                Ok(IndividualRequest {
                    request_id: request_id.clone(),
                    url: url.clone(),
                    wall_time,
                })
            },
            _ => bail!("IndividualRequest can only be created from ChromeDebuggerMessage::NetworkRequestWillBeSent")
        }
    }
}

trait GraphExt {
    fn transitive_closure(&mut self);
}

impl<N, E, Ty, Ix> GraphExt for Graph<N, E, Ty, Ix>
where
    E: Default,
    Ty: ::petgraph::EdgeType,
    Ix: ::petgraph::csr::IndexType,
{
    fn transitive_closure(&mut self) {
        // based on https://github.com/bluss/petgraph/pull/151
        use petgraph::visit::{Dfs, IntoNodeIdentifiers};

        let mut dfs = Dfs::empty(&*self);

        for node in self.node_identifiers() {
            dfs.reset(&*self);
            dfs.move_to(node);
            self.update_edge(node, node, E::default());
            while let Some(visited) = dfs.next(&*self) {
                self.update_edge(node, visited, E::default());
            }
        }
    }
}

pub mod chrome {
    use chrono::{DateTime, Utc};

    #[serde(tag = "method", content = "params")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub enum ChromeDebuggerMessage {
        // Everything Network
        #[serde(rename = "Network.requestWillBeSent", rename_all = "camelCase")]
        NetworkRequestWillBeSent {
            #[serde(rename = "documentURL")]
            document_url: String,
            request_id: String,
            request: Request,
            initiator: Initiator,
            redirect_response: Option<RedirectResponse>,
            #[serde(deserialize_with = "::deserialize::timestamp")]
            wall_time: DateTime<Utc>,
        },
        #[serde(rename = "Network.requestServedFromCache", rename_all = "camelCase")]
        NetworkRequestServedFromCache { request_id: String },
        #[serde(rename = "Network.responseReceived", rename_all = "camelCase")]
        NetworkResponseReceived { request_id: String },
        #[serde(rename = "Network.resourceChangedPriority", rename_all = "camelCase")]
        NetworkResourceChangedPriority { request_id: String },
        #[serde(rename = "Network.loadingFailed", rename_all = "camelCase")]
        NetworkLoadingFailed { request_id: String },
        #[serde(rename = "Network.dataReceived", rename_all = "camelCase")]
        NetworkDataReceived { request_id: String },
        #[serde(rename = "Network.loadingFinished", rename_all = "camelCase")]
        NetworkLoadingFinished { request_id: String },

        // Everything Target
        #[serde(rename = "Target.targetCreated", rename_all = "camelCase")]
        TargetTargetCreated {},
        #[serde(rename = "Target.targetInfoChanged", rename_all = "camelCase")]
        TargetTargetInfoChanged {},
        #[serde(rename = "Target.targetDestroyed", rename_all = "camelCase")]
        TargetTargetDestroyed {},
        #[serde(rename = "Target.attachedToTarget", rename_all = "camelCase")]
        TargetAttachedToTarget {},

        // Everything Debugger
        #[serde(rename = "Debugger.scriptParsed", rename_all = "camelCase")]
        DebuggerScriptParsed {
            script_id: String,
            url: String,
            stack_trace: Option<StackTrace>,
        },
        #[serde(rename = "Debugger.scriptFailedToParse", rename_all = "camelCase")]
        DebuggerScriptFailedToParse {
            script_id: String,
            url: String,
            stack_trace: Option<StackTrace>,
        },
    }

    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct Request {
        pub url: String,
        pub headers: Headers,
    }

    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct Headers {
        #[serde(rename = "Referer")]
        pub referer: Option<String>,
    }

    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct RedirectResponse {
        pub url: String,
    }

    #[serde(tag = "type", rename_all = "lowercase")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub enum Initiator {
        Other {},
        Parser { url: String },
        Script { stack: StackTrace },
    }

    #[serde(rename_all = "camelCase")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct StackTrace {
        pub call_frames: Vec<CallFrame>,
        pub parent: Option<Box<StackTrace>>,
    }

    #[serde(rename_all = "camelCase")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct CallFrame {
        pub url: String,
        pub script_id: String,
    }
}

mod deserialize {
    use chrono::{DateTime, NaiveDateTime, Utc};
    use serde::de::{Deserializer, Error, Unexpected, Visitor};

    /// Deserialize a Unix timestamp with optional subsecond precision into a `DateTime<Utc>`.
    pub fn timestamp<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper;
        impl<'de> Visitor<'de> for Helper {
            type Value = DateTime<Utc>;

            fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                formatter.write_str(
                    "Invalid timestamp. Must be in the form of '123456789' or '123456789.123456'.",
                )
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let ndt = NaiveDateTime::from_timestamp_opt(value, 0);
                if let Some(ndt) = ndt {
                    Ok(DateTime::<Utc>::from_utc(ndt, Utc))
                } else {
                    Err(Error::custom(format!(
                        "Invalid or out of range value '{}' for NaiveDateTime",
                        value
                    )))
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let ndt = NaiveDateTime::from_timestamp_opt(value as i64, 0);
                if let Some(ndt) = ndt {
                    Ok(DateTime::<Utc>::from_utc(ndt, Utc))
                } else {
                    Err(Error::custom(format!(
                        "Invalid or out of range value '{}' for NaiveDateTime",
                        value
                    )))
                }
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let seconds = value.trunc() as i64;
                let nsecs = (value.fract() * 1_000_000_000_f64) as u32;
                let ndt = NaiveDateTime::from_timestamp_opt(seconds, nsecs);
                if let Some(ndt) = ndt {
                    Ok(DateTime::<Utc>::from_utc(ndt, Utc))
                } else {
                    Err(Error::custom(format!(
                        "Invalid or out of range value '{}' for NaiveDateTime",
                        value
                    )))
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let parts: Vec<_> = value.split('.').collect();

                match *parts.as_slice() {
                    [seconds] => {
                        if let Ok(seconds) = i64::from_str_radix(seconds, 10) {
                            let ndt = NaiveDateTime::from_timestamp_opt(seconds, 0);
                            if let Some(ndt) = ndt {
                                Ok(DateTime::<Utc>::from_utc(ndt, Utc))
                            } else {
                                Err(Error::custom(format!(
                                    "Invalid or out of range value '{}' for NaiveDateTime",
                                    value
                                )))
                            }
                        } else {
                            Err(Error::invalid_value(Unexpected::Str(value), &self))
                        }
                    }
                    [seconds, subseconds] => {
                        if let Ok(seconds) = i64::from_str_radix(seconds, 10) {
                            let subseclen = subseconds.chars().count() as u32;
                            if let Ok(mut subseconds) = u32::from_str_radix(subseconds, 10) {
                                // convert subseconds to nanoseconds (10^-9), require 9 places for nanoseconds
                                subseconds *= 10u32.pow(9 - subseclen);
                                let ndt = NaiveDateTime::from_timestamp_opt(seconds, subseconds);
                                if let Some(ndt) = ndt {
                                    Ok(DateTime::<Utc>::from_utc(ndt, Utc))
                                } else {
                                    Err(Error::custom(format!(
                                        "Invalid or out of range value '{}' for NaiveDateTime",
                                        value
                                    )))
                                }
                            } else {
                                Err(Error::invalid_value(Unexpected::Str(value), &self))
                            }
                        } else {
                            Err(Error::invalid_value(Unexpected::Str(value), &self))
                        }
                    }

                    _ => Err(Error::invalid_value(Unexpected::Str(value), &self)),
                }
            }
        }

        deserializer.deserialize_any(Helper)
    }
}
