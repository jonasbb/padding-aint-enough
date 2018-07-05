use rayon::prelude::*;
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    collections::HashMap,
    mem,
};
use take_smallest;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
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
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
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

pub fn knn(
    trainings_data: &[(String, Vec<Sequence>)],
    validation_data: &[Sequence],
    k: u8,
) -> Vec<String> {
    assert!(k > 0, "kNN needs a k with k > 0");

    validation_data
        .into_par_iter()
        .map(|vsample| {
            let distances = take_smallest(
                trainings_data
                .into_iter()
                // iterate over all elements of the trainings data
                .flat_map(|(label, tsample)| tsample.iter().map(move |s| ClassifierData {label, distance:vsample.distance(s)})),
                // collect the k smallest distances
                k as usize,
            );

            // k == 1 is easy, just take the one with smallest distance
            if k == 1 {
                if distances.len() >= 1 {
                    return distances[0].label.to_string();
                } else {
                    panic!("Not enough trainings data");
                }
            }

            let mut most_common_label: HashMap<String, usize> = HashMap::new();
            // let mut distance = 0;
            for class in distances {
                    *most_common_label
                    .entry(class.label.to_string())
                        .or_insert(0) += 1;
            }
            // // additionally to the first k entries also collect all entries with equal cost/distance than the highest one so far
            // while let Some(class) = distances.pop() {
            //     if class.0.distance <= distance {
            //         *most_common_label
            //             .entry(class.0.label.to_string())
            //             .or_insert(0) += 1;
            //     } else {
            //         // the entries are sorted by distance in increasing order
            //         // so as soon as the first one doesn't match anymore, the others
            //         // won't match either
            //         break;
            //     }
            // }
            let mut labels = most_common_label
                .iter()
                .fold(
                    (0, Vec::with_capacity(5)),
                    |(mut count, mut labels), (other_label, &other_count)| {
                        if other_count > count {
                            labels.clear();
                            labels.push(&**other_label);
                            count = other_count;
                        } else if other_count == count {
                            labels.push(&**other_label);
                        }
                        (count, labels)
                    },
                )
                .1;
            labels.sort();
            labels.join(" - ")
        })
        .collect()
}

#[cfg_attr(feature = "cargo-clippy", allow(type_complexity))]
pub fn split_training_test_data(
    data: &[(String, Vec<Sequence>)],
    fold: u8,
) -> (Vec<(String, Vec<Sequence>)>, Vec<(String, Sequence)>) {
    debug!("Start splitting trainings and test data");
    let mut training = Vec::with_capacity(data.len());
    let mut test = Vec::with_capacity(data.len());

    for (label, elements) in data {
        if elements.is_empty() {
            error!("{} has no data", label);
        }

        let mut trainings = elements.clone();
        let element = trainings.remove(fold as usize % elements.len());

        training.push((label.to_string(), trainings));
        // only take each test element once, if it belongs to exactly that fold
        if (fold as usize) < elements.len() {
            test.push((label.to_string(), element));
        }
    }

    debug!("Finished splitting trainings and test data");
    (training, test)
}

#[derive(Debug)]
struct ClassifierData<'a> {
    label: &'a str,
    distance: usize,
}

impl<'a> PartialEq for ClassifierData<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl<'a> Eq for ClassifierData<'a> {}

impl<'a> PartialOrd for ClassifierData<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl<'a> Ord for ClassifierData<'a> {
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

    assert_eq!(vec!["B"], knn(&*trainings_data, &*validation_data, 1));
    assert_eq!(vec!["B"], knn(&*trainings_data, &*validation_data, 2));
    assert_eq!(vec!["B"], knn(&*trainings_data, &*validation_data, 3));
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

    assert_eq!(vec!["B"], knn(&*trainings_data, &*validation_data, 1));
    assert_eq!(vec!["A - B"], knn(&*trainings_data, &*validation_data, 2));
    assert_eq!(vec!["A - B"], knn(&*trainings_data, &*validation_data, 3));
}
