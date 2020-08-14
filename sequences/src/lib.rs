mod constants;
pub mod dnstap;
pub mod load_sequence;
#[cfg(feature = "read_pcap")]
pub mod pcap;
pub mod precision_sequence;
mod sequence;
mod utils;

pub use crate::{
    constants::common_sequence_classifications,
    load_sequence::{
        convert_to_sequence, GapMode, LoadSequenceConfig, Padding, SimulatedCountermeasure,
    },
    precision_sequence::PrecisionSequence,
    sequence::{distance_cost_info, knn, OneHotEncoding, Sequence, SequenceElement},
    utils::{load_all_files_with_extension_from_dir_with_config, Probability},
};
use chrono::NaiveDateTime;

/// Interaperability type used when building sequences
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct AbstractQueryResponse {
    pub time: NaiveDateTime,
    pub size: u32,
}

impl From<&AbstractQueryResponse> for AbstractQueryResponse {
    fn from(other: &AbstractQueryResponse) -> Self {
        *other
    }
}

pub fn sequence_stats(
    sequences_a: &[Sequence],
    sequences_b: &[Sequence],
) -> (Vec<usize>, Vec<usize>, usize, usize) {
    let dists: Vec<Vec<usize>> = sequences_a
        .iter()
        .map(|seq| {
            sequences_b
                .iter()
                .filter(|other_seq| seq != *other_seq)
                .map(|other_seq| seq.distance(&other_seq))
                .collect()
        })
        .collect();

    let avg_distances: Vec<_> = dists
        .iter()
        .map(|dists2| {
            if !dists2.is_empty() {
                dists2.iter().sum::<usize>() / dists2.len()
            } else {
                0
            }
        })
        .collect();
    let median_distances: Vec<_> = dists
        .into_iter()
        .map(|mut dists2| {
            if !dists2.is_empty() {
                dists2.sort();
                dists2[dists2.len() / 2]
            } else {
                0
            }
        })
        .collect();
    let avg_avg = avg_distances.iter().sum::<usize>() / avg_distances.len();
    let avg_median = median_distances.iter().sum::<usize>() / median_distances.len();

    (avg_distances, median_distances, avg_avg, avg_median)
}
