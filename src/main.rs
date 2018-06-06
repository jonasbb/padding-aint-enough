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
#[macro_use]
extern crate lazy_static;
extern crate num_traits;
extern crate serde_json;
extern crate serde_pickle;
extern crate serde_with;
extern crate url;

mod chrome;
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
use std::fs::{create_dir_all, remove_dir_all, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::RwLock;
use structopt::StructOpt;
use url::Url;

lazy_static! {
    /// Global output directory for all generated files
    static ref OUTDIR: RwLock<PathBuf> = RwLock::new(PathBuf::new());

    static ref PYTHON_DNS_TIMING: PathBuf = Path::new("./python/dns-timing-chart.py")
        .canonicalize()
        .expect("Canonicalizing a path should not fail.");
}

const DNS_TIMING: &str = "dns-timing.pickle";
const DEP_GRAPH: &str = "dependencies.graphml";

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
        let _ = writeln!(out, "{}", err.backtrace());
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

    // Setup output dir, but only if input file exists
    let outdir = cli_args.webpage_log.with_extension("generated");
    // Create directory and delete old versions
    let _ = remove_dir_all(&outdir);
    create_dir_all(&outdir)?;
    {
        let mut lock = OUTDIR.write().expect("Setting output dir may not fail");
        *lock = outdir;
    }

    let messages: Vec<ChromeDebuggerMessage> = serde_json::from_reader(rdr)?;
    process_messages(&messages)?;

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
        .map(|d| d.to_string())
        .ok_or_else(|| {
            format_err!(
                "The URL must have a domain part, but does not. URL: '{}'",
                parsed_url
            )
        })?)
}

fn dns_timing_chart(messages: &[ChromeDebuggerMessage]) -> Result<(), Error> {
    let timings: Vec<(String, String, Timing)> = messages
        .into_iter()
        .filter_map(|msg| {
            if let ChromeDebuggerMessage::NetworkResponseReceived {
                response: Response { url, timing },
                ..
            } = msg
            {
                if !should_ignore_url(url) {
                    if let Some(timing) = timing {
                        if timing.dns_start.is_some() {
                            return Some((url_to_domain(url).unwrap(), url.clone(), *timing));
                        }
                    }
                }
            }
            None
        })
        .collect();

    let fname = get_output_dir().join(DNS_TIMING);
    let mut wtr = file_open_write(
        &fname,
        WriteOptions::default().set_open_options(OpenOptions::new().create(true).truncate(true)),
    ).map_err(|err| {
        format_err!("Opening input file '{}' failed: {}", &fname.display(), err)
    })?;
    serde_pickle::to_writer(&mut wtr, &timings, true)?;

    Ok(())
}

fn process_messages(messages: &[ChromeDebuggerMessage]) -> Result<(), Error> {
    dns_timing_chart(messages)?;
    let _status = Command::new(&*PYTHON_DNS_TIMING)
        .arg(
            &*get_output_dir()
                .join(DNS_TIMING)
                .canonicalize()?
                .to_string_lossy(),
        )
        .current_dir(get_output_dir())
        .status()
        .context("Could not start Python process")?;

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
    let fname = get_output_dir().join(DEP_GRAPH);
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
    let fname = get_output_dir().join("dependencies.pickle");
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
    earliest_wall_time: Option<DateTime<Utc>>,
    requests: Vec<IndividualRequest>,
}

impl RequestInfo {
    /// Panics if `normalized_domain_name` is not equal
    fn merge_with(&mut self, other: &Self) {
        assert_eq!(self.normalized_domain_name, other.normalized_domain_name);

        self.requests.extend(other.requests.iter().cloned());
        self.earliest_wall_time = match (self.earliest_wall_time, other.earliest_wall_time) {
            (None, None) => None,
            (Some(s), None) => Some(s),
            (None, Some(o)) => Some(o),
            (Some(s), Some(o)) => Some(cmp::min(s, o)),
        };
    }

    pub fn graphml_support(&self) -> Vec<(Cow<'static, str>, Cow<str>)> {
        vec![
            ("domain_name".into(), (&*self.normalized_domain_name).into()),
            (
                "earliest_wall_time".into(),
                self.earliest_wall_time
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "".to_string())
                    .into(),
            ),
            (
                "request_ids".into(),
                format!(
                    "{:#?}",
                    self.requests
                        .iter()
                        .map(|r| &r.request_id)
                        .collect::<Vec<_>>()
                ).into(),
            ),
            (
                "urls".into(),
                format!(
                    "{:#?}",
                    self.requests.iter().map(|r| &r.url).collect::<Vec<_>>()
                ).into(),
            ),
            (
                "wall_times".into(),
                format!(
                    "{:#?}",
                    self.requests
                        .iter()
                        .map(|r| &r.wall_time)
                        .collect::<Vec<_>>()
                ).into(),
            ),
            (
                "domain+time".into(),
                format!(
                    "{}\n{}",
                    self.normalized_domain_name,
                    self.earliest_wall_time
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "".to_string())
                ).into(),
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
                let ind_req = IndividualRequest::try_from(from)?;
                Ok(RequestInfo{
                    normalized_domain_name: url_to_domain(url)?,
                    earliest_wall_time: ind_req.wall_time,
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

/// Some internal URLs in chrome are not interesting for us, so ignore them
///
/// Data URIs are not fetched from a server, so they do not cause network traffic.
/// chrome-extension is specific to chrome and does not cause network traffic.
fn should_ignore_url(url: &str) -> bool {
    url.starts_with("data:") || url.starts_with("chrome-extension:")
}
