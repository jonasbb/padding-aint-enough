use super::*;
use rayon::prelude::*;
use utils::take_smallest;

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
        })
        .collect()
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
pub(crate) struct ClassifierData<'a, S>
where
    S: 'a,
{
    label: &'a S,
    pub distance: usize,
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
        LabelledSequences {
            true_domain: "A",
            mapped_domain: "A",
            sequences: vec![Sequence(
                vec![Size(1), Gap(2), Size(1), Size(2), Size(1)],
                "".into(),
            )],
        },
        LabelledSequences {
            true_domain: "B",
            mapped_domain: "B",
            sequences: vec![
                Sequence(vec![Size(1)], "".into()),
                Sequence(vec![Size(2)], "".into()),
            ],
        },
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
        LabelledSequences {
            true_domain: "A",
            mapped_domain: "A",
            sequences: vec![Sequence(
                vec![Size(1), Gap(2), Size(1), Size(2), Size(1)],
                "".into(),
            )],
        },
        LabelledSequences {
            true_domain: "B",
            mapped_domain: "B",
            sequences: vec![Sequence(vec![Size(1)], "".into())],
        },
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
