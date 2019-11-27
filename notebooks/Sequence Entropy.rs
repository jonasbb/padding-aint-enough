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
// # Calculating Entropy
//
// Ideas:
//
// * Use the probability of seeing certain Gap() and Size() in different n-grams (n=1,2,3)
// * Use the length
//     * Use the length, but remove the gaps
// * Use how often this exact sequence appears in the dataset

// %%
// :dep sequences = { path = "/home/jbushart/projects/encrypted-dns/sequences"}

// %%
// :dep serde = { version = "1.0.84", features = [ "derive" ] }

// %%
extern crate itertools;
extern crate misc_utils;
extern crate rayon;
extern crate sequences;
extern crate serde_json;
extern crate serde_with;
extern crate serde;
extern crate string_cache;

// %%
use itertools::Itertools;
use misc_utils::fs::*;
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
    path::{Path, PathBuf},
};
use string_cache::DefaultAtom as Atom;

// %% [markdown]
// # Load dnstap files for "entropy" calculation

// %%
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
pub struct Entropy {
    pub length: usize,
    pub count_messages: usize,
    pub complexity: usize,
    pub shannon_n1: f64,
    pub shannon_n2: f64,
    pub shannon_n3: f64,
}

// %%
// let data_closed_world = load_all_dnstap_files_from_dir(&"/home/jbushart/projects/data/dnscaptures-main-group".as_ref()).unwrap();
// let data_open_world = load_all_dnstap_files_from_dir(&"/home/jbushart/projects/data/dnscaptures-open-world".as_ref()).unwrap();
// let data: Vec<(String, Vec<Sequence>)> = data_closed_world.iter().cloned().chain(data_open_world.iter().cloned()).collect();

let data_closed_world = load_all_files_with_extension_from_dir_with_config(&"/mnt/data/Downloads/dnscaptures-2019-11-18-full-rescan/extracted/0".as_ref(), "pcap".as_ref(), Default::default()).unwrap();

let data = data_closed_world.clone();
// let data = data_open_world.clone();

// %%
let mut counter_elem_n1: HashMap<SequenceElement, usize> = HashMap::default();
let mut counter_elem_n2: HashMap<(SequenceElement, SequenceElement), usize> = HashMap::default();
let mut counter_elem_n3: HashMap<(SequenceElement, SequenceElement, SequenceElement), usize> = HashMap::default();
let mut counter_lengths: HashMap<usize, usize> = HashMap::default();
let mut counter_lengths_only_size: HashMap<usize, usize> = HashMap::default();
let mut counter_complexity: HashMap<usize, usize> = HashMap::default();

for (domain, seqs) in &data {
    for seq in seqs {
        let length_size_only = seq.as_elements()
            .iter()
            .filter(|&&elem| {
                if let SequenceElement::Size(_) = elem {
                    true
                } else {
                    false
                }
            }).count();

        for &elem in seq.as_elements() {
            *counter_elem_n1.entry(elem).or_insert(0) += 1;
        }
        for elem in seq.as_elements().iter().cloned().tuple_windows::<(_,_)>() {
            *counter_elem_n2.entry(elem).or_insert(0) += 1;
        }
        for elem in seq.as_elements().iter().cloned().tuple_windows::<(_,_,_)>() {
            *counter_elem_n3.entry(elem).or_insert(0) += 1;
        }
        *counter_lengths.entry(seq.len()).or_insert(0) += 1;
        *counter_lengths_only_size.entry(length_size_only).or_insert(0) += 1;
        *counter_complexity.entry(seq.complexity()).or_insert(0) += 1;
    }
};
{
    let mut counter_sorted: Vec<_> = counter_elem_n1.iter().collect();
    counter_sorted.sort_by_key(|x| x.0);
    dbg!(counter_elem_n1.len());
//     for (elem, count) in counter_sorted {
//         println!("{:?}: {:>7}", elem, count);
//     }

    let mut counter_sorted: Vec<_> = counter_elem_n2.iter().collect();
    counter_sorted.sort_by_key(|x| x.0);
    dbg!(counter_elem_n2.len());
//     for (elem, count) in counter_sorted {
//         println!("{:?}: {:>7}", elem, count);
//     }

    let mut counter_sorted: Vec<_> = counter_elem_n3.iter().collect();
    counter_sorted.sort_by_key(|x| x.0);
    dbg!(counter_elem_n3.len());
//     for (elem, count) in counter_sorted {
//         println!("{:?}: {:>7}", elem, count);
//     }

    let mut counter_sorted: Vec<_> = counter_lengths.iter().collect();
    counter_sorted.sort_by_key(|x| x.0);
    dbg!(counter_lengths.len());
//     for (elem, count) in counter_sorted {
//         println!("{:?}: {:>7}", elem, count);
//     }

    let mut counter_sorted: Vec<_> = counter_lengths_only_size.iter().collect();
    counter_sorted.sort_by_key(|x| x.0);
    dbg!(counter_lengths_only_size.len());
//     for (elem, count) in counter_sorted {
//         println!("{:?}: {:>7}", elem, count);
//     }

    let mut counter_sorted: Vec<_> = counter_complexity.iter().collect();
    counter_sorted.sort_by_key(|x| x.0);
    dbg!(counter_complexity.len());
//     for (elem, count) in counter_sorted {
//         println!("{:?}: {:>7}", elem, count);
//     }
}

// %%
// For the shannon entropy calculate the probability of seeing a certain thing
let counter_elem_n1_total: usize = counter_elem_n1.values().sum();
let prop_n1: HashMap<SequenceElement, f64> = counter_elem_n1.iter()
    .map(|(se, &count)| (*se, count as f64 / counter_elem_n1_total as f64))
    .collect();
let counter_elem_n2_total: usize = counter_elem_n2.values().sum();
let prop_n2: HashMap<(SequenceElement, SequenceElement), f64> = counter_elem_n2.iter()
    .map(|(se, &count)| (*se, count as f64 / counter_elem_n2_total as f64))
    .collect();
let counter_elem_n3_total: usize = counter_elem_n3.values().sum();
let prop_n3: HashMap<(SequenceElement, SequenceElement, SequenceElement), f64> = counter_elem_n3.iter()
    .map(|(se, &count)| (*se, count as f64 / counter_elem_n3_total as f64))
    .collect();

// %%
pub fn calculate_shannon_entropy<
    T: std::hash::Hash + Eq,
    I: IntoIterator<Item = T>,
>(
    iter: I,
    propabilities: &HashMap<T, f64>,
) -> f64 {
    let mut entropy_sum: f64 = 0.;
    let mut count: usize = 0;
    for t in iter {
        entropy_sum += propabilities[&t];
        count += 1;
    }
    entropy_sum / count as f64
}

// %%
let file_to_entropy: HashMap<PathBuf, Entropy> =
    data.iter()
    .flat_map(|(_, seqs)| seqs)
    .map(|seq| {
        let fname = PathBuf::from(OsString::from(PathBuf::from(seq.id()).file_name().unwrap()));
        let e = Entropy {
            length: seq.len(),
            count_messages: seq.message_count(),
            complexity: seq.complexity(),
            shannon_n1: calculate_shannon_entropy(seq.as_elements().iter().cloned(), &prop_n1),
            shannon_n2: calculate_shannon_entropy(seq.as_elements().iter().cloned().tuple_windows::<(_,_)>(), &prop_n2),
            shannon_n3: calculate_shannon_entropy(seq.as_elements().iter().cloned().tuple_windows::<(_,_,_)>(), &prop_n3),
        };
        (fname, e)
    })
    .collect();

// %% [markdown]
// # Load misclassification results, to determine result quality

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
// let (mis_open_world, mis_closed_world) = rayon::join(
//     || load_misclassifications(&"../results/2019-01-15-open-world-no-thres-mis.json"),
//     || load_misclassifications(&"../results/2019-01-11-closed-world-no-thres-mis.json")
// );
// dbg!(mis_open_world.len());
// dbg!(mis_closed_world.len());

let mis_closed_world = load_misclassifications(&"../results/2019-11-18-full-rescan/crossvalidate/crossvalidate-miss-0.json.xz");
dbg!(mis_closed_world.len());

// %%
let file_to_quality: HashMap<PathBuf, ClassificationResultQuality> = /*mis_open_world.iter()
    .chain(*/mis_closed_world.iter()/*)*/
    .filter(|m| m.k == 7)
    .map(|m| (m.id.clone(), m.determine_quality()))
    .collect();

// %% [markdown]
// # Combine above's data and write to file

// %%
// We need to use a std HashMap, as
let file_to_something: std::collections::HashMap<PathBuf, (Entropy, ClassificationResultQuality)> =
    file_to_entropy.iter()
    .map(|(file, ent)| {
        let q = file_to_quality.get(file).cloned().unwrap_or(ClassificationResultQuality::Exact);
        (file.clone(), (ent.clone(), q))
    })
    .collect();

// %%
// Write all the data into a file to make it usable from python
std::fs::write("./sequences-stats.json", &*serde_json::to_string(&file_to_something).unwrap())
