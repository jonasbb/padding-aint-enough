// ---
// jupyter:
//   jupytext:
//     formats: ipynb,rs:percent
//     text_representation:
//       extension: .rs
//       format_name: percent
//       format_version: '1.2'
//       jupytext_version: 0.8.6
//   kernelspec:
//     display_name: Rust
//     language: rust
//     name: rust
// ---

// %%
:help

// %%
:dep sequences = { path = "/home/jbushart/projects/encrypted-dns/sequences"}

// %%
extern crate sequences;
extern crate rayon;
extern crate serde_json;

// %%
use rayon::prelude::*;
use std::fs;
use sequences::load_all_dnstap_files_from_dir;

// %%
let data = load_all_dnstap_files_from_dir(&"/mnt/data/Downloads/dnscaptures-main-group".as_ref()).unwrap();

// %%
data.len()

// %%
let tmp: Vec<Vec<Vec<(usize, usize, usize)>>> = data.par_iter().map(|(label, seqs)| -> Vec<Vec<(usize, usize, usize)>> {
    seqs.par_iter().map(|s1| -> Vec<(usize, usize, usize)> {
        seqs.par_iter().filter(|&s2| s1 != s2).map(|s2| {
            (s1.distance(s2), s1.as_elements().len(), s2.as_elements().len())
        }).collect()
    }).collect()
}).collect();

// %%
let s = serde_json::to_string(&tmp).unwrap();

// %%
fs::write("./distances-per-cluster.json", s)

// %%
:vars

// %%

