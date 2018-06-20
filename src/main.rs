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
extern crate encrypted_dns;
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
use encrypted_dns::{
    dnstap::Message_Type, protos::DnstapContent, MatchKey, Query, QuerySource, UnmatchedClientQuery,
};
use failure::{Error, ResultExt};
use misc_utils::fs::{file_open_read, file_open_write, WriteOptions};
use petgraph::prelude::*;
use petgraph_graphml::GraphMl;
use std::borrow::Cow;
use std::cmp;
use std::collections::BTreeMap;
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

    let dnstap_file = cli_args.webpage_log.with_extension("dnstap");
    process_dnstap(&*dnstap_file)
        .with_context(|_| format_err!("Processing dnstap file '{}'", dnstap_file.display()))?;
    let messages: Vec<ChromeDebuggerMessage> = serde_json::from_reader(rdr).with_context(|_| {
        format_err!(
            "Error while deserializing '{}'",
            cli_args.webpage_log.display()
        )
    })?;
    process_messages(&messages).with_context(|_| {
        format_err!(
            "Processing chrome debugger log '{}'",
            cli_args.webpage_log.display()
        )
    })?;

    Ok(())
}

fn process_dnstap(dnstap_file: &Path) -> Result<(), Error> {
    // process dnstap if available
    if dnstap_file.exists() {
        info!("Found dnstap file.");
        let mut events: Vec<encrypted_dns::protos::Dnstap> =
            encrypted_dns::process_dnstap(&*dnstap_file)?.collect::<Result<_, Error>>()?;

        // the dnstap events can be out of order, so sort them by timestamp
        // always take the later timestamp if there are multiple
        events.sort_by_key(|ev| {
            let DnstapContent::Message {
                query_time,
                response_time,
                ..
            } = ev.content;
            if let Some(time) = response_time {
                return time;
            } else if let Some(time) = query_time {
                return time;
            } else {
                panic!("The dnstap message must contain either a query or response time.")
            }
        });

        let mut unanswered_client_queries = BTreeMap::new();
        let mut matched = Vec::new();

        for ev in events
            .into_iter()
                // search for the CLIENT_RESPONE `start.example.` message as the end of the prefetching events
            .skip_while(|ev| {
                let DnstapContent::Message {
                    message_type,
                    ref response_message,
                    ..
                } = ev.content;
                if message_type == Message_Type::CLIENT_RESPONSE {
                    let (dnsmsg, _size) =
                        response_message.as_ref().expect("Unbound always sets this");
                    let qname = dnsmsg.queries()[0].name().to_utf8();
                    if qname == "start.example." {
                        return false;
                    }
                }
                true
            })
            // the skip while returns the CLIENT_RESPONSE with `start.example.`
            // We want to remove this as well, so skip over the first element here
            .skip(1)
            // Only process messages until the end message is found in form of the first (thus CLIENT_QUERY)
            // message forr domain `end.example.`
            .take_while(|ev| {
                let DnstapContent::Message {
                    message_type,
                    ref query_message,
                    ..
                } = ev.content;
                if message_type == Message_Type::CLIENT_QUERY {
                    let (dnsmsg, _size) =
                        query_message.as_ref().expect("Unbound always sets this");
                    let qname = dnsmsg.queries()[0].name().to_utf8();
                    if qname == "end.example." {
                        return false;
                    }
                }
                true
            })
            .filter(|ev| {
                // The only interesting information is the timestamp which is also contained in the response
                let DnstapContent::Message { message_type, .. } = ev.content;
                message_type != Message_Type::FORWARDER_QUERY
            }) {
            let DnstapContent::Message {
                message_type,
                query_message,
                response_message,
                query_time,
                response_time,
                query_port,
                ..
            } = ev.content;
            match message_type {
                Message_Type::CLIENT_QUERY => {
                    let (dnsmsg, size) = query_message.expect("Unbound always sets this");
                    let qname = dnsmsg.queries()[0].name().to_utf8();
                    let qtype = dnsmsg.queries()[0].query_type().to_string();
                    let id = dnsmsg.id();
                    let start = query_time.expect("Unbound always sets this");
                    let port = query_port.expect("Unbound always sets this");

                    let key = MatchKey {
                        qname: qname.clone(),
                        qtype: qtype.clone(),
                        id,
                        port,
                    };
                    let value = UnmatchedClientQuery {
                        qname,
                        qtype,
                        start,
                        size: size as u32,
                    };
                    let existing_value = unanswered_client_queries.insert(key, value);
                    if let Some(existing_value) = existing_value {
                        info!(
                            "Duplicate Client Query for '{}' ({})",
                            existing_value.qname, existing_value.qtype
                        );
                    }
                }

                Message_Type::CLIENT_RESPONSE => {
                    let (dnsmsg, size) = response_message.expect("Unbound always sets this");
                    let qname = dnsmsg.queries()[0].name().to_utf8();
                    let qtype = dnsmsg.queries()[0].query_type().to_string();
                    let id = dnsmsg.id();
                    let end = response_time.expect("Unbound always sets this");
                    let port = query_port.expect("Unbound always sets this");

                    let key = MatchKey {
                        qname: qname.clone(),
                        qtype: qtype.clone(),
                        id,
                        port,
                    };
                    if let Some(unmatched) = unanswered_client_queries.remove(&key) {
                        matched.push(Query {
                            source: QuerySource::Client,
                            qname: unmatched.qname,
                            qtype: unmatched.qtype,
                            start: unmatched.start,
                            end,
                            query_size: unmatched.size,
                            response_size: size as u32,
                        })
                    } else {
                        info!("Unmatched Client Response for '{}' ({})", qname, qtype);
                    };
                }

                Message_Type::FORWARDER_RESPONSE => {
                    let (dnsmsg, size) = response_message.expect("Unbound always sets this");
                    let qname = dnsmsg.queries()[0].name().to_utf8();
                    let qtype = dnsmsg.queries()[0].query_type().to_string();
                    let start = query_time.expect("Unbound always sets this");
                    let end = response_time.expect("Unbound always sets this");
                    matched.push(Query {
                        source: QuerySource::Forwarder,
                        qname,
                        qtype,
                        start,
                        end,
                        query_size: u32::max_value(),
                        response_size: size as u32,
                    });
                }

                _ => bail!("Unexpected message type {:?}", message_type),
            }
        }

        // cleanup some messages
        // filter out all the queries which are just noise
        matched.retain(|query| !(query.qtype == "NULL" && query.qname.starts_with("_ta")));

        let fname = get_output_dir().join("dns.pickle");
        let mut wtr = file_open_write(
            &fname,
            WriteOptions::default()
                .set_open_options(OpenOptions::new().create(true).truncate(true)),
        ).map_err(|err| {
            format_err!("Opening output file '{}' failed: {}", &fname.display(), err)
        })?;
        serde_pickle::to_writer(&mut wtr, &matched, true)?;
    }

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
        .filter_map(|msg| match msg {
            ChromeDebuggerMessage::NetworkRequestWillBeSent {
                redirect_response: Some(RedirectResponse { url, timing }),
                ..
            }
            | ChromeDebuggerMessage::NetworkResponseReceived {
                response:
                    Response {
                        url,
                        timing: Some(timing),
                    },
                ..
            } => {
                if !should_ignore_url(url) && timing.dns_start.is_some() {
                    return Some((url_to_domain(url).unwrap(), url.clone(), *timing));
                }
                None
            }
            // Ignore all other messages
            _ => None,
        })
        .collect();

    // protect against failed network requests. Sometimes this might end up empty, in which case we do not want to plot anything
    let fname = get_output_dir().join(DNS_TIMING);
    if timings.is_empty() {
        warn!("Skipping {} because no timing information available", fname.display());
        return Ok(());
    }

    let mut wtr = file_open_write(
        &fname,
        WriteOptions::default().set_open_options(OpenOptions::new().create(true).truncate(true)),
    ).map_err(|err| {
        format_err!("Opening input file '{}' failed: {}", &fname.display(), err)
    })?;
    serde_pickle::to_writer(&mut wtr, &timings, true)?;
    // we need to close the writer to flush everything
    drop(wtr);

    let status = Command::new(&*PYTHON_DNS_TIMING)
        .arg(
            &*get_output_dir()
                .join(DNS_TIMING)
                .canonicalize()?
                .to_string_lossy(),
        )
        .current_dir(get_output_dir())
        .status()
        .context("Could not start Python process")?;

    if !status.success() {
        match status.code() {
            Some(code) => bail!("Python exited with status code: {}", code),
            None => bail!("Python terminated by signal"),
        }
    }

    Ok(())
}

fn process_messages(messages: &[ChromeDebuggerMessage]) -> Result<(), Error> {
    dns_timing_chart(messages)?;

    let mut depgraph = DepGraph::new(messages).context("Failure to build the graph.")?;
    depgraph.simplify_graph();
    depgraph.duplicate_domains();
    let graph = depgraph.as_graph();
    export_as_graphml(graph)?;
    export_as_pickle(graph)?;

    // for domain in  depgraph.get_domain_names(){
    //     println!("{}", domain);
    // }

    Ok(())
}

fn export_as_graphml(graph: &Graph<RequestInfo, ()>) -> Result<(), Error> {
    let graphml = GraphMl::new(&graph).export_node_weights(Box::new(RequestInfo::graphml_support));
    let fname = get_output_dir().join(DEP_GRAPH);
    let wtr = file_open_write(
        &fname,
        WriteOptions::default().set_open_options(OpenOptions::new().create(true).truncate(true)),
    ).map_err(|err| {
        format_err!("Opening output file '{}' failed: {}", &fname.display(), err)
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
        format_err!("Opening output file '{}' failed: {}", &fname.display(), err)
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
            ChromeDebuggerMessage::TargetTargetInfoChanged {
                target_info: TargetInfo{ref url, ..}
            } => {
                Ok(RequestInfo {
                    normalized_domain_name: url_to_domain(&url)?,
                    earliest_wall_time: None,
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
    url.starts_with("data:")
        || url.starts_with("chrome-extension:")
        || url.starts_with("blob:")
        || url.starts_with("about:")
        || url.starts_with("extensions::")
}
