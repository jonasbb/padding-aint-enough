mod constants;
pub mod knn;
mod load_sequence;
mod serialization;
mod utils;

pub use crate::utils::{
    load_all_dnstap_files_from_dir, load_all_dnstap_files_from_dir_with_config,
};
use crate::{common_sequence_classifications::*, constants::*};
use chrono::{self, DateTime, NaiveDateTime, Utc};
use failure::{self, Error};
use lazy_static::lazy_static;
pub use load_sequence::convert_to_sequence;
use misc_utils::{self, Min};
use serde::{
    self,
    de::{MapAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    collections::HashMap,
    fmt::{self, Debug},
    hash::Hash,
    mem,
    path::Path,
    sync::{Arc, RwLock},
};
use string_cache::{self, DefaultAtom as Atom};

lazy_static! {
    static ref LOADING_FAILED: RwLock<Arc<HashMap<Atom, &'static str>>> = RwLock::default();
}

#[allow(clippy::implicit_hasher)]
pub fn replace_loading_failed(new_data: HashMap<Atom, &'static str>) {
    *LOADING_FAILED
        .write()
        .expect("Writing to LOADING_FAILED should always work") = Arc::new(new_data);
}

pub mod common_sequence_classifications {
    // These patterns were generated for traces using DNSSEC
    pub const R001: &str = "R001 Single Domain. A + DNSKEY";
    pub const R002: &str = "R002 Single Domain with www redirect. A + DNSKEY + A (for www)";
    pub const R003: &str = "R003 Two domains for website. (A + DNSKEY) * 2";
    pub const R004_SIZE1: &str = "R004 Single packet of size 1.";
    pub const R004_SIZE2: &str = "R004 Single packet of size 2.";
    pub const R004_SIZE3: &str = "R004 Single packet of size 3.";
    pub const R004_SIZE4: &str = "R004 Single packet of size 4.";
    pub const R004_SIZE5: &str = "R004 Single packet of size 5.";
    pub const R004_SIZE6: &str = "R004 Single packet of size 6.";
    pub const R004_UNKNOWN: &str = "R004 A single packet of unknown size.";
    pub const R005: &str = "R005 Two domains for website second is CNAME.";
    pub const R006: &str = "R006 www redirect + Akamai";
    pub const R006_3RD_LVL_DOM: &str =
        "R006 www redirect + Akamai on 3rd-LVL domain without DNSSEC";
    pub const R007: &str = "R007 Unreachable Name Server";
    pub const R008: &str =
        "R008 Domain did not load properly and Chrome performed a Google search on the error page.";
    pub const R009: &str = "R009 No network response received.";

    // These patterns are intended for traces without DNSSEC
    pub const R102: &str = "R102 Single Domain with www redirect. A + A (for www)";
    pub const R102A: &str = "R102A Single Domain with www redirect. A + A (for www). Missing gap.";
    pub const R103: &str = "R103 Three Domain requests. Can sometimes be R102 with an erroneous `ssl.gstatic.com` or similar.";
    pub const R103A: &str = "R103A Three Domain requests. Missing first gap.";
    pub const R103B: &str = "R103B Three Domain requests. Missing second gap.";
    pub const R103C: &str = "R103C Three Domain requests. No gaps.";
    pub const R104A: &str = "R104A Four Domain requests. No gaps.";
    pub const R104B: &str = "R104B Four Domain requests. Gap after one.";
    pub const R104C: &str = "R104C Four Domain requests. Gap after two.";
    pub const R104D: &str = "R104D Four Domain requests. Gap after three.";
    pub const R104E: &str = "R104E Four Domain requests.";
    pub const R104F: &str = "R104F Four Domain requests.";
    pub const R104G: &str = "R104G Four Domain requests.";
    pub const R105A: &str = "R105A Five Domain requests. No gaps.";
    pub const R105B: &str = "R105B Five Domain requests.";
    pub const R105C: &str = "R105C Five Domain requests.";
    pub const R105D: &str = "R105D Five Domain requests.";
    pub const R105E: &str = "R105E Five Domain requests.";
    pub const R106A: &str = "R106A Five Domain requests. No gaps.";
}

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

impl From<&Query> for AbstractQueryResponse {
    fn from(other: &Query) -> Self {
        AbstractQueryResponse {
            time: other.end.naive_utc(),
            size: other.response_size,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum LoadDnstapConfig {
    /// Load the Dnstap file normally
    Normal,
    /// Assume perfect padding is applied.
    ///
    /// This removes all [`SequenceElement::Size`] from the [`Sequence`]
    PerfectPadding,
    /// Assume perfect timing defense
    ///
    /// This removes all [`SequenceElement::Gap`] from the [`Sequence`]
    PerfectTiming,
}

// Gap + S1-S15
pub type OneHotEncoding = Vec<u8>;

#[derive(Clone, Debug)]
pub struct Sequence(Vec<SequenceElement>, String);

#[allow(clippy::len_without_is_empty)]
impl Sequence {
    pub fn new(sequence: Vec<SequenceElement>, identifier: String) -> Sequence {
        Sequence(sequence, identifier)
    }

    /// Load a [`Sequence`] from a file path. The file has to be a dnstap file.
    pub fn from_path(path: &Path) -> Result<Sequence, Error> {
        load_sequence::dnstap_to_sequence(path)
    }

    /// Load a [`Sequence`] from a file path. The file has to be a dnstap file.
    ///
    /// `config` allows to alter the loading according to [`LoadDnstapConfig`]
    pub fn from_path_with_config(path: &Path, config: LoadDnstapConfig) -> Result<Sequence, Error> {
        load_sequence::dnstap_to_sequence_with_config(path, config)
    }

    /// Return the [`Sequence`]'s identifier. Normally, the file name.
    pub fn id(&self) -> &str {
        &*self.1
    }

    /// Return the number of [`SequenceElement`]s contained
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Generalize the [`Sequence`] and make an abstract version by removing some features.
    ///
    /// * ID will be the empty string, thus allowing comparisons between different abstract Sequences
    /// * All [`Gap`][`SequenceElement::Gap`] elements will have a value of `0`, as exact values are often undesireable.
    pub fn to_abstract_sequence(&self) -> Self {
        Sequence(
            self.0
                .iter()
                .map(|seqelem| match seqelem {
                    SequenceElement::Gap(_) => SequenceElement::Gap(0),
                    elem => *elem,
                })
                .collect(),
            "".to_string(),
        )
    }

    /// Return a rough complexity score for the [`Sequence`]
    pub fn complexity(&self) -> usize {
        self.0
            .iter()
            .filter_map(|x| match x {
                SequenceElement::Size(n) => Some(*n as usize),
                _ => None,
            })
            .sum()
    }

    /// Return the number of [`SequenceElement::Size`] elements in the [`Sequence`]
    pub fn message_count(&self) -> usize {
        self.0
            .iter()
            .filter(|x| match x {
                SequenceElement::Size(_) => true,
                _ => false,
            })
            .count()
    }

    pub fn to_one_hot_encoding(&self) -> Vec<OneHotEncoding> {
        self.0
            .iter()
            .cloned()
            .map(SequenceElement::to_one_hot_encoding)
            .collect()
    }

    /// Return the distance to the `other` [`Sequence`].
    pub fn distance(&self, other: &Self) -> usize {
        self.distance_with_limit(other, usize::max_value(), false)
    }

    /// Same as [`Sequence::distance`] but with an early exit criteria
    ///
    /// `max_distance` specifies an early exit criteria.
    /// The function will exit early, if the distance found will be larger than `max_distance` without computing the final value.
    /// The return value for an early exit will be larger than `max_distance`.
    ///
    /// This means that early exit can be disabled by setting `max_distance` to `usize::max_value()`, as there can be no larger value.
    ///
    /// If `use_length_prefilter` is true, the function performs an initial check, if the length of the sequences are similar enough.
    /// The idea is that sequences of largly differing lengths, cannot be similar to start with.
    /// Two sequences are similar, if
    /// * they differ by less than 40, which allows for at least 20 new requests to appear
    /// * OR they differ by less than 20% of the larger sequence
    /// whichever of those two is larger.
    pub fn distance_with_limit(
        &self,
        other: &Self,
        max_distance: usize,
        use_length_prefilter: bool,
    ) -> usize {
        let mut larger = self;
        let mut smaller = other;

        if larger.0.len() < smaller.0.len() {
            mem::swap(&mut larger, &mut smaller);
        }
        // smaller is always shorter or equal sized

        const ABSOLUTE_LENGTH_DIFF: usize = 40;
        const RELATIVE_LENGTH_DIFF_FACTOR: usize = 5;
        let length_diff = larger.0.len() - smaller.0.len();
        if use_length_prefilter
            && length_diff > ABSOLUTE_LENGTH_DIFF
            && length_diff > (larger.0.len() / RELATIVE_LENGTH_DIFF_FACTOR)
        {
            return usize::max_value();
        }

        if smaller.0.is_empty() {
            let mut cost: usize = 0;
            for x in &larger.0 {
                cost = cost.saturating_add(x.insert_cost());
            }
            return cost;
        }

        let mut prev_prev_row = vec![0usize; smaller.0.len() + 1];
        // let mut previous_row: Vec<usize> = (0..(smaller.0.len() + 1)).into_iter().collect();
        let mut cost = 0;
        let mut previous_row: Vec<usize> = Some(0)
            .into_iter()
            .chain(smaller.0.iter().cloned().map(SequenceElement::insert_cost))
            .map(|c| {
                cost += c;
                cost
            })
            .collect();
        let mut current_row = vec![0usize; smaller.0.len() + 1];
        debug_assert_eq!(
            previous_row.len(),
            current_row.len(),
            "Row length must be equal"
        );

        for (i, elem1) in larger.0.iter().enumerate() {
            current_row.clear();
            current_row.push(previous_row[0] + elem1.delete_cost());
            let mut min_cost_current_row: Min<usize> = Default::default();

            for (j, &elem2) in smaller.0.iter().enumerate() {
                let insertions = previous_row[j + 1] + elem1.insert_cost();
                let deletions = current_row[j] + elem2.delete_cost();
                let substitutions = previous_row[j] + elem1.substitute_cost(elem2);
                let swapping = if i > 0
                    && j > 0
                    && larger.0[i] == smaller.0[j - 1]
                    && larger.0[i - 1] == smaller.0[j]
                {
                    prev_prev_row[j - 1] + elem1.swap_cost(elem2)
                } else {
                    // generate a large value but not so large, that an overflow might happen while performing some addition
                    usize::max_value() / 4
                };
                let cost = insertions.min(deletions).min(substitutions).min(swapping);
                min_cost_current_row.update(cost);
                current_row.push(cost);
            }

            // See whether we can abort early
            // `min_cost_current_row` keeps the minimal cost which we encountered this row
            // We also know, that is the cost ever becomes larger than `max_distance`, then the result of this function is uninteresting.
            // If we see that `min_cost_current_row > max_distance`, then we know that this function can never return a result smaller than `max_distance`,
            // because there is always a cost added to the value of the previous row.
            if min_cost_current_row.get_min_extreme() > max_distance {
                return usize::max_value();
            }

            mem::swap(&mut prev_prev_row, &mut previous_row);
            mem::swap(&mut previous_row, &mut current_row);
        }

        *previous_row
            .last()
            .expect("The rows are never empty, thus there is a last.")
    }

    /// Return the internal slice of [`SequenceElement`]s
    pub fn as_elements(&self) -> &[SequenceElement] {
        &self.0
    }

    pub fn classify(&self) -> Option<&'static str> {
        {
            let lock = LOADING_FAILED
                .read()
                .expect("Reading LOADING_FAILED must always work");
            if let Some(Some(reason)) = Path::new(self.id())
                // extract file name from id
                .file_name()
                // convert to `Atom`
                .map(|file_name| Atom::from(file_name.to_string_lossy()))
                // see if this is a known bad id
                .map(|file_atom| lock.get(&file_atom))
            {
                return Some(reason);
            }
        }

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

            /*
            [SequenceElement::Size(1), SequenceElement::Size(2)] => Some(R001),
            [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1)] => {
                Some(R002)
            }
            [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1), SequenceElement::Size(2)] => {
                Some(R003)
            }
            [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(2)] => {
                Some(R005)
            }
            [SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(2)] => {
                Some(R006)
            }
            [SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(1), SequenceElement::Size(2), SequenceElement::Size(2)] => {
                Some(R006_3RD_LVL_DOM)
            }
            */
            _ => {
                // This part only makes sense when operating on DNSSEC data
                // This check should be unnecessary now, given that we check for unreachability in
                // the Chrome debugger messages and retry the loading there already
                /*
                let mut is_unreachable_domain = true;
                {
                    // Unreachable domains have many requests of Size 1 but never a DNSKEY
                    let mut iter = self.as_elements().iter().fuse();
                    // Sequence looks like for Size and Gap
                    // S G S G S G S G S
                    // we only need to loop until we find a counter proof
                    while is_unreachable_domain {
                        match (iter.next(), iter.next()) {
                            // This is the end of the sequence
                            (Some(SequenceElement::Size(1)), None) => break,
                            // this is the normal, good case
                            (Some(SequenceElement::Size(1)), Some(SequenceElement::Gap(_))) => {}

                            // This can never happen with the above pattern
                            (None, None) => is_unreachable_domain = false,
                            // Sequence may not end on a Gap
                            (Some(SequenceElement::Gap(_)), None) => is_unreachable_domain = false,
                            // all other patterns, e.g., different Sizes do not match
                            _ => is_unreachable_domain = false,
                        }
                    }
                }

                if is_unreachable_domain {
                    Some(R007)
                } else {
                    None
                }
                */
                None
            }
        }
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
        map_ser.serialize_entry(&self.1, &self.0)?;
        map_ser.end()
    }
}

impl<'de> Deserialize<'de> for Sequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper;
        use serde::de::Error;

        impl<'de> Visitor<'de> for Helper where {
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
                    Ok(Sequence(entry.1, entry.0))
                } else {
                    Err(Error::custom("The map must contain one element."))
                }
            }
        }

        deserializer.deserialize_map(Helper)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SequenceElement {
    Size(u8),
    Gap(u8),
}

impl SequenceElement {
    fn insert_cost(self) -> usize {
        use self::SequenceElement::*;

        debug_assert_ne!(self, Size(0), "Sequence contains a Size(0) elements");

        match self {
            // Size(0) => {
            //     // A size 0 packet should never occur
            //     error!("Sequence contains a Size(0) elements");
            //     usize::max_value()
            // }
            Size(_) => SIZE_INSERT_COST,
            Gap(g) => g as usize * GAP_INSERT_COST_MULTIPLIER,
        }
    }

    fn delete_cost(self) -> usize {
        // The delete costs have to be identical to the insert costs in order to be a metric.
        // There is no order in which two Sequences will be compared, so
        // xABCy -> xACy
        // must be the same as
        // xACy -> xABCy
        self.insert_cost()
    }

    fn substitute_cost(self, other: Self) -> usize {
        if self == other {
            return 0;
        }

        use self::SequenceElement::*;
        match (self, other) {
            // 2/3rds cost of insert
            (Size(_), Size(_)) => {
                (self.insert_cost() + other.delete_cost()) / SIZE_SUBSTITUTE_COST_DIVIDER
            }
            (Gap(g1), Gap(g2)) => {
                (g1.max(g2) - g1.min(g2)) as usize * GAP_SUBSTITUTE_COST_MULTIPLIER
            }
            (a, b) => a.delete_cost() + b.insert_cost(),
        }
    }

    fn swap_cost(self, other: Self) -> usize {
        if self == other {
            return 0;
        }

        SWAP_COST
    }

    fn to_one_hot_encoding(self) -> OneHotEncoding {
        use self::SequenceElement::*;
        let mut res = vec![0; 16];
        let len = res.len();
        match self {
            Size(0) => unreachable!(),
            Size(s) if s < len as u8 => res[s as usize] = 1,
            Gap(g) => res[0] = g,

            Size(s) => panic!("One Hot Encoding only works for Sequences not exceeding a Size({}), but found a Size({})", len - 1, s),
        }
        res
    }
}

impl Debug for SequenceElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::SequenceElement::*;
        let (l, v) = match self {
            Size(v) => ("S", v),
            Gap(v) => ("G", v),
        };
        write!(f, "{}{:>2}", l, v)
    }
}

impl Serialize for SequenceElement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let res = match self {
            SequenceElement::Gap(g) => format!("G{:0>2}", g),
            SequenceElement::Size(s) => format!("S{:0>2}", s),
        };
        serializer.serialize_str(&res)
    }
}

impl<'de> Deserialize<'de> for SequenceElement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper;
        use serde::de::Error;

        impl<'de> Visitor<'de> for Helper where {
            type Value = SequenceElement;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "string in format `S00` or `G00`")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let chars = value.chars().count();
                if chars != 3 {
                    return Err(Error::custom(format!("The string must be of length 3 (but got {}), in the format `S00` or `G00`.", chars)));
                }
                let start = value.chars().next().expect("String is 3 chars large.");
                match start {
                    'G' => {
                        let v = value[1..].parse::<u8>().map_err(|_| {
                            Error::custom(format!(
                                "The string must end in two digits, but got `{:?}`.",
                                &value[1..]
                            ))
                        })?;
                        Ok(SequenceElement::Gap(v))
                    }
                    'S' => {
                        let v = value[1..].parse::<u8>().map_err(|_| {
                            Error::custom(format!(
                                "The string must end in two digits, but got `{:?}`.",
                                &value[1..]
                            ))
                        })?;
                        Ok(SequenceElement::Size(v))
                    }
                    _ => Err(Error::custom(format!(
                        "The string must start with `G` or `S` but got `{}`.",
                        start
                    ))),
                }
            }
        }

        deserializer.deserialize_str(Helper)
    }
}

pub struct LabelledSequence<S = Atom> {
    pub true_domain: S,
    pub mapped_domain: S,
    pub sequence: Sequence,
}

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

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize)]
pub struct Query {
    pub source: QuerySource,
    pub qname: String,
    pub qtype: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub query_size: u32,
    pub response_size: u32,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize)]
pub enum QuerySource {
    Client,
    Forwarder,
    ForwarderLostQuery,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub struct MatchKey {
    pub qname: String,
    pub qtype: String,
    pub id: u16,
    pub port: u16,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct UnmatchedClientQuery {
    pub qname: String,
    pub qtype: String,
    pub start: DateTime<Utc>,
    pub size: u32,
}

#[cfg(test)]
mod test_edit_dist {
    use super::{
        Sequence,
        SequenceElement::{Gap, Size},
    };

    #[test]
    fn test_edit_distance_dist1() {
        let seq1 = Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)], "".into());

        // substitution
        let seq2 = Sequence(vec![Size(2), Gap(2), Size(1), Size(2), Size(1)], "".into());
        assert_eq!(6, seq1.distance(&seq2));
        let seq2b = Sequence(vec![Size(1), Gap(3), Size(1), Size(2), Size(1)], "".into());
        assert_eq!(3, seq1.distance(&seq2b));

        // swapping
        let seq3 = Sequence(vec![Size(1), Gap(2), Size(2), Size(1), Size(1)], "".into());
        assert_eq!(3, seq1.distance(&seq3));

        // deletion
        let seq4 = Sequence(vec![Size(1), Size(1), Size(2), Size(1)], "".into());
        assert_eq!(2, seq1.distance(&seq4));

        // insertion
        let seq5 = Sequence(
            vec![Size(1), Size(2), Gap(2), Size(1), Size(2), Size(1)],
            "".into(),
        );
        assert_eq!(12, seq1.distance(&seq5));
    }

    #[test]
    fn test_edit_distance_inserts() {
        let seq1 = Sequence(vec![], "".into());
        let seq2 = Sequence(vec![Size(1), Size(1)], "".into());

        let seq6 = Sequence(vec![Gap(3)], "".into());
        let seq7 = Sequence(vec![Gap(10)], "".into());
        println!("Smaller gap: {}", seq1.distance(&seq6));
        println!("Bigger gap: {}", seq1.distance(&seq7));
        assert!(
            seq1.distance(&seq6) < seq1.distance(&seq7),
            "Bigger Gaps have higher cost."
        );

        let seq6 = Sequence(vec![Size(1), Gap(3), Size(1)], "".into());
        let seq7 = Sequence(vec![Size(1), Gap(10), Size(1)], "".into());
        println!("Smaller gap: {}", seq2.distance(&seq6));
        println!("Bigger gap: {}", seq2.distance(&seq7));
        assert!(
            seq2.distance(&seq6) < seq2.distance(&seq7),
            "Bigger Gaps have higher cost."
        );
    }

    #[test]
    fn test_edit_distance_substitutions() {
        let seq1 = Sequence(vec![Size(1)], "".into());
        let seq2 = Sequence(vec![Gap(10)], "".into());

        let seqa = Sequence(vec![Gap(9)], "".into());
        let seqb = Sequence(vec![Gap(1)], "".into());
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

        let seq3 = Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)], "".into());
        let seq4 = Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)], "".into());
        assert_eq!(0, seq3.distance(&seq4));
    }
}
