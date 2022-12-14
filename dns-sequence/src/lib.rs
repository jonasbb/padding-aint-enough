use anyhow::{anyhow, Context as _, Error};
use csv::ReaderBuilder;
use log::{error, info, warn};
use misc_utils::fs::file_open_read;
use once_cell::sync::Lazy;
use sequences::{knn::LabelledSequences, LoadSequenceConfig, Sequence, SimulatedCountermeasure};
use serde::Deserialize;
use std::{
    collections::HashMap,
    ffi::OsStr,
    path::Path,
    sync::{Arc, RwLock},
};
use string_cache::DefaultAtom as Atom;
use structopt::clap::arg_enum;

static CONFUSION_DOMAINS: Lazy<RwLock<Arc<HashMap<Atom, Atom>>>> = Lazy::new(Default::default);

arg_enum! {
    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub enum SimulateOption {
        Normal,
        PerfectPadding,
        PerfectTiming,
    }
}

impl From<SimulateOption> for SimulatedCountermeasure {
    fn from(so: SimulateOption) -> Self {
        match so {
            SimulateOption::Normal => SimulatedCountermeasure::None,
            SimulateOption::PerfectPadding => SimulatedCountermeasure::PerfectPadding,
            SimulateOption::PerfectTiming => SimulatedCountermeasure::PerfectTiming,
        }
    }
}

pub fn prepare_confusion_domains<D, P>(data: D) -> Result<(), Error>
where
    D: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    #[derive(Debug, Deserialize)]
    struct Record {
        domain: Atom,
        is_similar_to: Atom,
    }

    let mut conf_domains = HashMap::default();

    for path in data {
        let path = path.as_ref();
        let mut reader = ReaderBuilder::new().has_headers(false).from_reader(
            file_open_read(path)
                .with_context(|| format!("Opening confusion file '{}' failed", path.display()))?,
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

pub fn load_all_files(
    base_dir: &Path,
    file_extension: &OsStr,
    simulate: SimulateOption,
) -> Result<Vec<LabelledSequences>, Error> {
    // Support to read a pre-processed JSON file instead of reading many directories from disk
    // Implementing this here means this works in all cases
    if base_dir.is_file() {
        let s = misc_utils::fs::read_to_string(base_dir)
            .with_context(|| anyhow!("Could not open {} to read from it.", base_dir.display()))?;
        return serde_json::from_str(&s).with_context(|| {
            anyhow!(
                "The file {} could not be deserialized into LabelledSequences",
                base_dir.display()
            )
        });
    }

    let check_confusion_domains = make_check_confusion_domains();

    let sequence_config = LoadSequenceConfig {
        simulated_countermeasure: simulate.into(),
        ..LoadSequenceConfig::default()
    };

    let seqs = sequences::load_all_files_with_extension_from_dir_with_config(
        base_dir,
        file_extension,
        sequence_config,
    )
    .with_context(|| {
        format!(
            "Could not load some sequence files from dir: {}",
            base_dir.display()
        )
    })?;
    info!("Start creating LabelledSequences");
    Ok(seqs
        .into_iter()
        .map(|(label, seqs): (String, Vec<Sequence>)| {
            let label = Atom::from(label);
            let mapped_label = check_confusion_domains(&label);
            LabelledSequences {
                true_domain: label,
                mapped_domain: mapped_label,
                sequences: seqs,
            }
        })
        .collect::<Vec<_>>())
}

fn make_check_confusion_domains() -> impl Fn(&Atom) -> Atom {
    let lock = CONFUSION_DOMAINS.read().unwrap();
    let conf_domains: Arc<_> = lock.clone();
    move |domain: &Atom| -> Atom {
        let mut curr = domain;
        let mut loop_check = 10;
        while let Some(other) = conf_domains.get(curr) {
            curr = other;

            loop_check -= 1;
            if loop_check == 0 {
                error!("Loop detected");
                let mut visited = Vec::new();
                let mut c = domain;
                visited.push(c);
                warn!("Visit {}", c);
                while let Some(o) = conf_domains.get(c) {
                    if visited.contains(&o) {
                        error!("{:#?}", visited);
                        return o.into();
                    }
                    warn!("Visit {}", o);
                    c = o;
                }
            }
        }
        curr.into()
    }
}
