#[cfg(feature = "read_pcap")]
mod bounded_buffer;
mod constants;
pub mod knn;
mod load_sequence;
#[cfg(feature = "read_pcap")]
pub mod pcap;
pub mod precision_sequence;
mod serialization;
mod utils;

use crate::{common_sequence_classifications::*, constants::*, load_sequence::Padding};
pub use crate::{
    precision_sequence::PrecisionSequence,
    utils::{
        load_all_dnstap_files_from_dir, load_all_dnstap_files_from_dir_with_config,
        load_all_files_with_extension_from_dir_with_config, Probability,
    },
};
use chrono::{self, DateTime, Duration, NaiveDateTime, Utc};
use failure::{self, Error, ResultExt};
use lazy_static::lazy_static;
pub use load_sequence::convert_to_sequence;
use misc_utils::{self, fs, path::PathExt, Min};
use serde::{
    self,
    de::{MapAccess, Visitor},
    ser::SerializeMap,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    collections::{BTreeMap, HashMap},
    fmt::{self, Debug},
    hash::Hash,
    mem,
    path::Path,
    sync::{Arc, RwLock},
    time::Instant,
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

    /// Load a [`Sequence`] from a file path.
    pub fn from_path(path: &Path) -> Result<Sequence, Error> {
        // Iterate over all file extensions, from last to first.
        for ext in path.extensions() {
            match ext.to_str() {
                Some("dnstap") => return load_sequence::dnstap_to_sequence(path),
                Some("json") => {
                    let seq_json = fs::read_to_string(path)
                        .with_context(|_| format!("Cannot read file `{}`", path.display()))?;
                    return Ok(serde_json::from_str(&seq_json)?);
                }
                #[cfg(feature = "read_pcap")]
                Some("pcap") => return crate::pcap::load_pcap_file(path, None),
                _ => {}
            }
        }
        // Fallback to the old behavior
        load_sequence::dnstap_to_sequence(path)
    }

    /// Load a [`Sequence`] from a file path. The file has to be a dnstap file.
    ///
    /// `config` allows to alter the loading according to [`LoadDnstapConfig`]
    pub fn from_path_with_config(path: &Path, config: LoadDnstapConfig) -> Result<Sequence, Error> {
        load_sequence::dnstap_to_sequence_with_config(path, config)
    }

    /// FIXME merge with convert_to_sequence
    pub fn from_sizes_and_times(
        path: String,
        sizes_and_times: &[(u16, Instant)],
    ) -> Result<Sequence, Error> {
        let base_gap_size = Duration::microseconds(1000);
        let mut last_time = None;
        let elements = sizes_and_times
            .iter()
            .flat_map(|&(size, time)| {
                let mut gap = None;
                if let Some(last_time) = last_time {
                    gap = load_sequence::gap_size(
                        Duration::from_std(time - last_time).unwrap(),
                        base_gap_size,
                    );
                }
                let size = Some(load_sequence::pad_size(
                    u32::from(size),
                    false,
                    Padding::Q128R468,
                ));
                last_time = Some(time);
                gap.into_iter().chain(size)
            })
            .collect();
        Ok(Sequence(elements, path))
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

    pub fn to_vector_encoding(&self) -> Vec<(u16, u16)> {
        self.0
            .iter()
            .cloned()
            .map(SequenceElement::to_vector_encoding)
            .collect()
    }

    /// Return the distance to the `other` [`Sequence`].
    pub fn distance(&self, other: &Self) -> usize {
        self.distance_with_limit::<()>(other, usize::max_value(), false, false)
            .0
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
    pub fn distance_with_limit<DCI>(
        &self,
        other: &Self,
        max_distance: usize,
        use_length_prefilter: bool,
        use_cr_mode: bool,
    ) -> (usize, DCI)
    where
        DCI: DistanceCostInfo,
    {
        let mut larger = self;
        let mut smaller = other;

        if larger.0.len() < smaller.0.len() {
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
            for &x in &larger.0[smaller.0.len()..] {
                cost = cost.saturating_add(x.insert_cost());
                cost_info = cost_info.insert(cost, x);
            }
            return (cost, cost_info);
        }

        const ABSOLUTE_LENGTH_DIFF: usize = 40;
        const RELATIVE_LENGTH_DIFF_FACTOR: usize = 5;
        let length_diff = larger.0.len() - smaller.0.len();
        if use_length_prefilter
            && length_diff > ABSOLUTE_LENGTH_DIFF
            && length_diff > (larger.0.len() / RELATIVE_LENGTH_DIFF_FACTOR)
        {
            let cost_info = DCI::default().abort();
            return (usize::max_value(), cost_info);
        }

        if smaller.0.is_empty() {
            let mut cost: usize = 0;
            let mut cost_info = DCI::default();
            for &x in &larger.0 {
                cost = cost.saturating_add(x.insert_cost());
                cost_info = cost_info.insert(cost, x);
            }
            return (cost, cost_info);
        }

        type RowType<DCI> = Vec<(usize, DCI)>;

        let mut prev_prev_row: RowType<DCI> = (0..smaller.0.len() + 1)
            .map(|_| (0, DCI::default()))
            .collect();
        let mut cost = 0;
        let mut previous_row: RowType<DCI> = Some((0, DCI::default()))
            .into_iter()
            .chain(smaller.0.iter().map(|&elem| {
                cost += elem.insert_cost();
                let cost_info = DCI::default().insert(cost, elem);
                (cost, cost_info)
            }))
            .collect();
        let mut current_row: RowType<DCI> = (0..smaller.0.len() + 1)
            .map(|_| (0, DCI::default()))
            .collect();
        debug_assert_eq!(
            previous_row.len(),
            current_row.len(),
            "Row length must be equal"
        );

        for (i, &elem1) in larger.0.iter().enumerate() {
            current_row.clear();
            let p = previous_row[0].0 + elem1.delete_cost();
            let p_info = previous_row[0].1.delete(p, elem1);
            current_row.push((p, p_info));
            let mut min_cost_current_row: Min<usize> = Default::default();

            for (j, &elem2) in smaller.0.iter().enumerate() {
                let insertions = previous_row[j + 1].0 + elem1.insert_cost();
                let insertions_info = previous_row[j + 1].1.insert(insertions, elem1);
                let deletions = current_row[j].0 + elem2.delete_cost();
                let deletions_info = current_row[j].1.delete(deletions, elem2);
                let substitutions = previous_row[j].0 + elem1.substitute_cost(elem2);
                let substitutions_info = previous_row[j].1.substitute(substitutions, elem1, elem2);
                let (swapping, swapping_info) = if i > 0
                    && j > 0
                    && larger.0[i] == smaller.0[j - 1]
                    && larger.0[i - 1] == smaller.0[j]
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

            // See whether we can abort early
            // `min_cost_current_row` keeps the minimal cost which we encountered this row
            // We also know, that is the cost ever becomes larger than `max_distance`, then the result of this function is uninteresting.
            // If we see that `min_cost_current_row > max_distance`, then we know that this function can never return a result smaller than `max_distance`,
            // because there is always a cost added to the value of the previous row.
            if min_cost_current_row.get_min_extreme() > max_distance {
                let cost_info = DCI::default().abort();
                return (usize::max_value(), cost_info);
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

            _ => None,
        }
    }

    pub fn to_json(&self) -> Result<String, Error> {
        Ok(serde_json::to_string(self)?)
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

    fn to_vector_encoding(self) -> (u16, u16) {
        use self::SequenceElement::*;
        match self {
            Size(s) => (s as u16, 0),
            Gap(g) => (0, g as u16),
        }
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

        impl<'de> Visitor<'de> for Helper {
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

pub trait DistanceCostInfo: Clone + Default {
    /// Indicates that the insert operation was the cheapest and the current cost is `cost`.
    #[must_use]
    fn insert(&self, cost: usize, elem1: SequenceElement) -> Self;
    /// Indicates that the delete operation was the cheapest and the current cost is `cost`.
    #[must_use]
    fn delete(&self, cost: usize, elem1: SequenceElement) -> Self;
    /// Indicates that the substitute operation was the cheapest and the current cost is `cost`.
    #[must_use]
    fn substitute(&self, cost: usize, elem1: SequenceElement, elem2: SequenceElement) -> Self;
    /// Indicates that the swap operation was the cheapest and the current cost is `cost`.
    #[must_use]
    fn swap(&self, cost: usize, elem1: SequenceElement, elem2: SequenceElement) -> Self;
    /// Indicates that the distance computation was aborted early.
    ///
    /// This occurs, if the current distance is already larger than any distance in the kNN set.
    #[must_use]
    fn abort(&self) -> Self;
}

impl DistanceCostInfo for () {
    fn insert(&self, _cost: usize, _elem1: SequenceElement) -> Self {}
    fn delete(&self, _cost: usize, _elem1: SequenceElement) -> Self {}
    fn substitute(&self, _cost: usize, _elem1: SequenceElement, _elem2: SequenceElement) -> Self {}
    fn swap(&self, _cost: usize, _elem1: SequenceElement, _elem2: SequenceElement) -> Self {}
    fn abort(&self) -> Self {}
}

#[derive(Debug, Clone, Default)]
pub struct CostTracker {
    pub insert_gap: usize,
    pub insert_size: usize,
    pub delete_gap: usize,
    pub delete_size: usize,
    pub substitute_gap_gap: usize,
    pub substitute_gap_size: usize,
    pub substitute_size_gap: usize,
    pub substitute_size_size: usize,
    pub swap_gap_gap: usize,
    pub swap_gap_size: usize,
    pub swap_size_gap: usize,
    pub swap_size_size: usize,
    pub is_abort: bool,
    pub from_gap_to_gap: Arc<BTreeMap<(u8, u8), usize>>,
    current_cost: usize,
}

impl CostTracker {
    pub fn as_btreemap(&self) -> BTreeMap<String, usize> {
        let mut res = BTreeMap::default();

        // Convert all the gap-to-gap counts
        for ((from, to), &count) in &*self.from_gap_to_gap {
            res.insert(format!("gap({})_to_gap({})", from, to), count);
        }

        res.insert("insert_gap".into(), self.insert_gap);
        res.insert("insert_size".into(), self.insert_size);
        res.insert("delete_gap".into(), self.delete_gap);
        res.insert("delete_size".into(), self.delete_size);
        res.insert("substitute_gap_gap".into(), self.substitute_gap_gap);
        res.insert("substitute_gap_size".into(), self.substitute_gap_size);
        res.insert("substitute_size_gap".into(), self.substitute_size_gap);
        res.insert("substitute_size_size".into(), self.substitute_size_size);
        res.insert("swap_gap_gap".into(), self.swap_gap_gap);
        res.insert("swap_gap_size".into(), self.swap_gap_size);
        res.insert("swap_size_gap".into(), self.swap_size_gap);
        res.insert("swap_size_size".into(), self.swap_size_size);
        res.insert("is_abort".into(), self.is_abort as usize);
        res
    }

    fn update<F>(&self, cost: usize, f: F) -> Self
    where
        F: Fn(&mut Self, usize),
    {
        let mut res = self.clone();
        let diff = cost - self.current_cost;
        res.current_cost = cost;
        f(&mut res, diff);
        res
    }
}

impl DistanceCostInfo for CostTracker {
    fn insert(&self, cost: usize, elem1: SequenceElement) -> Self {
        self.update(cost, |x, diff| match elem1 {
            SequenceElement::Gap(_) => x.insert_gap += diff,
            SequenceElement::Size(_) => x.insert_size += diff,
        })
    }
    fn delete(&self, cost: usize, elem1: SequenceElement) -> Self {
        self.update(cost, |x, diff| match elem1 {
            SequenceElement::Gap(_) => x.delete_gap += diff,
            SequenceElement::Size(_) => x.delete_size += diff,
        })
    }
    fn substitute(&self, cost: usize, elem1: SequenceElement, elem2: SequenceElement) -> Self {
        let mut this = self.clone();
        if self.current_cost != cost {
            if let (SequenceElement::Gap(g1), SequenceElement::Gap(g2)) = (elem1, elem2) {
                let bmap = Arc::make_mut(&mut this.from_gap_to_gap);
                let min = g1.min(g2);
                let max = g1.max(g2);
                *bmap.entry((min, max)).or_insert(0) += 1;
            }
        }
        this.update(cost, |x, diff| match (elem1, elem2) {
            (SequenceElement::Gap(_), SequenceElement::Gap(_)) => x.substitute_gap_gap += diff,
            (SequenceElement::Gap(_), SequenceElement::Size(_)) => x.substitute_gap_size += diff,
            (SequenceElement::Size(_), SequenceElement::Gap(_)) => x.substitute_size_gap += diff,
            (SequenceElement::Size(_), SequenceElement::Size(_)) => x.substitute_size_size += diff,
        })
    }
    fn swap(&self, cost: usize, elem1: SequenceElement, elem2: SequenceElement) -> Self {
        self.update(cost, |x, diff| match (elem1, elem2) {
            (SequenceElement::Gap(_), SequenceElement::Gap(_)) => x.swap_gap_gap += diff,
            (SequenceElement::Gap(_), SequenceElement::Size(_)) => x.swap_gap_size += diff,
            (SequenceElement::Size(_), SequenceElement::Gap(_)) => x.swap_size_gap += diff,
            (SequenceElement::Size(_), SequenceElement::Size(_)) => x.swap_size_size += diff,
        })
    }
    fn abort(&self) -> Self {
        let mut res = self.clone();
        res.is_abort = true;
        res
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
