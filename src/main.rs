#![feature(try_from)]

extern crate env_logger;
#[macro_use]
extern crate failure;
// #[macro_use]
extern crate log;
#[macro_use]
extern crate structopt;
extern crate misc_utils;
extern crate petgraph;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate url;

use chrome::*;
use failure::Error;
use failure::ResultExt;
use misc_utils::fs::{file_open_read, file_open_write, WriteOptions};
use petgraph::graphml::{Config as GraphMLConfig, GraphML};
use petgraph::prelude::*;
use std::collections::HashMap;
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
    // println!("{:#?}", messages);

    Ok(())
}

fn process_messages(messages: &[ChromeDebuggerMessage]) -> Result<(), Error> {
    let mut graph: Graph<_, u8> = Graph::new();
    let mut nodes_cache: HashMap<String, NodeIndex> = HashMap::new();

    for message in messages {
        use chrome::ChromeDebuggerMessage::*;
        if let NetworkRequestWillBeSent {
            request: Request { url, .. },
            ..
        } = message
        {
            let _entry = nodes_cache
                .entry(url.clone())
                .or_insert_with(|| graph.add_node(url.clone()));
        }
    }

    let graphml = GraphML::with_config(
        &graph,
        GraphMLConfig::new()
            .export_node_weights(true)
            .export_edge_weights(true),
    );
    let fname = PathBuf::from("res.graphml");
    let mut wtr = file_open_write(
        &fname,
        WriteOptions::default().set_open_options(OpenOptions::new().create(true).truncate(true)),
    ).map_err(|err| {
        format_err!("Opening input file '{}' failed: {}", &fname.display(), err)
    })?;
    wtr.write_all(graphml.to_string().as_bytes())?;

    Ok(())
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
struct RequestInfo {
    normalized_domain_name: String,
    requests: Vec<IndividualRequest>,
}

impl<'a> TryFrom<&'a ChromeDebuggerMessage> for RequestInfo {
    type Error = Error;

    fn try_from(from: &'a ChromeDebuggerMessage) -> Result<Self, Error> {
        match *from {
            ChromeDebuggerMessage::NetworkRequestWillBeSent{
                request: Request { ref url, .. },
                ref request_id,
                ..
            } => {
                let u = Url::parse(&url).context("RequestInfo needs a domain name, but URL is not a valid URL.")?;
                let ndn = u
                    .host_str()
                    .ok_or_else(|| format_err!("The URL must have a domain part, but does not. URL: '{}'", u))?;
                Ok(RequestInfo{
                    normalized_domain_name: ndn.to_string(),
                    requests: vec![IndividualRequest {
                        request_id: request_id.clone(),
                        url: url.clone()
                    }]
                })
           },
            _ => bail!("IndividualRequest can only be created from ChromeDebuggerMessage::NetworkRequestWillBeSent")
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
struct IndividualRequest {
    request_id: String,
    url: String,
}

// impl<'a> TryFrom<&'a ChromeDebuggerMessage> for IndividualRequest {
//     type Error = Error;

//     fn try_from(from: &'a ChromeDebuggerMessage) -> Result<Self, Error> {
//         match *from {
//             ChromeDebuggerMessage::NetworkRequestWillBeSent{request: Request { ref url, .. },
//             ref request_id,
//             ..} => {Ok(IndividualRequest {
//                 request_id: request_id.clone(),
//                 url: url.clone(),
//             })},
//             _ => bail!("IndividualRequest can only be created from ChromeDebuggerMessage::NetworkRequestWillBeSent")
//         }
//     }
// }

pub mod chrome {
    // TODO missing support for redirects
    #[cfg_attr(feature = "cargo-clippy", allow(enum_variant_names))]
    #[serde(tag = "method", content = "params")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub enum ChromeDebuggerMessage {
        #[serde(rename = "Network.requestWillBeSent", rename_all = "camelCase")]
        NetworkRequestWillBeSent {
            request_id: String,
            request: Request,
            initiator: Initiator,
        },
        #[serde(rename = "Network.requestServedFromCache", rename_all = "camelCase")]
        NetworkRequestServedFromCache { request_id: String },
        #[serde(rename = "Network.responseReceived", rename_all = "camelCase")]
        NetworkResponseReceived { request_id: String },
        #[serde(rename = "Network.resourceChangedPriority", rename_all = "camelCase")]
        NetworkResourceChangedPriority { request_id: String },
        #[serde(rename = "Network.loadingFailed", rename_all = "camelCase")]
        NetworkLoadingFailed { request_id: String },
    }

    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct Request {
        pub url: String,
    }

    #[serde(tag = "type", rename_all = "lowercase")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub enum Initiator {
        Other {},
        Parser { url: String },
        Script { stack: Script },
    }

    #[serde(rename_all = "camelCase")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct Script {
        call_frames: Vec<CallFrame>,
        parent: Option<Box<Script>>,
    }

    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct CallFrame {
        pub url: String,
    }
}
