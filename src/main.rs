mod depgraph;

use crate::depgraph::DepGraph;
use chrome::{ChromeDebuggerMessage, Request, TargetInfo};
use chrono::{DateTime, Utc};
use failure::{bail, format_err, Error, ResultExt};
use lazy_static::lazy_static;
use misc_utils::{
    fs::{file_open_write, read_to_string, WriteOptions},
    Min,
};
use petgraph::prelude::*;
use petgraph_graphml::GraphMl;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    convert::TryFrom,
    fs::{create_dir_all, remove_dir_all, OpenOptions},
    path::PathBuf,
    sync::RwLock,
};
use structopt::{self, StructOpt};
use url::Url;

lazy_static! {
    /// Global output directory for all generated files
    static ref OUTDIR: RwLock<PathBuf> = RwLock::new(PathBuf::new());
}

const DEP_GRAPH: &str = "dependencies.graphml";

#[derive(StructOpt, Debug)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
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
        for fail in err.iter_chain() {
            let _ = writeln!(out, "  {}", fail);
        }
        let _ = writeln!(out, "{}", err.backtrace());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    // generic setup
    env_logger::init();
    let cli_args = CliArgs::from_args();

    // Setup output dir, but only if input file exists
    let outdir = cli_args.webpage_log.with_extension("generated");
    // Create directory and delete old versions
    let _ = remove_dir_all(&outdir);
    create_dir_all(&outdir)?;
    {
        let mut lock = OUTDIR.write().expect("Setting output dir may not fail");
        *lock = outdir;
    }

    let content = read_to_string(&cli_args.webpage_log).with_context(|_| {
        format!(
            "Reading input file '{}' failed",
            cli_args.webpage_log.display(),
        )
    })?;
    let messages: Vec<ChromeDebuggerMessage> =
        serde_json::from_str(&content).with_context(|_| {
            format!(
                "Error while deserializing '{}'",
                cli_args.webpage_log.display()
            )
        })?;
    process_messages(&messages).with_context(|_| {
        format!(
            "Processing chrome debugger log '{}'",
            cli_args.webpage_log.display()
        )
    })?;

    Ok(())
}

/// Returns a directory under which all output files should be created
fn get_output_dir() -> PathBuf {
    let lock = OUTDIR.read().expect("Unlocking the RwLock must work");
    lock.clone()
}

fn url_to_domain(url: &str) -> Result<String, Error> {
    let parsed_url =
        Url::parse(&url).context("RequestInfo needs a domain name, but URL is not a valid URL.")?;
    Ok(parsed_url
        .host_str()
        .map(ToString::to_string)
        .ok_or_else(|| {
            format_err!(
                "The URL must have a domain part, but does not. URL: '{}'",
                parsed_url
            )
        })?)
}

fn process_messages(messages: &[ChromeDebuggerMessage]) -> Result<(), Error> {
    let mut depgraph = DepGraph::new(messages).context("Failure to build the graph.")?;
    depgraph.simplify_graph();
    depgraph.duplicate_domains();
    let graph = depgraph.as_graph();
    export_as_graphml(graph)?;

    Ok(())
}

fn export_as_graphml(graph: &Graph<RequestInfo, ()>) -> Result<(), Error> {
    let graphml = GraphMl::new(&graph).export_node_weights(Box::new(RequestInfo::graphml_support));
    let fname = get_output_dir().join(DEP_GRAPH);
    let wtr = file_open_write(
        &fname,
        WriteOptions::default().set_open_options(OpenOptions::new().create(true).truncate(true)),
    )
    .with_context(|_| format!("Opening output file '{}' failed", &fname.display(),))?;
    graphml.to_writer(wtr)?;

    Ok(())
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize)]
pub struct RequestInfo {
    normalized_domain_name: String,
    #[serde(with = "serde_with::rust::display_fromstr")]
    earliest_wall_time: Min<DateTime<Utc>>,
    requests: Vec<IndividualRequest>,
}

impl RequestInfo {
    /// Panics if `normalized_domain_name` is not equal
    fn merge_with(&mut self, other: &Self) {
        assert_eq!(self.normalized_domain_name, other.normalized_domain_name);

        self.requests.extend(other.requests.iter().cloned());
        self.earliest_wall_time.update(other.earliest_wall_time);
    }

    pub fn graphml_support(&self) -> Vec<(Cow<'static, str>, Cow<'_, str>)> {
        vec![
            ("domain_name".into(), (&*self.normalized_domain_name).into()),
            (
                "earliest_wall_time".into(),
                self.earliest_wall_time.to_string().into(),
            ),
            (
                "request_ids".into(),
                format!(
                    "{:#?}",
                    self.requests
                        .iter()
                        .map(|r| &r.request_id)
                        .collect::<Vec<_>>()
                )
                .into(),
            ),
            (
                "urls".into(),
                format!(
                    "{:#?}",
                    self.requests.iter().map(|r| &r.url).collect::<Vec<_>>()
                )
                .into(),
            ),
            (
                "wall_times".into(),
                format!(
                    "{:#?}",
                    self.requests
                        .iter()
                        .map(|r| &r.wall_time)
                        .collect::<Vec<_>>()
                )
                .into(),
            ),
            (
                "domain+time".into(),
                format!(
                    "{}\n{}",
                    self.normalized_domain_name, self.earliest_wall_time,
                )
                .into(),
            ),
        ]
    }
}

impl<'a> TryFrom<&'a ChromeDebuggerMessage> for RequestInfo {
    type Error = Error;

    fn try_from(from: &'a ChromeDebuggerMessage) -> Result<Self, Error> {
        match *from {
            ChromeDebuggerMessage::TargetTargetInfoChanged {
                target_info: TargetInfo{ref url, ..}
            } => {
                Ok(RequestInfo {
                    normalized_domain_name: url_to_domain(&url)?,
                    earliest_wall_time: Min::default(),
                    requests: vec![],
                })
            }
            ChromeDebuggerMessage::NetworkRequestWillBeSent{
                request: Request { ref url, .. },
                ..
            } => {
                let ind_req = IndividualRequest::try_from(from)?;
                Ok(RequestInfo{
                    normalized_domain_name: url_to_domain(url)?,
                    earliest_wall_time: ind_req.wall_time.map(Into::into).unwrap_or_default(),
                    requests: vec![ind_req],
                })
           },
            ChromeDebuggerMessage::NetworkWebSocketCreated{
                ref url,
                ..
            } => {
                let ind_req = IndividualRequest::try_from(from)?;
                Ok(RequestInfo{
                    normalized_domain_name: url_to_domain(url)?,
                    earliest_wall_time: ind_req.wall_time.map(Into::into).unwrap_or_default(),
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
    wall_time: Option<DateTime<Utc>>,
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
                    wall_time: Some(wall_time),
                })
            },
            ChromeDebuggerMessage::NetworkWebSocketCreated{
                ref url,
                ref request_id,
                ..
            } => {
                Ok(IndividualRequest {
                    request_id: request_id.clone(),
                    url: url.clone(),
                    wall_time: None,
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
        use petgraph::visit::IntoNodeIdentifiers;

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

/// Some internal URLs in chrome are not interesting for us, so ignore them
///
/// Data URIs are not fetched from a server, so they do not cause network traffic.
/// chrome-extension is specific to chrome and does not cause network traffic.
fn should_ignore_url(url: &str) -> bool {
    url.starts_with("data:")
        || url.starts_with("chrome-extension:")
        || url.starts_with("blob:")
        || url.starts_with("about:")
        || url.starts_with("extensions::")
}
