use minmax::{Max, Min};
use rayon::prelude::*;
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    collections::HashMap,
    fmt::{self, Debug, Display},
    mem,
};
use string_cache::DefaultAtom as Atom;
use take_smallest;

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
            }).sum()
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
            }).collect();
        let mut current_row = vec![0usize; other.0.len() + 1];
        assert_eq!(
            previous_row.len(),
            current_row.len(),
            "Row length must be equal"
        );

        for (i, elem1) in self.0.iter().enumerate() {
            current_row.clear();
            // TODO give different costs for different elements
            current_row.push(previous_row[0].saturating_add(elem1.delete_cost()));

            for (j, &elem2) in other.0.iter().enumerate() {
                let insertions = previous_row[j + 1].saturating_add(elem1.insert_cost());
                let deletions = current_row[j].saturating_add(elem2.delete_cost());
                let substitutions = previous_row[j].saturating_add(elem1.substitute_cost(elem2));
                let swapping =
                    if i > 0 && j > 0 && self.0[i] == other.0[j - 1] && self.0[i - 1] == other.0[j]
                    {
                        prev_prev_row[j - 1].saturating_add(elem1.swap_cost(elem2))
                    } else {
                        usize::max_value()
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
        match self {
            Size(0) => {
                // A size 0 packet should never occur
                error!("Sequence contains a Size(0) elements");
                usize::max_value()
            }
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

/// Find the k-nearest-neighbours in trainings_data for each element in validation_data
///
/// Returns a label for each entry in validation_data together with the minimal and maximal distance seen.
pub fn knn<S>(
    trainings_data: &[LabelledSequences<S>],
    validation_data: &[Sequence],
    k: u8,
) -> Vec<(String, Min<usize>, Max<usize>)>
where
    S: Clone + Display + Sync,
{
    assert!(k > 0, "kNN needs a k with k > 0");

    validation_data
        .into_par_iter()
        .map(|vsample| {
            let distances = take_smallest(
                trainings_data
                    .into_iter()
                    // iterate over all elements of the trainings data
                    .flat_map(|tlseq| {
                        tlseq.sequences.iter().map(move |s| ClassifierData {
                            label: &tlseq.mapped_domain,
                            distance: vsample.distance(s),
                        })
                    }),
                // collect the k smallest distances
                k as usize,
            );

            // k == 1 is easy, just take the one with smallest distance
            if k == 1 {
                if !distances.is_empty() {
                    return (
                        distances[0].label.to_string(),
                        Min::with_initial(distances[0].distance),
                        Max::with_initial(distances[0].distance),
                    );
                } else {
                    panic!("Not enough trainings data");
                }
            }

            let mut most_common_label: HashMap<String, (usize, Min<usize>, Max<usize>)> =
                HashMap::new();
            // let mut distance = 0;
            for class in distances {
                let entry = most_common_label.entry(class.label.to_string()).or_insert((
                    0,
                    Min::default(),
                    Max::default(),
                ));
                entry.0 += 1;
                entry.1.update(class.distance);
                entry.2.update(class.distance);
            }

            let (_count, min_dist, max_dist, mut labels) = most_common_label.iter().fold(
                (0, Min::default(), Max::default(), Vec::with_capacity(5)),
                |(mut count, mut min_dist, mut max_dist, mut labels),
                 (other_label, &(other_count, other_min_dist, other_max_dist))| {
                    if other_count > count {
                        labels.clear();
                        labels.push(&**other_label);
                        count = other_count;
                        min_dist = other_min_dist;
                        max_dist = other_max_dist;
                    } else if other_count == count {
                        labels.push(&**other_label);
                        min_dist.update(other_min_dist);
                        max_dist.update(other_max_dist);
                    }
                    (count, min_dist, max_dist, labels)
                },
            );
            labels.sort();
            (labels.join(" - "), min_dist, max_dist)
        }).collect()
}

#[cfg_attr(feature = "cargo-clippy", allow(type_complexity))]
pub fn split_training_test_data<S>(
    data: &[LabelledSequences<S>],
    fold: u8,
) -> (Vec<LabelledSequences<S>>, Vec<LabelledSequence<S>>)
where
    S: Clone + Display,
{
    debug!("Start splitting trainings and test data");
    let mut training: Vec<LabelledSequences<S>> = Vec::with_capacity(data.len());
    let mut test = Vec::with_capacity(data.len());

    for LabelledSequences {
        true_domain,
        mapped_domain,
        sequences,
    } in data
    {
        if sequences.is_empty() {
            error!("{} has no data", &true_domain);
        }

        let mut trainings = sequences.clone();
        let test_sequence = trainings.remove(fold as usize % sequences.len());

        training.push(LabelledSequences {
            true_domain: true_domain.clone(),
            mapped_domain: mapped_domain.clone(),
            sequences: trainings,
        });
        // only take each test element once, if it belongs to exactly that fold
        if (fold as usize) < sequences.len() {
            test.push(LabelledSequence {
                true_domain: true_domain.clone(),
                mapped_domain: mapped_domain.clone(),
                sequence: test_sequence,
            });
        }
    }

    debug!("Finished splitting trainings and test data");
    (training, test)
}

#[derive(Debug)]
struct ClassifierData<'a, S>
where
    S: 'a,
{
    label: &'a S,
    distance: usize,
}

impl<'a, S> PartialEq for ClassifierData<'a, S> {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl<'a, S> Eq for ClassifierData<'a, S> {}

impl<'a, S> PartialOrd for ClassifierData<'a, S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl<'a, S> Ord for ClassifierData<'a, S> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance.cmp(&other.distance)
    }
}

#[test]
fn test_knn() {
    use self::SequenceElement::*;
    let trainings_data = vec![
        (
            "A".into(),
            vec![Sequence(
                vec![Size(1), Gap(2), Size(1), Size(2), Size(1)],
                "".into(),
            )],
        ),
        (
            "B".into(),
            vec![
                Sequence(vec![Size(1)], "".into()),
                Sequence(vec![Size(2)], "".into()),
            ],
        ),
    ];
    let validation_data = vec![Sequence::new(vec![Size(1)], "".into())];

    assert_eq!(
        vec![("B".to_string(), Min::with_initial(0), Max::with_initial(0))],
        knn(&*trainings_data, &*validation_data, 1)
    );
    assert_eq!(
        vec![("B".to_string(), Min::with_initial(0), Max::with_initial(13))],
        knn(&*trainings_data, &*validation_data, 2)
    );
    assert_eq!(
        vec![("B".to_string(), Min::with_initial(0), Max::with_initial(13))],
        knn(&*trainings_data, &*validation_data, 3)
    );
}

#[test]
fn test_knn_tie() {
    use self::SequenceElement::*;
    let trainings_data = vec![
        (
            "A".into(),
            vec![Sequence(
                vec![Size(1), Gap(2), Size(1), Size(2), Size(1)],
                "".into(),
            )],
        ),
        ("B".into(), vec![Sequence(vec![Size(1)], "".into())]),
    ];
    let validation_data = vec![Sequence::new(vec![Size(1)], "".into())];

    assert_eq!(
        vec![("B".to_string(), Min::with_initial(0), Max::with_initial(0))],
        knn(&*trainings_data, &*validation_data, 1)
    );
    assert_eq!(
        vec![(
            "A - B".to_string(),
            Min::with_initial(0),
            Max::with_initial(70)
        )],
        knn(&*trainings_data, &*validation_data, 2)
    );
    assert_eq!(
        vec![(
            "A - B".to_string(),
            Min::with_initial(0),
            Max::with_initial(70)
        )],
        knn(&*trainings_data, &*validation_data, 3)
    );
}
