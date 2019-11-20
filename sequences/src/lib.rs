#[cfg(feature = "read_pcap")]
mod bounded_buffer;
mod constants;
pub mod distance_cost_info;
pub mod knn;
pub mod load_sequence;
#[cfg(feature = "read_pcap")]
pub mod pcap;
pub mod precision_sequence;
mod sequence_element;
mod serialization;
mod utils;

use crate::common_sequence_classifications::*;
pub use crate::{
    constants::common_sequence_classifications,
    load_sequence::{GapMode, LoadSequenceConfig, Padding, SimulatedCountermeasure},
    precision_sequence::PrecisionSequence,
    sequence_element::SequenceElement,
    utils::{
        load_all_dnstap_files_from_dir, load_all_dnstap_files_from_dir_with_config,
        load_all_files_with_extension_from_dir_with_config, Probability,
    },
};
use chrono::NaiveDateTime;
use failure::{bail, Error, ResultExt};
use fnv::FnvHasher;
use internment::Intern;
pub use load_sequence::convert_to_sequence;
use misc_utils::{fs, path::PathExt, Min};
use serde::{
    de::{Error as SerdeError, MapAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    mem,
    path::Path,
};
use string_cache::DefaultAtom as Atom;

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

impl From<&load_sequence::Query> for AbstractQueryResponse {
    fn from(other: &load_sequence::Query) -> Self {
        AbstractQueryResponse {
            time: other.end.naive_utc(),
            size: other.response_size,
        }
    }
}

// Gap + S1-S15
pub type OneHotEncoding = Vec<u16>;

/// A sequence of DNS messages and timing gaps between them.
#[derive(Clone, Debug)]
pub struct Sequence(InternedSequence, String);

#[allow(clippy::len_without_is_empty)]
impl Sequence {
    pub fn new(sequence: Vec<SequenceElement>, identifier: String) -> Sequence {
        let interned = InternedSequence::from_vec(sequence);
        Sequence(interned, identifier)
    }

    /// Load a [`Sequence`] from a file path with default configuration.
    ///
    /// See [`Sequence::from_path_with_config`] for how to customize the loading of [`Sequence`]s.
    pub fn from_path(path: &Path) -> Result<Sequence, Error> {
        Self::from_path_with_config(path, LoadSequenceConfig::default())
    }

    /// Load a [`Sequence`] from a file path. The file has to be a dnstap file.
    ///
    /// `config` allows to alter the loading according to [`LoadSequenceConfig`]
    pub fn from_path_with_config(
        path: &Path,
        config: LoadSequenceConfig,
    ) -> Result<Sequence, Error> {
        // Iterate over all file extensions, from last to first.
        for ext in path.extensions() {
            match ext.to_str() {
                Some("dnstap") => {
                    return load_sequence::dnstap_to_sequence_with_config(path, config)
                }
                Some("json") => {
                    if config != Default::default() {
                        bail!("Trying to load a Sequence from JSON with a custom LoadSequenceConfig: LoadSequenceConfig is not supported for JSON format.")
                    }
                    let seq_json = fs::read_to_string(path)
                        .with_context(|_| format!("Cannot read file `{}`", path.display()))?;
                    return Ok(serde_json::from_str(&seq_json)?);
                }
                #[cfg(feature = "read_pcap")]
                Some("pcap") => return crate::pcap::load_pcap_file(path, None, config),
                _ => {}
            }
        }
        // Fallback to the old behavior
        load_sequence::dnstap_to_sequence_with_config(path, config)
    }

    /// Return the [`Sequence`]'s identifier. Normally, the file name.
    pub fn id(&self) -> &str {
        &*self.1
    }

    /// Return the number of [`SequenceElement`]s contained
    pub fn len(&self) -> usize {
        self.as_elements().len()
    }

    /// Generalize the [`Sequence`] and make an abstract version by removing some features.
    ///
    /// * ID will be the empty string, thus allowing comparisons between different abstract Sequences
    /// * All [`Gap`][`SequenceElement::Gap`] elements will have a value of `0`, as exact values are often undesireable.
    pub fn to_abstract_sequence(&self) -> Self {
        let seq = self
            .as_elements()
            .iter()
            .map(|seqelem| match seqelem {
                SequenceElement::Gap(_) => SequenceElement::Gap(0),
                elem => *elem,
            })
            .collect();
        Sequence::new(seq, "".to_string())
    }

    /// Return a rough complexity score for the [`Sequence`]
    pub fn complexity(&self) -> usize {
        self.as_elements()
            .iter()
            .filter_map(|x| match x {
                SequenceElement::Size(n) => Some(*n as usize),
                _ => None,
            })
            .sum()
    }

    /// Return the number of [`SequenceElement::Size`] elements in the [`Sequence`]
    pub fn message_count(&self) -> usize {
        self.as_elements()
            .iter()
            .filter(|x| match x {
                SequenceElement::Size(_) => true,
                _ => false,
            })
            .count()
    }

    pub fn to_one_hot_encoding(&self) -> Vec<OneHotEncoding> {
        self.as_elements()
            .iter()
            .cloned()
            .map(SequenceElement::to_one_hot_encoding)
            .collect()
    }

    pub fn to_vector_encoding(&self) -> Vec<(u16, u16)> {
        self.as_elements()
            .iter()
            .cloned()
            .map(SequenceElement::to_vector_encoding)
            .collect()
    }

    /// Return the distance to the `other` [`Sequence`].
    pub fn distance(&self, other: &Self) -> usize {
        self.distance_with_limit::<()>(other, false, false).0
    }

    /// Same as [`Sequence::distance`] but with an early exit criteria
    ///
    /// This means that early exit can be disabled by setting `max_distance` to `usize::max_value()`, as there can be no larger value.
    ///
    /// If `use_length_prefilter` is true, the function performs an initial check, if the length of the sequences are similar enough.
    /// The idea is that sequences of largly differing lengths, cannot be similar to start with.
    /// Two sequences are similar, if
    /// * they differ by less than 40, which allows for at least 20 new requests to appear
    /// * OR they differ by less than 20% of the larger sequence
    /// whichever of those two is larger.
    pub fn distance_with_limit<DCI>(
        &self,
        other: &Self,
        use_length_prefilter: bool,
        use_cr_mode: bool,
    ) -> (usize, DCI)
    where
        DCI: distance_cost_info::DistanceCostInfo,
    {
        let mut larger = self.as_elements();
        let mut smaller = other.as_elements();

        if larger.len() < smaller.len() {
            mem::swap(&mut larger, &mut smaller);
        }
        // smaller is always shorter or equal sized

        // if we are in CR simulation mode, we can skip most of the calculation
        // we can assume that the sequences are equal except for the length
        // (this is not quite true, as sometimes the sizes also differ)
        // and both sequences follow the same structure of:
        // S (G S)*
        // therefore, we only need to add the cost of additional length of the longer one
        if use_cr_mode {
            let mut cost: usize = 0;
            let mut cost_info = DCI::default();
            for &x in &larger[smaller.len()..] {
                cost = cost.saturating_add(x.insert_cost());
                cost_info = cost_info.insert(cost, x);
            }
            return (cost, cost_info);
        }

        const ABSOLUTE_LENGTH_DIFF: usize = 40;
        const RELATIVE_LENGTH_DIFF_FACTOR: usize = 5;
        let length_diff = larger.len() - smaller.len();
        if use_length_prefilter
            && length_diff > ABSOLUTE_LENGTH_DIFF
            && length_diff > (larger.len() / RELATIVE_LENGTH_DIFF_FACTOR)
        {
            let cost_info = DCI::default().abort();
            return (usize::max_value(), cost_info);
        }

        if smaller.is_empty() {
            let mut cost: usize = 0;
            let mut cost_info = DCI::default();
            for &x in larger.iter() {
                cost = cost.saturating_add(x.insert_cost());
                cost_info = cost_info.insert(cost, x);
            }
            return (cost, cost_info);
        }

        type RowType<DCI> = Vec<(usize, DCI)>;

        let mut prev_prev_row: RowType<DCI> =
            (0..=smaller.len()).map(|_| (0, DCI::default())).collect();
        let mut cost = 0;
        let mut previous_row: RowType<DCI> = Some((0, DCI::default()))
            .into_iter()
            .chain(smaller.iter().map(|&elem| {
                cost += elem.insert_cost();
                let cost_info = DCI::default().insert(cost, elem);
                (cost, cost_info)
            }))
            .collect();
        let mut current_row: RowType<DCI> =
            (0..=smaller.len()).map(|_| (0, DCI::default())).collect();
        debug_assert_eq!(
            previous_row.len(),
            current_row.len(),
            "Row length must be equal"
        );

        for (i, &elem1) in larger.iter().enumerate() {
            current_row.clear();
            let p = previous_row[0].0 + elem1.delete_cost();
            let p_info = previous_row[0].1.delete(p, elem1);
            current_row.push((p, p_info));
            let mut min_cost_current_row: Min<usize> = Default::default();

            for (j, &elem2) in smaller.iter().enumerate() {
                let insertions = previous_row[j + 1].0 + elem1.insert_cost();
                let insertions_info = previous_row[j + 1].1.insert(insertions, elem1);
                let deletions = current_row[j].0 + elem2.delete_cost();
                let deletions_info = current_row[j].1.delete(deletions, elem2);
                let substitutions = previous_row[j].0 + elem1.substitute_cost(elem2);
                let substitutions_info = previous_row[j].1.substitute(substitutions, elem1, elem2);
                let (swapping, swapping_info) =
                    if i > 0 && j > 0 && larger[i] == smaller[j - 1] && larger[i - 1] == smaller[j]
                    {
                        let swapping = prev_prev_row[j - 1].0 + elem1.swap_cost(elem2);
                        let swapping_info = prev_prev_row[j - 1].1.swap(swapping, elem1, elem2);
                        (swapping, swapping_info)
                    } else {
                        // generate a large value but not so large, that an overflow might happen while performing some addition
                        (usize::max_value() / 4, DCI::default().abort())
                    };
                let (a, a_info) = if insertions < deletions {
                    (insertions, insertions_info)
                } else {
                    (deletions, deletions_info)
                };
                let (b, b_info) = if substitutions < swapping {
                    (substitutions, substitutions_info)
                } else {
                    (swapping, swapping_info)
                };
                let (cost, cost_info) = if a < b { (a, a_info) } else { (b, b_info) };

                // let cost = insertions.min(deletions).min(substitutions).min(swapping);
                min_cost_current_row.update(cost);
                current_row.push((cost, cost_info));
            }

            mem::swap(&mut prev_prev_row, &mut previous_row);
            mem::swap(&mut previous_row, &mut current_row);
        }

        previous_row
            .last()
            .cloned()
            .expect("The rows are never empty, thus there is a last.")
    }

    /// Return the internal slice of [`SequenceElement`]s
    pub fn as_elements(&self) -> &[SequenceElement] {
        &((self.0).0).1
    }

    pub fn classify(&self) -> Option<&'static str> {
        // Sequences of length 6 and lower were the most problematic to classify.
        // Therefore, assign all of them a reason.
        use crate::SequenceElement::{Gap, Size};
        match &*self.as_elements() {
            [] => None,
            [Size(n)] => Some(match n {
                0 => unreachable!("Packets of size 0 may never occur."),
                1 => R004_SIZE1,
                2 => R004_SIZE2,
                3 => R004_SIZE3,
                4 => R004_SIZE4,
                5 => R004_SIZE5,
                6 => R004_SIZE6,
                _ => R004_UNKNOWN,
            }),
            [Size(_), Size(_)] => Some(R102),

            // Length 3
            // One gap
            [Size(_), Gap(_), Size(_)] => Some(R102A),
            // No gap
            [Size(_), Size(_), Size(_)] => Some(R103C),

            // Length 4
            // One gap
            [Size(_), Size(_), Gap(_), Size(_)] => Some(R103A),
            [Size(_), Gap(_), Size(_), Size(_)] => Some(R103B),
            // No gap
            [Size(_), Size(_), Size(_), Size(_)] => Some(R104A),

            // Length 5
            // One gap
            [Size(_), Gap(_), Size(_), Gap(_), Size(_)] => Some(R103),
            [Size(_), Gap(_), Size(_), Size(_), Size(_)] => Some(R104B),
            [Size(_), Size(_), Gap(_), Size(_), Size(_)] => Some(R104C),
            [Size(_), Size(_), Size(_), Gap(_), Size(_)] => Some(R104D),
            // No gap
            [Size(_), Size(_), Size(_), Size(_), Size(_)] => Some(R105A),

            // Length 6
            // Two gaps
            [Size(_), Gap(_), Size(_), Gap(_), Size(_), Size(_)] => Some(R104E),
            [Size(_), Gap(_), Size(_), Size(_), Gap(_), Size(_)] => Some(R104F),
            [Size(_), Size(_), Gap(_), Size(_), Gap(_), Size(_)] => Some(R104G),
            // One gap
            [Size(_), Gap(_), Size(_), Size(_), Size(_), Size(_)] => Some(R105B),
            [Size(_), Size(_), Gap(_), Size(_), Size(_), Size(_)] => Some(R105C),
            [Size(_), Size(_), Size(_), Gap(_), Size(_), Size(_)] => Some(R105D),
            [Size(_), Size(_), Size(_), Size(_), Gap(_), Size(_)] => Some(R105E),
            // No gap
            [Size(_), Size(_), Size(_), Size(_), Size(_), Size(_)] => Some(R106A),

            _ => None,
        }
    }

    pub fn to_json(&self) -> Result<String, Error> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn intern(&self) -> InternedSequence {
        self.0
    }
}

impl PartialEq for Sequence {
    fn eq(&self, other: &Self) -> bool {
        // compare IDs first, only then the sequences
        self.1 == other.1 && self.0 == other.0
    }
}

impl Eq for Sequence {}

impl Hash for Sequence {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        self.1.hash(state);
        self.0.hash(state);
    }
}

impl Ord for Sequence {
    fn cmp(&self, other: &Self) -> Ordering {
        self.complexity()
            .cmp(&other.complexity())
            .then_with(|| self.1.cmp(&other.1))
    }
}

impl PartialOrd for Sequence {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Serialize for Sequence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map_ser = serializer.serialize_map(Some(1))?;
        map_ser.serialize_entry(&self.1, &(self.0).0)?;
        map_ser.end()
    }
}

impl<'de> Deserialize<'de> for Sequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper;

        impl<'de> Visitor<'de> for Helper {
            type Value = Sequence;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "valid JSON object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let entry = map.next_entry()?;
                if let Some(entry) = entry {
                    Ok(Sequence::new(entry.1, entry.0))
                } else {
                    Err(SerdeError::custom("The map must contain one element."))
                }
            }
        }

        deserializer.deserialize_map(Helper)
    }
}

pub struct LabelledSequence<S = Atom> {
    pub true_domain: S,
    pub mapped_domain: S,
    pub sequence: Sequence,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct LabelledSequences<S = Atom> {
    pub true_domain: S,
    pub mapped_domain: S,
    pub sequences: Vec<Sequence>,
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

#[derive(Copy, Clone)]
pub struct InternedSequence(Intern<(u64, Vec<SequenceElement>)>);

impl InternedSequence {
    fn new(intern: Intern<(u64, Vec<SequenceElement>)>) -> Self {
        Self(intern)
    }

    fn from_vec(sequence: Vec<SequenceElement>) -> Self {
        let mut hasher = FnvHasher::default();
        sequence.hash(&mut hasher);
        let hash = hasher.finish();
        Self::new(Intern::new((hash, sequence)))
    }
}

impl PartialEq for InternedSequence {
    fn eq(&self, other: &Self) -> bool {
        // Rely on pointer comparison of Intern
        self.0.eq(&other.0)
    }
}

impl Eq for InternedSequence {}

impl Hash for InternedSequence {
    fn hash<H>(&self, state: &mut H)
    where
        H: std::hash::Hasher,
    {
        (self.0).0.hash(state);
    }
}

impl Ord for InternedSequence {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare hash first, then sequence
        (self.0)
            .0
            .cmp(&(other.0).0)
            .then_with(|| (self.0).1.cmp(&(other.0).1))
    }
}

impl PartialOrd for InternedSequence {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Debug for InternedSequence {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        (self.0).1.fmt(fmt)
    }
}

#[cfg(test)]
mod test_edit_dist {
    use super::{
        Sequence,
        SequenceElement::{Gap, Size},
    };

    #[test]
    fn test_edit_distance_dist1() {
        let seq1 = Sequence::new(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)], "".into());

        // substitution
        let seq2 = Sequence::new(vec![Size(2), Gap(2), Size(1), Size(2), Size(1)], "".into());
        assert_eq!(6, seq1.distance(&seq2));
        let seq2b = Sequence::new(vec![Size(1), Gap(3), Size(1), Size(2), Size(1)], "".into());
        assert_eq!(3, seq1.distance(&seq2b));

        // swapping
        let seq3 = Sequence::new(vec![Size(1), Gap(2), Size(2), Size(1), Size(1)], "".into());
        assert_eq!(3, seq1.distance(&seq3));

        // deletion
        let seq4 = Sequence::new(vec![Size(1), Size(1), Size(2), Size(1)], "".into());
        assert_eq!(2, seq1.distance(&seq4));

        // insertion
        let seq5 = Sequence::new(
            vec![Size(1), Size(2), Gap(2), Size(1), Size(2), Size(1)],
            "".into(),
        );
        assert_eq!(12, seq1.distance(&seq5));
    }

    #[test]
    fn test_edit_distance_inserts() {
        let seq1 = Sequence::new(vec![], "".into());
        let seq2 = Sequence::new(vec![Size(1), Size(1)], "".into());

        let seq6 = Sequence::new(vec![Gap(3)], "".into());
        let seq7 = Sequence::new(vec![Gap(10)], "".into());
        println!("Smaller gap: {}", seq1.distance(&seq6));
        println!("Bigger gap: {}", seq1.distance(&seq7));
        assert!(
            seq1.distance(&seq6) < seq1.distance(&seq7),
            "Bigger Gaps have higher cost."
        );

        let seq6 = Sequence::new(vec![Size(1), Gap(3), Size(1)], "".into());
        let seq7 = Sequence::new(vec![Size(1), Gap(10), Size(1)], "".into());
        println!("Smaller gap: {}", seq2.distance(&seq6));
        println!("Bigger gap: {}", seq2.distance(&seq7));
        assert!(
            seq2.distance(&seq6) < seq2.distance(&seq7),
            "Bigger Gaps have higher cost."
        );
    }

    #[test]
    fn test_edit_distance_substitutions() {
        let seq1 = Sequence::new(vec![Size(1)], "".into());
        let seq2 = Sequence::new(vec![Gap(10)], "".into());

        let seqa = Sequence::new(vec![Gap(9)], "".into());
        let seqb = Sequence::new(vec![Gap(1)], "".into());
        println!("Smaller gap change: {}", seq2.distance(&seqa));
        println!("Bigger gap change: {}", seq2.distance(&seqb));
        assert!(
            seq2.distance(&seqa) < seq2.distance(&seqb),
            "Bigger Gap changes have higher cost."
        );

        println!("Size to Gap change: {}", seq1.distance(&seqa));
        println!("Gap to Gap change: {}", seq2.distance(&seqa));
        assert!(
            seq1.distance(&seqa) > seq2.distance(&seqa),
            "Gap to Gap change is smaller than Size to Gap change"
        )
    }

    #[test]
    fn test_edit_distance_equal() {
        let seq1 = Sequence::new(vec![], "".into());
        let seq2 = Sequence::new(vec![], "".into());
        assert_eq!(seq1, seq2);
        assert_eq!(0, seq1.distance(&seq2));

        let seq3 = Sequence::new(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)], "".into());
        let seq4 = Sequence::new(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)], "".into());
        assert_eq!(0, seq3.distance(&seq4));
    }
}
