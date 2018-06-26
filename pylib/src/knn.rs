use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd, Reverse};
use std::collections::{BinaryHeap, HashMap};
use Sequence;

pub fn knn(
    trainings_data: &[(String, Vec<Sequence>)],
    validation_data: &[Sequence],
    k: u8,
) -> Vec<String> {
    assert!(k > 0, "kNN needs a k with k > 0");

    validation_data
        .into_par_iter()
        .map(|vsample| {
            let mut distances: BinaryHeap<_> = trainings_data
                .into_par_iter()
                // iterate over all elements of the trainings data
                .flat_map(|(label, tsample)| tsample.par_iter().map(move |s| (label, s)))
                // calculate distance for each
                .map(|(label, s)| Reverse(ClassifierData {label, distance:vsample.distance(s)}))
                .collect();

            // k == 1 is easy, just take the one with smallest distance
            if k == 1 {
                if let Some(x) = distances.pop() {
                    return x.0.label.to_string();
                } else {
                    panic!("Not enough trainings data");
                }
            }

            let mut most_common_label: HashMap<String, usize> = HashMap::new();
            // let mut distance = 0;
            for _ in 0..k {
                if let Some(class) = distances.pop() {
                    *most_common_label
                        .entry(class.0.label.to_string())
                        .or_insert(0) += 1;
                    // distance = class.0.distance;
                }
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

        let mut elements = elements.clone();
        let element = elements.remove(fold as usize % elements.len());

        training.push((label.to_string(), elements));
        test.push((label.to_string(), element));
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
    use SequenceElement::*;
    let trainings_data = vec![
        (
            "A".into(),
            vec![Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)])],
        ),
        (
            "B".into(),
            vec![Sequence(vec![Size(1)]), Sequence(vec![Size(2)])],
        ),
    ];
    let validation_data = vec![Sequence::new(vec![Size(1)])];

    assert_eq!(vec!["B"], knn(&*trainings_data, &*validation_data, 1));
    assert_eq!(vec!["B"], knn(&*trainings_data, &*validation_data, 2));
    assert_eq!(vec!["B"], knn(&*trainings_data, &*validation_data, 3));
}

#[test]
fn test_knn_tie() {
    use SequenceElement::*;
    let trainings_data = vec![
        (
            "A".into(),
            vec![Sequence(vec![Size(1), Gap(2), Size(1), Size(2), Size(1)])],
        ),
        ("B".into(), vec![Sequence(vec![Size(1)])]),
    ];
    let validation_data = vec![Sequence::new(vec![Size(1)])];

    assert_eq!(vec!["B"], knn(&*trainings_data, &*validation_data, 1));
    assert_eq!(vec!["A - B"], knn(&*trainings_data, &*validation_data, 2));
    assert_eq!(vec!["A - B"], knn(&*trainings_data, &*validation_data, 3));
}
