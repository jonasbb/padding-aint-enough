use csv::ReaderBuilder;
use dnstap::{
    dnstap::Message_Type,
    process_dnstap,
    protos::{self, DnstapContent},
};
use failure::{format_err, Error, ResultExt};
use lazy_static::lazy_static;
use log::{error, info};
use misc_utils::fs::{file_open_read, file_open_write};
use rayon::prelude::*;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::BufReader,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};
use structopt::{self, StructOpt};

lazy_static! {
    static ref CONFUSION_DOMAINS: RwLock<Arc<HashMap<String, String>>> = RwLock::default();
}

#[derive(StructOpt, Debug)]
#[structopt(global_settings(&[
    structopt::clap::AppSettings::ColoredHelp,
    structopt::clap::AppSettings::VersionlessSubcommands
]))]
struct CliArgs {
    /// Base directory containing per domain a folder which contains the dnstap files
    #[structopt(parse(from_os_str))]
    base_dir: PathBuf,
    /// Some domains are known similar. Specify a CSV file renaming the "original" domain to some other identifier.
    /// This option can be applied multiple times. It is not permitted to have conflicting entries to the same domain.
    #[structopt(short = "d", long = "confusion_domains", parse(from_os_str))]
    confusion_domains: Vec<PathBuf>,
    /// List of file names which did not load properly.
    /// Also see the `website-failed` tool.
    #[structopt(long = "loading_failed", parse(from_os_str))]
    loading_failed: Option<PathBuf>,
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

    info!("Start loading confusion domains...");
    prepare_confusion_domains(&cli_args.confusion_domains)?;
    info!("Done loading confusion domains.");

    let failed_domains = if let Some(path) = &cli_args.loading_failed {
        info!("Start loading of failed domains...");
        let tmp = prepare_failed_domains(path)?;
        info!("Done loading of failed domains.");
        tmp
    } else {
        HashSet::default()
    };

    let check_confusion_domains = make_check_confusion_domains();

    // Get a list of directories
    // Each directory corresponds to a label
    let directories: Vec<PathBuf> = fs::read_dir(&cli_args.base_dir)?
        .flat_map(|x| {
            x.and_then(|entry| {
                // Result<Option<PathBuf>>
                entry.file_type().map(|ft| {
                    if ft.is_dir()
                        || (ft.is_symlink() && fs::metadata(&entry.path()).ok()?.is_dir())
                    {
                        Some(entry.path())
                    } else {
                        None
                    }
                })
            })
            .transpose()
        })
        .collect::<Result<_, _>>()?;

    // Pairs of Label with Data (the Sequences)
    let loaded_domains: HashMap<String, Vec<Vec<String>>> = directories
        .into_par_iter()
        .map(|dir| -> Result<_, Error> {
            let label: String = check_confusion_domains(
                &dir.file_name()
                    .expect("Each directory has a name")
                    .to_string_lossy(),
            );

            let mut filenames: Vec<PathBuf> = fs::read_dir(&dir)?
                .flat_map(|x| {
                    x.and_then(|entry| {
                        // Result<Option<PathBuf>>
                        entry.file_type().map(|ft| {
                            if ft.is_file()
                                && entry.file_name().to_string_lossy().contains(".dnstap")
                            {
                                // Ignore all files which are in the failed domains list
                                let path = entry.path();
                                let fname = path.file_name().unwrap().to_string_lossy();
                                if !failed_domains.contains(&*fname) {
                                    Some(entry.path())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        })
                    })
                    .transpose()
                })
                .collect::<Result<_, _>>()?;
            // sort filenames for predictable results
            filenames.sort();

            let responses = filenames
                .into_iter()
                .map(|fname| -> Result<Vec<String>, Error> {
                    let mut events: Vec<protos::Dnstap> =
                        process_dnstap(fname)?.collect::<Result<_, Error>>()?;

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
                            panic!(
                                "The dnstap message must contain either a query or response time."
                            )
                        }
                    });

                    Ok(events
                        .into_iter()
                        .filter_map(|ev| {
                            let DnstapContent::Message {
                                message_type,
                                // query_message,
                                response_message,
                                ..
                            } = ev.content;
                            match message_type {
                                // Message_Type::FORWARDER_QUERY => {
                                //     let (_dnsmsg, size) =
                                //         query_message.expect("Unbound always sets this: FR r msg");
                                //     println!("{}", size);
                                //     None
                                // }
                                Message_Type::FORWARDER_RESPONSE => {
                                    let (dnsmsg, _size) = response_message
                                        .expect("Unbound always sets this: FR r msg");
                                    let qname = dnsmsg.queries()[0].name().to_utf8();
                                    let qtype = dnsmsg.queries()[0].query_type().to_string();
                                    Some(format!("{} {}", qname, qtype))
                                }

                                _ => None,
                            }
                        })
                        .collect())
                })
                .collect::<Result<Vec<Vec<String>>, Error>>()?;

            Ok((label, responses))
        })
        .collect::<Result<_, _>>()?;

    // Map domain to pair of (set of domains using this first domain, set of traces using this first domain)
    let mut usage_per_domain: HashMap<String, (HashSet<String>, HashSet<String>)> =
        HashMap::default();
    for (label, traces) in &loaded_domains {
        for (trace_num, trace) in traces.iter().enumerate() {
            for domain in trace {
                let entry = usage_per_domain.entry(domain.clone()).or_default();
                entry.0.insert(label.to_string());
                entry.1.insert(format!("{}-{}", label, trace_num));
            }
        }
    }
    // Map domain to pair of (count in how many domains the first domain is a third-party domain, count in how many traces the first domain is a thrid-party domain)
    let counts_per_domain: HashMap<String, (usize, usize)> = usage_per_domain
        .iter()
        .map(|(label, (label_set, trace_set))| (label.clone(), (label_set.len(), trace_set.len())))
        .collect();

    let (traces_labelcount, traces_tracecount): (Vec<Vec<usize>>, Vec<Vec<usize>>) = loaded_domains
        .values()
        .flat_map(|traces| {
            traces.iter().map(|trace| -> (Vec<usize>, Vec<usize>) {
                trace
                    .iter()
                    .map(|domain| counts_per_domain[&**domain])
                    .unzip()
            })
        })
        .unzip();

    let mut wtr = file_open_write("./traces_labelcount.json", Default::default())?;
    serde_json::to_writer(&mut wtr, &traces_labelcount)?;
    drop(wtr);
    let mut wtr = file_open_write("./traces_tracecount.json", Default::default())?;
    serde_json::to_writer(&mut wtr, &traces_tracecount)?;
    drop(wtr);
    let mut wtr = file_open_write("./counts_per_domain.json", Default::default())?;
    serde_json::to_writer(&mut wtr, &counts_per_domain)?;
    drop(wtr);

    Ok(())
}

fn prepare_failed_domains(path: impl AsRef<Path>) -> Result<HashSet<String>, Error> {
    #[derive(Debug, Deserialize)]
    struct Record {
        file: PathBuf,
        reason: String,
    }

    let rdr = BufReader::new(file_open_read(path.as_ref()).with_context(|_| {
        format!(
            "Opening failed domains file '{}' failed",
            path.as_ref().display()
        )
    })?);
    let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(rdr);

    let mut failed_domains = HashSet::default();

    for record in rdr.deserialize() {
        let record: Record = record.context("Failed to read from failed domains file")?;
        let file_name: String = (&record.file)
            .file_name()
            .map(|file_name| file_name.to_string_lossy().replace(".json", ".dnstap"))
            .ok_or_else(|| {
                format_err!(
                    "This line does not specify a path with a file name '{}'",
                    record.file.display()
                )
            })?;

        failed_domains.insert(file_name);
    }

    Ok(failed_domains)
}

fn prepare_confusion_domains<D, P>(data: D) -> Result<(), Error>
where
    D: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    #[derive(Debug, Deserialize)]
    struct Record {
        domain: String,
        is_similar_to: String,
    };

    let mut conf_domains = HashMap::default();

    for path in data {
        let path = path.as_ref();
        let mut reader = ReaderBuilder::new().has_headers(false).from_reader(
            file_open_read(path)
                .with_context(|_| format!("Opening confusion file '{}' failed", path.display()))?,
        );
        for record in reader.deserialize() {
            let record: Record = record?;
            // skip comment lines
            if record.domain.starts_with('#') {
                continue;
            }
            let existing = conf_domains.insert(record.domain.clone(), record.is_similar_to.clone());
            if let Some(existing) = existing {
                if existing != record.is_similar_to {
                    error!("Duplicate confusion mappings for domain '{}' but with different targets: 1) '{}' 2) '{}'", record.domain, existing, record.is_similar_to);
                }
            }
        }
    }

    let mut lock = CONFUSION_DOMAINS.write().unwrap();
    *lock = Arc::new(conf_domains);

    Ok(())
}

fn make_check_confusion_domains() -> impl Fn(&str) -> String {
    let lock = CONFUSION_DOMAINS.read().unwrap();
    let conf_domains: Arc<_> = lock.clone();
    move |domain: &str| -> String {
        let mut curr = domain;
        while let Some(other) = conf_domains.get(curr) {
            curr = other;
        }
        curr.into()
    }
}
