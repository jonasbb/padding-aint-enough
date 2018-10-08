#![cfg_attr(feature = "cargo-clippy", allow(renamed_and_removed_lints))]

extern crate chrono;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate misc_utils;
extern crate rayon;
#[macro_use]
extern crate serde;
extern crate string_cache;

pub mod knn;
mod utils;

use chrono::{DateTime, Utc};
use common_sequence_classifications::*;
use misc_utils::{Max, Min};
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    collections::HashMap,
    fmt::{self, Debug, Display},
    mem,
    path::Path,
    sync::{Arc, RwLock},
};
use string_cache::DefaultAtom as Atom;

lazy_static! {
    static ref LOADING_FAILED: RwLock<Arc<HashMap<Atom, &'static str>>> = RwLock::default();
}

#[cfg_attr(feature = "cargo-clippy", allow(implicit_hasher))]
pub fn replace_loading_failed(new_data: HashMap<Atom, &'static str>) {
    *LOADING_FAILED
        .write()
        .expect("Writing to LOADING_FAILED should always work") = Arc::new(new_data);
}

pub mod common_sequence_classifications {
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sequence(Vec<SequenceElement>, String);

impl Sequence {
    pub fn new(sequence: Vec<SequenceElement>, identifier: String) -> Sequence {
        Sequence(sequence, identifier)
    }

    pub fn id(&self) -> &str {
        &*self.1
    }
}

impl Sequence {
    pub fn complexity(&self) -> usize {
        self.0
            .iter()
            .filter_map(|x| match x {
                SequenceElement::Size(n) => Some(*n as usize),
                _ => None,
            })
            .sum()
    }

    pub fn distance(&self, other: &Self) -> usize {
        if self.0.len() < other.0.len() {
            return other.distance(self);
        }
        // other is always shorter or equal sized

        if other.0.is_empty() {
            let mut cost: usize = 0;
            for x in &self.0 {
                cost = cost.saturating_add(x.insert_cost());
            }
            return cost;
        }

        let mut prev_prev_row = vec![0usize; other.0.len() + 1];
        // let mut previous_row: Vec<usize> = (0..(other.0.len() + 1)).into_iter().collect();
        let mut cost = 0;
        let mut previous_row: Vec<usize> = Some(0)
            .into_iter()
            .chain(other.0.iter().cloned().map(|elem| elem.insert_cost()))
            .map(|c| {
                cost += c;
                cost
            })
            .collect();
        let mut current_row = vec![0usize; other.0.len() + 1];
        debug_assert_eq!(
            previous_row.len(),
            current_row.len(),
            "Row length must be equal"
        );

        for (i, elem1) in self.0.iter().enumerate() {
            current_row.clear();
            // TODO give different costs for different elements
            current_row.push(previous_row[0] + elem1.delete_cost());

            for (j, &elem2) in other.0.iter().enumerate() {
                let insertions = previous_row[j + 1] + elem1.insert_cost();
                let deletions = current_row[j] + elem2.delete_cost();
                let substitutions = previous_row[j] + elem1.substitute_cost(elem2);
                let swapping =
                    if i > 0 && j > 0 && self.0[i] == other.0[j - 1] && self.0[i - 1] == other.0[j]
                    {
                        prev_prev_row[j - 1] + elem1.swap_cost(elem2)
                    } else {
                        // generate a large value but not so large, that an overflow might happen while performing some addition
                        usize::max_value() / 4
                    };
                let cost = insertions.min(deletions).min(substitutions).min(swapping);
                current_row.push(cost);
            }

            mem::swap(&mut prev_prev_row, &mut previous_row);
            mem::swap(&mut previous_row, &mut current_row);
        }

        *previous_row
            .last()
            .expect("The rows are never empty, thus there is a last.")
    }

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

        // Test if sequence only contains two responses of size 1 and then 2
        let packets: Vec<_> = self
            .as_elements()
            .iter()
            .filter(|elem| {
                if let SequenceElement::Size(_) = elem {
                    true
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        match &*packets {
            [] => {
                error!(
                    "Empty sequence for ID {}. Should never occur",
                    self.id()
                );
                None
            }
            [SequenceElement::Size(n)] => Some(match n {
                0 => unreachable!("Packets of size 0 may never occur."),
                1 => R004_SIZE1,
                2 => R004_SIZE2,
                3 => R004_SIZE3,
                4 => R004_SIZE4,
                5 => R004_SIZE5,
                6 => R004_SIZE6,
                _ => R004_UNKNOWN,
            }),
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
            _ => {
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
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
            Size(_) => 20,
            Gap(g) => g as usize * 5,
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
            (Size(_), Size(_)) => self.insert_cost().saturating_add(other.delete_cost()) / 3,
            (Gap(g1), Gap(g2)) => (g1.max(g2) - g1.min(g2)) as usize * 2,
            (a, b) => a.delete_cost().saturating_add(b.insert_cost()),
        }
    }

    fn swap_cost(self, other: Self) -> usize {
        if self == other {
            return 0;
        }

        20
    }
}

impl Debug for SequenceElement {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use SequenceElement::*;
        let (l, v) = match self {
            Size(v) => ("S", v),
            Gap(v) => ("G", v),
        };
        write!(f, "{}{:>2}", l, v)
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
        .map(|dists2| dists2.iter().sum::<usize>() / dists2.len())
        .collect();
    let median_distances: Vec<_> = dists
        .into_iter()
        .map(|mut dists2| {
            dists2.sort();
            dists2[dists2.len() / 2]
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
        assert_eq!(13, seq1.distance(&seq2));

        // swapping
        let seq3 = Sequence(vec![Size(1), Gap(2), Size(2), Size(1), Size(1)], "".into());
        assert_eq!(20, seq1.distance(&seq3));

        // deletion
        let seq4 = Sequence(vec![Size(1), Size(1), Size(2), Size(1)], "".into());
        assert_eq!(10, seq1.distance(&seq4));

        // insertion
        let seq5 = Sequence(
            vec![Size(1), Size(2), Gap(2), Size(1), Size(2), Size(1)],
            "".into(),
        );
        assert_eq!(20, seq1.distance(&seq5));
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
