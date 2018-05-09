#![feature(try_from)]

extern crate env_logger;
#[macro_use]
extern crate failure;
#[macro_use]
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

mod depgraph;

use chrome::*;
use depgraph::DepGraph;
use failure::Error;
use failure::ResultExt;
use misc_utils::fs::{file_open_read, file_open_write, WriteOptions};
use petgraph::graphml::{Config as GraphMLConfig, GraphML};
use petgraph::prelude::*;
use std::borrow::Cow;
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
    let mut depgraph = DepGraph::new();
    depgraph
        .process_messages(messages)
        .context("Failure to build the graph.")?;
    depgraph.simplify_graph();
    depgraph.duplicate_domains();
    let graph = depgraph.into_graph();
    export_as_graphml(&graph)?;

    Ok(())
}

fn export_as_graphml(graph: &Graph<RequestInfo, ()>) -> Result<(), Error> {
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
    wtr.write_all(
        graphml
            .to_string_with_weight_functions(|_ew| vec![], RequestInfo::graphml_support)
            .as_bytes(),
    )?;

    Ok(())
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct RequestInfo {
    normalized_domain_name: String,
    requests: Vec<IndividualRequest>,
}

impl RequestInfo {
    /// Panics if `normalized_domain_name` is not equal
    fn merge_with(&mut self, other: &Self) {
        assert_eq!(self.normalized_domain_name, other.normalized_domain_name);

        self.requests.extend(other.requests.iter().cloned());
    }

    pub fn graphml_support(&self) -> Vec<(String, Cow<str>)> {
        vec![
            ("domain_name".into(), (&*self.normalized_domain_name).into()),
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
                let parsed_url;
                let ndn = if url.starts_with("data:") {
                    "data"
                } else {
                    parsed_url = Url::parse(&url).context("RequestInfo needs a domain name, but URL is not a valid URL.")?;
                    parsed_url
                        .host_str()
                        .ok_or_else(|| format_err!("The URL must have a domain part, but does not. URL: '{}'", parsed_url))?
                };
                Ok(RequestInfo{
                    normalized_domain_name: ndn.to_string(),
                    requests: vec![IndividualRequest::try_from(from)?],
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

impl<'a> TryFrom<&'a ChromeDebuggerMessage> for IndividualRequest {
    type Error = Error;

    fn try_from(from: &'a ChromeDebuggerMessage) -> Result<Self, Error> {
        match *from {
            ChromeDebuggerMessage::NetworkRequestWillBeSent{
                request: Request { ref url, .. },
                ref request_id,
                ..
            } => {
                Ok(IndividualRequest {
                    request_id: request_id.clone(),
                    url: url.clone(),
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
    #[serde(tag = "method", content = "params")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub enum ChromeDebuggerMessage {
        // Everything Network
        #[serde(rename = "Network.requestWillBeSent", rename_all = "camelCase")]
        NetworkRequestWillBeSent {
            request_id: String,
            request: Request,
            initiator: Initiator,
            redirect_response: Option<RedirectResponse>,
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

        // Everything Debugger
        #[serde(rename = "Debugger.scriptParsed", rename_all = "camelCase")]
        DebuggerScriptParsed {},
        #[serde(rename = "Debugger.scriptFailedToParse", rename_all = "camelCase")]
        DebuggerScriptFailedToParse {},
    }

    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct Request {
        pub url: String,
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
        Script { stack: Script },
    }

    #[serde(rename_all = "camelCase")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct Script {
        pub call_frames: Vec<CallFrame>,
        pub parent: Option<Box<Script>>,
    }

    #[serde(rename_all = "camelCase")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct CallFrame {
        pub url: String,
        pub script_id: String,
    }
}
