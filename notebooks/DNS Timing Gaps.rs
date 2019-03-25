// ---
// jupyter:
//   jupytext:
//     formats: ipynb,auto:percent
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
:dep dnstap = { path = "/home/jbushart/projects/encrypted-dns/dnstap"}

// %%
//:help

// %%
extern crate chrono;
extern crate counter;
extern crate dnstap;
extern crate glob;
extern crate itertools;

// %%
use chrono::Duration;
use counter::Counter;
use dnstap::{
    dnstap::Message_Type,
    process_dnstap,
    protos::{self, DnstapContent},
    sanity_check_dnstap,
};
use itertools::Itertools;
use std::path::{Path, PathBuf};

// %%
// fn main() {

// %%
pub fn extract_gaps<P: AsRef<Path>>(file: P) -> Vec<Duration> {
    let file = file.as_ref();
    let mut events: Vec<protos::Dnstap> = process_dnstap(&*file)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

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

    let client_timings = events
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
                let (dnsmsg, _size) = query_message.as_ref().expect("Unbound always sets this");
                let qname = dnsmsg.queries()[0].name().to_utf8();
                if qname == "end.example." {
                    return false;
                }
            }
            true
        })
        .filter_map(|ev| {
            let DnstapContent::Message {
                message_type,
                query_message,
                response_message,
                query_time,
                response_time,
                ..
            } = ev.content;
            match message_type {
//                 Message_Type::CLIENT_QUERY => {
                Message_Type::FORWARDER_QUERY => {
                    Some(query_time.expect("Unbound always sets this"))
                }

                _ => None,
            }
        })
        .collect::<Vec<_>>();

    client_timings
        .windows(2)
        .map(|w| match w {
            &[a, b] => b - a,
            _ => unreachable!(),
        })
        .collect()
}

// %%
let files: Vec<PathBuf> =
    glob::glob("/home/jbushart/projects/data/dnscaptures-main-group/*/*.xz")
        .map(|paths| paths.filter_map(|p| p.ok()).collect())
        .unwrap_or_else(|_| Vec::new());
let durations_in_microseconds: Vec<i64> = files
    .iter()
    .flat_map(extract_gaps)
    .map(|d| d.num_microseconds().unwrap())
    .sorted()
    .collect();

// %%
// Convert duration into a exponentally sized buckets of 2^i
let counts = durations_in_microseconds
    .iter()
    .cloned()
    .collect::<Counter<_>>();

// %%
let peaks = counts
    .iter()
    .sorted()
    .collect::<Vec<_>>()
    .windows(500)
    .filter_map(|w| -> Option<(u64, usize)> {
        // Select element, if larger than all the other
        let (&value, &count) = w[w.len() / 2];
        if !(w.iter().any(|(v, &c)| c > count)) {
            // round value to 1000s
            let round_to = 1000.;
            let value = ((value as f64 / round_to).round() * round_to) as u64;
            Some((value, count))
        } else {
            None
        }
    })
    .collect::<Vec<_>>();

// %%
durations_in_microseconds
    .iter()
    .map(|d| 64-d.leading_zeros())
    .collect::<Counter<_>>().iter().sorted()

// %%
std::fs::write(
    "peaks.csv",
    peaks
        .iter()
        .map(|(d, count)| format!("{},{}", d, count))
        .collect::<Vec<_>>()
        .join("\n"),
);

// %%
std::fs::write(
    "gaps.csv",
    counts
        .iter()
        .sorted()
        .map(|(d, count)| format!("{},{}", d, count))
        .collect::<Vec<_>>()
        .join("\n"),
);

// %%
// Calculate `steps` points for the x and y axis, where the y axis is the cummulutive sum
pub fn fractions<T>(iter: Vec<T>, steps: usize) -> Vec<(T, usize)> {
    let total = iter.len();
    let step_size = if steps > 0 {
        total / (steps - 1)
    } else {
        total
    };
    iter.into_iter()
        .enumerate()
        .filter_map(|(i, x)| {
            if (i % step_size == 0) || i == total - 1 {
                Some((x, i))
            } else {
                None
            }
        })
        .collect()
}

// %%
for (a, b) in fractions(durations_in_microseconds.clone(), 3) {
    println!("{},{}", a, b)
}

// %%
let burst_lengths: Vec<usize> = {
    files
        .iter()
        .flat_map(|x| {
            let groups = extract_gaps(x)
            .into_iter()
            .map(|d| d.num_microseconds().unwrap())
            .group_by(|&d| d < 1000);
            groups
                .into_iter()
                .map(|(_key, group)| {
                    {group.count()}
                })
                .collect::<Vec<_>>()
        })
        .collect()
};

// %%
std::fs::write(
    "burst_lengths.csv",
    burst_lengths.iter().cloned().collect::<Counter<_>>()
        .iter()
        .sorted()
        .map(|(d, count)| format!("{},{}", d, count))
        .collect::<Vec<_>>()
        .join("\n"),
);

// %%
// }

// %%
:last_error_json


// %%
// :clear

// %%

