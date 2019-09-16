// ---
// jupyter:
//   jupytext:
//     formats: ipynb,rs:percent
//     text_representation:
//       extension: .rs
//       format_name: percent
//       format_version: '1.2'
//       jupytext_version: 1.1.2
//   kernelspec:
//     display_name: Rust
//     language: rust
//     name: rust
// ---

// %%
:timing

// %%
:dep sequences = { path = "/home/jbushart/projects/encrypted-dns/sequences", features = [ "read_pcap" ] }
:dep serde = {version = "1.0.90", features = [ "derive" ]}

// %%
extern crate sequences;
extern crate glob;
extern crate failure;
extern crate rayon;
extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate rayon;
extern crate misc_utils;

// %%
use glob::glob;
use sequences::Sequence;
use rayon::prelude::*;
use std::path::PathBuf;
use sequences::PrecisionSequence;
use chrono::Duration;
use sequences::Probability;
use rayon::prelude::*;
use std::collections::BTreeMap;

// %%
let pseqs: Vec<PrecisionSequence> = serde_json::from_str(&std::fs::read_to_string("pass-pcap.json").unwrap()).unwrap();
pseqs[0]

// %%
let mut converted_sequences: BTreeMap<String, BTreeMap<String, Vec<PrecisionSequence>>> = Default::default();

// %%
let base_path = PathBuf::from("simulated/baseline");
for seq in &pseqs {
    let mut fpath = base_path.join(seq.id());
    fpath.set_extension("json");

    // Add precision sequence to map
    let domain = fpath.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
    let category = fpath.parent().unwrap().parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
    converted_sequences.entry(category).or_default().entry(domain).or_default().push(seq.clone());
}

// %%
// Duration in ms
for &duration in &[100, 50, 25, 12] {
//     for &prob in &[0.95, 0.9, 0.85, 0.8] {
//     for &prob in &[0.75, 0.7, 0.65, 0.6, 0.55, 0.5] {
    for &prob in &[0.4, 0.3, 0.2, 0.1] {
        let new_seqs: Vec<_> = pseqs.par_iter().map(|pseq| {
            pseq.apply_constant_rate(Duration::milliseconds(duration), Probability::new(prob).unwrap())//.to_sequence()
        }).collect();
        let base_path = PathBuf::from(format!("simulated/cr-{}ms-{}p", duration, prob));
        for seq in new_seqs {
            let mut fpath = base_path.join(seq.id());
            fpath.set_extension("json");

            // Write sequence to disc
            std::fs::create_dir_all(fpath.parent().unwrap()).unwrap();
            std::fs::write(&fpath, &seq.to_sequence().to_json().unwrap()).unwrap();

            // Add precision sequence to map
            let domain = fpath.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
            let category = fpath.parent().unwrap().parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
            converted_sequences.entry(category).or_default().entry(domain).or_default().push(seq);
        }
    }
}

// %%
// Duration in ms
let median_burst_length = 2;
// for &prob in &[0.95, 0.9, 0.85, 0.8] {
// for &prob in &[0.75, 0.7, 0.65, 0.6, 0.55, 0.5] {
for &prob in &[0.4, 0.3, 0.2, 0.1] {
    let new_seqs: Vec<_> = pseqs.par_iter().map(|pseq| {
        pseq.apply_adaptive_padding(median_burst_length, Probability::new(prob).unwrap())//.to_sequence()
    }).collect();
    let base_path = PathBuf::from(format!("simulated/ap-{}length-{}p", median_burst_length, prob));
    for seq in new_seqs.into_iter() {
        let mut fpath = base_path.join(seq.id());
        fpath.set_extension("json");

        // Write sequence to disc
        std::fs::create_dir_all(fpath.parent().unwrap()).unwrap();
        std::fs::write(&fpath, &seq.to_sequence().to_json().unwrap()).unwrap();

        // Add precision sequence to map
        let domain = fpath.parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
        let category = fpath.parent().unwrap().parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
        converted_sequences.entry(category).or_default().entry(domain).or_default().push(seq);
    }
}

// %%
let mut writer = misc_utils::fs::file_open_write("./precision-sequences10.json.xz", misc_utils::fs::WriteOptions::default().set_filetype(misc_utils::fs::FileType::Xz)).unwrap();
serde_json::to_writer(writer, &converted_sequences).unwrap();

// %%
let oh = pseqs[12].overhead(&pseqs[12].apply_adaptive_padding(2, Probability::new(0.95).unwrap()));

// %%
oh

// %%
oh + oh + oh

// %%
:clear

// %%
