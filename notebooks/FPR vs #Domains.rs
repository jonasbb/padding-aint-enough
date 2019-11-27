// -*- coding: utf-8 -*-
// ---
// jupyter:
//   jupytext:
//     formats: ipynb,rs:percent
//     text_representation:
//       extension: .rs
//       format_name: percent
//       format_version: '1.3'
//       jupytext_version: 1.3.0
//   kernelspec:
//     display_name: Rust
//     language: rust
//     name: rust
// ---

// %% [markdown]
// # README
//
// This file calculates the threshold vs domains correctly classified data.
// The problem is, the n/10 data contains also sequences with reason and they need to be filtered out.

// %% [markdown]
// # Generic Includes

// %%
// :dep prettytable = {package = "prettytable-rs"}

// %%
// :dep sequences = { path = "/home/jbushart/projects/encrypted-dns/sequences"}
// %%
// :dep serde = { version = "1.0.84", features = [ "derive" ] }
// %%
extern crate misc_utils;
extern crate prettytable;
extern crate rayon;
extern crate sequences;
extern crate serde_json;
extern crate serde;
// %%
use misc_utils::fs::*;
use prettytable::{
    cell,
    format::{FormatBuilder, LinePosition, LineSeparator, TableFormat},
    row, Table,
};
use rayon::prelude::*;
use sequences::{
    knn::{ClassificationResult, ClassificationResultQuality},
    load_all_dnstap_files_from_dir, Sequence, SequenceElement,
    load_all_files_with_extension_from_dir_with_config
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::OsString,
    fmt::{self, Display},
    hash::Hash,
    path::{Path, PathBuf},
};

// %% [markdown]
// # Parsing of misclassification files

// %%
#[derive(Clone, Debug, Deserialize)]
pub struct Misclassification {
    pub id: PathBuf,
    pub k: usize,
    pub label: String,
    pub class_result: ClassificationResult,
    pub reason: Option<String>,
}

impl Misclassification {
    pub fn determine_quality(&self) -> sequences::knn::ClassificationResultQuality {
        self.class_result.determine_quality(&*self.label)
    }
}

// %%
pub fn load_misclassifications(file: impl AsRef<Path>) -> Vec<Misclassification> {
    let buffer = read_to_string(&file.as_ref())
        .unwrap();
    let mut counter = 0;
    serde_json::Deserializer::from_str(&buffer)
        .into_iter::<Misclassification>()
        .filter(Result::is_ok)
        .map(|x: Result<Misclassification, _>| {
            let mut x = x.unwrap();
            x.id = PathBuf::from(OsString::from(x.id.file_name().unwrap()));
            x
        })
        .collect()
}

// %%
let file_to_quality: Vec<HashMap<PathBuf, (ClassificationResultQuality, Option<String>)>> = (1..11)
    .into_iter()
    .map(|i| format!("../results/2019-01-28-FPR-in-CW/misclassifications-fpr-{}.json.xz", i*10))
    .map(|s| {
        let mis = load_misclassifications(&s);
        mis.into_iter()
            .filter(|m| m.k == 7)
            .map(|m| {
                let q = m.determine_quality();
                (m.id, (q, m.reason))
            })
            .collect()
    })
    .collect();

// %% [markdown]
// # Parsing of dnstap files, to extract the reason information

// %%
let data_closed_world = load_all_dnstap_files_from_dir(&"/home/jbushart/projects/data/dnscaptures-main-group".as_ref()).unwrap();


// %% [markdown]
// # Combine above resources into one

// %%
#[must_use]
pub fn reverse_cum_sum(counts: &[usize]) -> Vec<usize> {
    let mut accu = 0;
    // convert the counts per "correctness level" into accumulative counts
    let mut tmp: Vec<_> = counts
        .iter()
        // go from 10 to 0
        .rev()
        // sum them like
        // 10; 10 + 9; 10 + 9 + 8; etc.
        .map(|&count| {
            accu += count;
            accu
        })
        .collect();
    // revert them again to go from 0 to 10
    tmp.reverse();
    tmp
}

#[derive(Debug)]
pub struct StatsCollector<S: Eq + Hash> {
    pub data: HashMap<u8, StatsInternal<S>>,
}

#[derive(Debug)]
pub struct StatsCounter<S: Eq + Hash> {
    /// Counts pairs of `ClassificationResultQuality` and if it is known problematic (bool).
    pub results: HashMap<(ClassificationResultQuality, bool), usize>,
    /// Counts the problematic reasons
    pub reasons: HashMap<S, usize>,
}

#[derive(Debug)]
pub struct StatsInternal<S: Eq + Hash> {
    pub true_domain: HashMap<S, StatsCounter<S>>,
    pub mapped_domain: HashMap<S, StatsCounter<S>>,
    pub global: StatsCounter<S>,
}

impl<S: Eq + Hash> StatsCollector<S> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn update(
        &mut self,
        k: u8,
        true_domain: S,
        mapped_domain: S,
        result: ClassificationResultQuality,
        known_problems: Option<S>,
    ) where
        S: Clone,
    {
        // Do not even try to insert them, such that they can also never be reported
        if known_problems.is_some() {
            return;
        }

        let k_stats = self.data.entry(k).or_default();
        k_stats
            .true_domain
            .entry(true_domain)
            .or_default()
            .update(result, known_problems.clone());
        k_stats
            .mapped_domain
            .entry(mapped_domain)
            .or_default()
            .update(result, known_problems.clone());
        k_stats.global.update(result, known_problems);
    }

    /// Count the number of domains with at least x correctly labelled domains, where x is the array index
    pub fn count_correct(&self) -> HashMap<u8, HashMap<ClassificationResultQuality, Vec<usize>>> {
        self.data
            .iter()
            .map(|(&k, stats)| {
                let res: HashMap<ClassificationResultQuality, Vec<_>> =
                    ClassificationResultQuality::iter_variants()
                        .map(|quality| {
                            // Count how many domains have x domains with a classification result of quality
                            // `quality` or higher.
                            // x will be used as index into the vector
                            // needs to store values from 0 to 10 (inclusive)
                            let mut counts = vec![0; 11];
                            stats
                                .true_domain
                                .iter()
                                .for_each(|(_domain, internal_stats)| {
                                    let mut corrects = 0;
                                    for higher_q in ClassificationResultQuality::iter_variants()
                                        .filter(|&other_q| other_q >= quality)
                                    {
                                        corrects += internal_stats
                                            .results
                                            .get(&(higher_q, false))
                                            .cloned()
                                            .unwrap_or(0)
                                            + internal_stats
                                                .results
                                                .get(&(higher_q, true))
                                                .cloned()
                                                .unwrap_or(0);
                                    }
                                    counts[corrects] += 1;
                                });
                            (quality, counts)
                        })
                        .collect();
                (k, res)
            })
            .collect()
    }
}

impl<S> Display for StatsCollector<S>
where
    S: Display + Eq + Hash,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use prettytable::Row;

        let UNICODE_LIGHT_SEP: LineSeparator = LineSeparator::new('─', '┼', '├', '┤');
        let UNICODE_HEAVY_SEP: LineSeparator = LineSeparator::new('━', '┿', '┝', '┥');
        let UNICODE_DOUBLE_SEP: LineSeparator = LineSeparator::new('═', '╪', '╞', '╡');
        let FORMAT_NO_BORDER_UNICODE: TableFormat = FormatBuilder::new()
            .padding(1, 1)
            // .separator(LinePosition::Intern, *UNICODE_LIGHT_SEP)
            .separator(LinePosition::Title, UNICODE_DOUBLE_SEP)
            .column_separator('│')
            .build();

        let mut keys: Vec<_> = self.data.keys().collect();
        let count_corrects = self.count_correct();
        keys.sort();

        let mut first = true;
        for k in keys {
            if !first {
                write!(f, "\n\n")?;
            }
            first = false;

            writeln!(
                f,
                "#Domains with at least x classification results of quality or higher:"
            )?;
            let header = Row::new(
                Some(cell!(bc->"Method"))
                    .into_iter()
                    .chain((0..=10).map(|c| cell!(bc->c)))
                    .collect(),
            );
            let tmp = &count_corrects[k];

            // For each quality level, we want to count the number of classifications with equal or better quality
            let counts: Vec<_> = ClassificationResultQuality::iter_variants()
                // skip some qualitys, as this does not match the semantics of the rest
                .filter(|&q| q != ClassificationResultQuality::NoResult)
                .filter(|&q| q != ClassificationResultQuality::Wrong)
                .map(|quality| {
                    let mut num_class = tmp[&quality].clone();

                    num_class = reverse_cum_sum(&num_class);
                    Row::new(
                        Some(cell!(l->quality))
                            .into_iter()
                            .chain(num_class.into_iter().map(|c| cell!(r->c)))
                            .collect(),
                    )
                })
                .collect();
            let mut table = Table::init(counts);
            table.set_titles(header);
            table.set_format(FORMAT_NO_BORDER_UNICODE);
            table.fmt(f)?;
        }
        Ok(())
    }
}

impl<S: Eq + Hash> Default for StatsInternal<S> {
    fn default() -> Self {
        Self {
            true_domain: HashMap::default(),
            mapped_domain: HashMap::default(),
            global: StatsCounter::default(),
        }
    }
}

impl<S: Eq + Hash> Default for StatsCounter<S> {
    fn default() -> Self {
        Self {
            results: HashMap::default(),
            reasons: HashMap::default(),
        }
    }
}

impl<S: Eq + Hash> StatsCounter<S> {
    pub fn update(&mut self, result: ClassificationResultQuality, known_problems: Option<S>) {
        *self
            .results
            .entry((result, known_problems.is_some()))
            .or_default() += 1;
        if let Some(reason) = known_problems {
            *self.reasons.entry(reason).or_default() += 1;
        }
    }
}

// %%
for i in 1..11 {
    let mut collector = StatsCollector::<String>::new();
    for (domain, seqs) in &data_closed_world {
        for seq in seqs {
            let fname = PathBuf::from(OsString::from(PathBuf::from(seq.id()).file_name().unwrap()));
            let result = file_to_quality[i-1]
                .get(&fname)
                .cloned()
                .unwrap_or_else(|| (ClassificationResultQuality::Exact, None))
                .0;
            collector.update(7, domain.to_string(), domain.to_string(), result, seq.classify().map(|s| s.to_string()));
        }
    }

    println!("FPR {}0%\n{}", i, &collector);
}

// %%


