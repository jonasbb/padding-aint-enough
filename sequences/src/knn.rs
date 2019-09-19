use crate::{
    utils::{take_smallest, take_smallest_opt},
    LabelledSequence, LabelledSequences, Sequence,
};
use log::{debug, error};
use misc_utils::{Max, Min};
use ordered_float::NotNan;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    cmp::{Eq, Ord, Ordering, PartialEq, PartialOrd},
    fmt::{self, Display},
};

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum ClassificationResultQuality {
    /// There are no classification labels
    NoResult,
    /// None of the classification labels matches the real label
    Wrong,
    /// One of the classification labels matches the real label
    Contains,
    /// The plurality of classification labels match the real label
    ///
    /// If there are multiple pluralities, take the plurality with the minimal distance.
    /// If both pluralities have the same minimal distance, then this quality does not apply.
    ///
    /// This variant also implies `Contains`.
    PluralityThenMinDist,
    /// The plurality of classification labels match the real label
    ///
    /// This variant also implies `PluralityThenMinDist`.
    Plurality,
    /// The majority of classification labels match the real label
    ///
    /// This variant also implies `Plurality`.
    Majority,
    /// All classification labels match the real label
    ///
    /// This variant also implies `Majority`.
    Exact,
}

impl Display for ClassificationResultQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassificationResultQuality::NoResult => write!(f, "NoResult"),
            ClassificationResultQuality::Wrong => write!(f, "Wrong"),
            ClassificationResultQuality::Contains => write!(f, "Contains"),
            ClassificationResultQuality::PluralityThenMinDist => write!(f, "PluralityThenMinDist"),
            ClassificationResultQuality::Plurality => write!(f, "Plurality"),
            ClassificationResultQuality::Majority => write!(f, "Majority"),
            ClassificationResultQuality::Exact => write!(f, "Exact"),
        }
    }
}

impl ClassificationResultQuality {
    pub fn iter_variants<'a>(
    ) -> std::iter::Cloned<std::slice::Iter<'a, ClassificationResultQuality>> {
        use self::ClassificationResultQuality::*;
        [
            NoResult,
            Wrong,
            Contains,
            PluralityThenMinDist,
            Plurality,
            Majority,
            Exact,
        ]
        .iter()
        .cloned()
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct ClassificationResult {
    options: Vec<LabelOption>,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
struct LabelOption {
    name: String,
    count: u8,
    #[serde(with = "::serde_with::rust::display_fromstr")]
    distance_min: Min<usize>,
    #[serde(with = "::serde_with::rust::display_fromstr")]
    distance_max: Max<usize>,
    #[serde(
        serialize_with = "::serde_with::rust::display_fromstr::serialize",
        deserialize_with = "crate::serialization::deserialize_min_notnan"
    )]
    distance_min_norm: Min<NotNan<f64>>,
    #[serde(
        serialize_with = "::serde_with::rust::display_fromstr::serialize",
        deserialize_with = "crate::serialization::deserialize_max_notnan"
    )]
    distance_max_norm: Max<NotNan<f64>>,
}

impl ClassificationResult {
    fn from_classifier_data<S: AsRef<str>>(data: &[ClassifierData<'_, S>]) -> ClassificationResult {
        let mut result = ClassificationResult {
            options: Vec::with_capacity(9),
        };

        for entry in data {
            match result
                .options
                .iter_mut()
                .find(|opt| opt.is(entry.label.as_ref()))
            {
                None => {
                    let new_opt = LabelOption {
                        name: entry.label.as_ref().to_string(),
                        count: 1,
                        distance_min: Min::with_initial(entry.distance),
                        distance_max: Max::with_initial(entry.distance),
                        distance_min_norm: Min::with_initial(entry.distance_norm),
                        distance_max_norm: Max::with_initial(entry.distance_norm),
                    };
                    result.options.push(new_opt);
                }
                Some(opt) => opt.update(entry.distance),
            }
        }

        result
    }

    #[allow(clippy::block_in_if_condition_stmt)]
    pub fn determine_quality(&self, real_label: &str) -> ClassificationResultQuality {
        if self.options.is_empty() {
            return ClassificationResultQuality::NoResult;
        }

        if self.is(real_label) {
            return ClassificationResultQuality::Exact;
        }

        // try to find the label option matching to the real label
        let corr_option = match self.options.iter().find(|opt| opt.is(real_label)) {
            None => return ClassificationResultQuality::Wrong,
            Some(opt) => opt,
        };
        // Total number of label options
        let option_count = self.options.iter().map(|opt| opt.count).sum();

        if (corr_option.count * 2) > option_count {
            return ClassificationResultQuality::Majority;
        }

        // corr_option is the only Plurality if there is no other option with the same or higher count
        if !self
            .options
            .iter()
            // ignore the corr_option for the later tests
            .filter(|&opt| opt != corr_option)
            .any(|other| other.count >= corr_option.count)
        {
            return ClassificationResultQuality::Plurality;
        }

        // same as plurality check, but we also check the minimal distance
        if !self
            .options
            .iter()
            // ignore the corr_option for the later tests
            .filter(|&opt| opt != corr_option)
            .any(|other| {
                // if this is true, then corr_option is not a plurality
                other.count > corr_option.count
                // if there are multiple pluralities check if there is one with a smaller or equal minimal distance
                    || (other.count == corr_option.count
                        && other.distance_min <= corr_option.distance_min)
            })
        {
            return ClassificationResultQuality::PluralityThenMinDist;
        }

        // we already found an option with the correct label, so we know that Contains must be true
        ClassificationResultQuality::Contains
    }

    /// Returns `true` if `Label` is exactly `name` and there is no ambiguity
    fn is(&self, real_label: &str) -> bool {
        self.options.len() == 1 && self.options[0].is(real_label)
    }
}

impl LabelOption {
    /// Returns `true` if `LabelOption` is `name`
    fn is(&self, name: &str) -> bool {
        self.name == name
    }

    fn update(&mut self, distance: usize) {
        self.count += 1;
        self.distance_min.update(distance);
        self.distance_max.update(distance);
    }
}

/// Find the k-nearest-neighbours in `trainings_data` for each element in `validation_data`
///
/// Returns a label for each entry in `validation_data` together with the minimal and maximal distance seen.
/// This is grouped together in a [`ClassificationResult`].
pub fn knn<S>(
    trainings_data: &[LabelledSequences<S>],
    validation_data: &[Sequence],
    k: u8,
    use_cr_mode: bool,
) -> Vec<ClassificationResult>
where
    S: AsRef<str> + Clone + Display + Sync,
{
    assert!(k > 0, "kNN needs a k with k > 0");

    // TODO by precalculating a distance metric half of the distance() calls could be eliminated()

    validation_data
        .into_par_iter()
        .with_max_len(1)
        .map(|vsample| {
            let distances = take_smallest(
                trainings_data
                    .iter()
                    // iterate over all elements of the trainings data
                    .flat_map(|tlseq| {
                        tlseq.sequences.iter().map(move |s| {
                            move |max_dist: usize| {
                                let distance = vsample
                                    .distance_with_limit::<()>(s, max_dist, true, use_cr_mode)
                                    .0;
                                // Avoid divide by 0 cases, which can happen in the PerfectPadding scenario
                                let distance_norm = if distance == 0 {
                                    NotNan::new(0.).unwrap()
                                } else {
                                    NotNan::new(distance as f64 / s.len().max(vsample.len()) as f64)
                                        .unwrap_or_else(|err| {
                                            error!(
                                                "Failed to calculate normalized distance: {}",
                                                err
                                            );
                                            NotNan::new(999.).unwrap()
                                        })
                                };
                                ClassifierData {
                                    label: &tlseq.mapped_domain,
                                    distance,
                                    distance_norm,
                                }
                            }
                        })
                    }),
                // collect the k smallest distances
                k as usize,
            );
            ClassificationResult::from_classifier_data(&distances)
        })
        .collect()
}

pub fn knn_with_threshold<S>(
    trainings_data: &[LabelledSequences<S>],
    validation_data: &[Sequence],
    k: u8,
    distance_threshold: f32,
    use_cr_mode: bool,
) -> Vec<ClassificationResult>
where
    S: AsRef<str> + Clone + Display + Sync,
{
    assert!(k > 0, "kNN needs a k with k > 0");

    validation_data
        .into_par_iter()
        .with_max_len(1)
        .map(|vsample| {
            let distances = take_smallest_opt(
                trainings_data
                    .iter()
                    // iterate over all elements of the trainings data
                    .flat_map(|tlseq| {
                        tlseq.sequences.iter().map(move |s| {
                            let length_of_longer_sequence = vsample.len().max(s.len());
                            let abs_threshold = (length_of_longer_sequence as f32
                                * distance_threshold.ceil())
                                as usize;

                            move |max_dist: usize| {
                                // this is the distance as determined by the DNS sequence distance function
                                let real_distance = vsample
                                    .distance_with_limit::<()>(
                                        s,
                                        max_dist.min(abs_threshold),
                                        true,
                                        use_cr_mode,
                                    )
                                    .0;

                                if (real_distance as f32 / length_of_longer_sequence as f32)
                                    > distance_threshold
                                {
                                    // In case the distance reaches our threshold, we do not want any result
                                    None
                                } else {
                                    let distance = vsample
                                        .distance_with_limit::<()>(
                                            s,
                                            max_dist.min(abs_threshold),
                                            true,
                                            use_cr_mode,
                                        )
                                        .0;
                                    // Avoid divide by 0 cases, which can happen in the PerfectPadding scenario
                                    let distance_norm =
                                        if distance == 0 {
                                            NotNan::new(0.).unwrap()
                                        } else {
                                            NotNan::new(
                                                distance as f64 / s.len().max(vsample.len()) as f64,
                                            )
                                            .unwrap_or_else(|err| {
                                                error!(
                                                    "Failed to calculate normalized distance: {}",
                                                    err
                                                );
                                                NotNan::new(999.).unwrap()
                                            })
                                        };
                                    Some(ClassifierData {
                                        label: &tlseq.mapped_domain,
                                        distance,
                                        distance_norm,
                                    })
                                }
                            }
                        })
                    }),
                // collect the k smallest distances
                k as usize,
            );
            ClassificationResult::from_classifier_data(&distances)
        })
        .collect()
}

#[allow(clippy::type_complexity)]
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
        for idx in (0..sequences.len()).rev() {
            if idx % 10 == fold as usize {
                let test_sequence = trainings.remove(idx);
                // only take each test element once, if it belongs to exactly that fold
                if (fold as usize) < sequences.len() {
                    test.push(LabelledSequence {
                        true_domain: true_domain.clone(),
                        mapped_domain: mapped_domain.clone(),
                        sequence: test_sequence,
                    });
                }
            }
        }

        training.push(LabelledSequences {
            true_domain: true_domain.clone(),
            mapped_domain: mapped_domain.clone(),
            sequences: trainings,
        });
    }

    debug!("Finished splitting trainings and test data");
    (training, test)
}

#[derive(Debug)]
pub(crate) struct ClassifierData<'a, S: ?Sized> {
    label: &'a S,
    pub distance: usize,
    pub distance_norm: NotNan<f64>,
}

impl<'a, S: ?Sized> PartialEq for ClassifierData<'a, S> {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance && self.distance_norm == other.distance_norm
    }
}

impl<'a, S: ?Sized> Eq for ClassifierData<'a, S> {}

impl<'a, S: ?Sized> PartialOrd for ClassifierData<'a, S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl<'a, S: ?Sized> Ord for ClassifierData<'a, S> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .cmp(&other.distance)
            .then_with(|| self.distance_norm.cmp(&other.distance_norm))
    }
}
