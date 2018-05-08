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
use std::cell::RefCell;
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
    let mut graph: Graph<_, ()> = Graph::new();
    let mut nodes_cache: HashMap<String, NodeIndex> = HashMap::new();

    // Insert a node for "other" type dependencies
    nodes_cache.entry("other".to_string()).or_insert_with(|| {
        graph.add_node(RequestInfo {
            normalized_domain_name: "other".into(),
            requests: Vec::new(),
        })
    });

    {
        let graph = RefCell::new(&mut graph);
        let nodes_cache = RefCell::new(&mut nodes_cache);

        // Create a new node and add it to the node cache
        let create_node = |msg: &ChromeDebuggerMessage| -> Result<NodeIndex, Error> {
            if let ChromeDebuggerMessage::NetworkRequestWillBeSent {
                request: Request { ref url, .. },
                ..
            } = *msg
            {
                let mut graph = graph.borrow_mut();
                let mut nodes_cache = nodes_cache.borrow_mut();

                let entry = nodes_cache.entry(url.clone()).or_insert_with(|| {
                    graph.add_node(RequestInfo::try_from(msg).expect(
                        "A requestWillBeSent must always be able to generate a valid node.",
                    ))
                });
                Ok(*entry)
            } else {
                bail!("Cannot create node from this message type.")
            }
        };
        // Find an existing node in the node cache by the URL
        let find_node = |url: String| -> Result<NodeIndex, Error> {
            let nodes_cache = nodes_cache.borrow();

            match nodes_cache.get(&*url) {
                Some(node) => Ok(*node),
                // TODO this probably needs better error handling
                // Also see https://projects.cispa.saarland/bushart/encrypted-dns/issues/3
                None => bail!("Cannot find node in cache even though there is a dependency to it"),
            }
        };
        let add_dependency = |from: NodeIndex, to: NodeIndex| {
            let mut graph = graph.borrow_mut();

            graph.update_edge(from, to, ());
        };

        for message in messages {
            use ChromeDebuggerMessage::NetworkRequestWillBeSent;
            if let NetworkRequestWillBeSent { initiator, .. } = message {
                let node = create_node(&message)?;

                // Add dependencies for node/msg combination
                match initiator {
                    Initiator::Other {} => {
                        let other = find_node("other".into())?;
                        add_dependency(node, other);
                    }
                    Initiator::Parser { ref url } => {
                        let other = find_node(url.clone())?;
                        add_dependency(node, other);
                    }
                    Initiator::Script { ref stack } => {
                        fn traverse_stack<FN, AD>(
                            node: NodeIndex,
                            stack: &Script,
                            find_node: FN,
                            add_dependency: AD,
                        ) -> Result<(), Error>
                        where
                            FN: Fn(String) -> Result<NodeIndex, Error>,
                            AD: Fn(NodeIndex, NodeIndex),
                        {
                            for frame in &stack.call_frames {
                                let other = find_node(frame.url.clone())?;
                                add_dependency(node, other);
                            }
                            if let Some(parent) = &stack.parent {
                                traverse_stack(node, parent, find_node, add_dependency)?;
                            }

                            Ok(())
                        };

                        traverse_stack(node, stack, find_node, add_dependency)?;
                    }
                }
            }
        }
    }

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
            .to_string_with_weight_functions(
                |_ew| vec![],
                |nw| {
                    vec![
                        ("domain_name".into(), (&*nw.normalized_domain_name).into()),
                        (
                            "request_ids".into(),
                            (format!(
                                "{:#?}",
                                nw.requests
                                    .iter()
                                    .map(|r| &r.request_id)
                                    .collect::<Vec<_>>()
                            ).into()),
                        ),
                        (
                            "urls".into(),
                            (format!(
                                "{:#?}",
                                nw.requests.iter().map(|r| &r.url).collect::<Vec<_>>()
                            ).into()),
                        ),
                    ]
                },
            )
            .as_bytes(),
    )?;

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

pub mod chrome {
    // TODO missing support for redirects
    #[serde(tag = "method", content = "params")]
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub enum ChromeDebuggerMessage {
        // Everything Network
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
        pub call_frames: Vec<CallFrame>,
        pub parent: Option<Box<Script>>,
    }

    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
    pub struct CallFrame {
        pub url: String,
    }
}
