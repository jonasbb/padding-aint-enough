mod adaptive_padding;

use self::adaptive_padding::AdaptivePadding;
use crate::{utils::Probability, AbstractQueryResponse, LoadSequenceConfig, Sequence};
#[cfg(feature = "read_pcap")]
use anyhow::{anyhow, Context as _};
use anyhow::{bail, Error};
use chrono::{Duration, NaiveDateTime};
use fnv::FnvHasher;
use misc_utils::{fs, path::PathExt};
use rand::{distributions::Open01, Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};
use serde_with::{formats::Flexible, serde_as, DurationSecondsWithFrac};
use std::{
    cmp::{max, min},
    fmt,
    hash::{Hash, Hasher},
    path::Path,
};

/// This type is similar to [`Sequence`] but provides higher precision timestamps and sizes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrecisionSequence(Vec<PrecisionSequenceEvent>, String);

impl PrecisionSequence {
    /// Create a new [`PrecisionSequence`] from it's building blocks
    ///
    /// This function panics, if the data sequence is empty.
    pub fn new<I, PSE>(data: I, identifier: String) -> Self
    where
        I: IntoIterator<Item = PSE>,
        PSE: Into<PrecisionSequenceEvent>,
    {
        let data: Vec<_> = data.into_iter().map(Into::into).collect();
        assert!(!data.is_empty());
        PrecisionSequence(data, identifier)
    }

    /// Load a [`PrecisionSequence`] from a file path.
    pub fn from_path(path: &Path) -> Result<Self, Error> {
        // Iterate over all file extensions, from last to first.
        for ext in path.extensions() {
            match ext.to_str() {
                Some("dnstap") => return crate::dnstap::build_precision_sequence(path),
                #[cfg(feature = "read_pcap")]
                Some("pcap") => {
                    return crate::pcap::build_precision_sequence(path, None, false).with_context(
                        || anyhow!("Could not build a sequence from the list of filtered records."),
                    );
                }
                Some("json") => {
                    let s = fs::read_to_string(path)?;
                    return Ok(serde_json::from_str(&s)?);
                }
                _ => {}
            }
        }
        bail!("No supported path extension could be found.")
    }

    /// Return the [`PrecisionSequence`]'s identifier. Normally, the file name.
    pub fn id(&self) -> &str {
        &*self.1
    }

    #[must_use]
    pub fn to_sequence(&self) -> Sequence {
        let seq = crate::load_sequence::convert_to_sequence(
            &self.0,
            self.1.clone(),
            LoadSequenceConfig::default(),
        );
        seq.expect("Building a sequence needs to work, as we already checked that there is at least one element.")
    }

    #[must_use]
    pub fn apply_constant_rate(&self, rate: Duration, timeout_prob: Probability) -> Self {
        // Setup a predictable RNG to randomly determine the ends
        let path = Path::new(&self.1);
        let filename = path.file_name().unwrap();
        let mut hasher = FnvHasher::with_key(0);
        filename.hash(&mut hasher);
        let mut rng = XorShiftRng::seed_from_u64(hasher.finish());

        // Internal state
        let mut next_schedule_time = self.0[0].time;
        let mut events = vec![];
        let mut need_more_dummy_elements = true;

        for mut event in self.0.iter().cloned() {
            while event.time > next_schedule_time && need_more_dummy_elements {
                events.push(PrecisionSequenceEvent {
                    time: next_schedule_time,
                    size: 128,
                    is_dummy_event: true,
                });
                next_schedule_time += rate;
                need_more_dummy_elements = rng.sample::<f32, _>(Open01) < timeout_prob.to_float();
            }
            // 1) either the time for the event came
            // OR 2) the previous sequence got terminated and a new one starts now
            // In case 1) next_schedule_time <= event.time
            // In case 2) next_schedule_time > event.time
            // Therefore, we need to take the maximum of both
            event.time = max(event.time, next_schedule_time);

            // each new event restarts the padding mode
            need_more_dummy_elements = true;
            // The next event should be `rate` after the current time
            next_schedule_time = event.time + rate;
            events.push(event);
        }

        // Add more dummy events at the end
        while need_more_dummy_elements {
            events.push(PrecisionSequenceEvent {
                time: next_schedule_time,
                size: 128,
                is_dummy_event: true,
            });
            next_schedule_time += rate;
            need_more_dummy_elements = rng.sample::<f32, _>(Open01) < timeout_prob.to_float();
        }

        Self(events, self.1.clone())
    }

    #[must_use]
    pub fn apply_adaptive_padding(
        &self,
        median_burst_length: u32,
        probability_fake_burst: Probability,
    ) -> Self {
        // Setup a predictable RNG to randomly determine the ends
        let path = Path::new(&self.1);
        let filename = path.file_name().unwrap();
        let mut hasher = FnvHasher::with_key(0);
        filename.hash(&mut hasher);
        let rng = XorShiftRng::seed_from_u64(hasher.finish());

        // Internal state
        let mut events = vec![];
        // Tracks the current simulated time
        let mut now;
        let mut ap = AdaptivePadding::new(
            rng,
            self.0[0].time,
            median_burst_length,
            probability_fake_burst,
        );

        for event in self.0.iter().cloned() {
            // This loop handles all timeouts which happen BEFORE the current packet can be send
            // Each timeout generates a dummy packet
            while event.time > ap.deadline {
                now = ap.deadline;
                events.push(PrecisionSequenceEvent {
                    time: now,
                    size: 128,
                    is_dummy_event: true,
                });
                ap.handle_timeout(now);
            }

            // Now that the time out this packet came send it
            now = event.time;
            ap.handle_application_payload(now);
            events.push(event);
        }

        // AP continues after the last real packet.
        // This processes all timeouts at the very end
        while !ap.has_terminated() {
            now = ap.deadline;
            events.push(PrecisionSequenceEvent {
                time: now,
                size: 128,
                is_dummy_event: true,
            });
            ap.handle_timeout(now);
        }

        assert!(self.0.len() <= events.len());
        Self(events, self.1.clone())
    }

    pub fn count_queries(&self) -> usize {
        self.0.len()
    }

    pub fn duration(&self) -> Duration {
        let mut iter = self.0.iter().filter(|x| !x.is_dummy_event);
        let first = (&mut iter).next();
        let last = iter.last();
        match (first, last) {
            (Some(_), None) => Duration::nanoseconds(0),
            (Some(first), Some(last)) => last.time - first.time,
            (None, _) => panic!("The PrecsionSequence must contain at least one non-dummy event."),
        }
    }

    /// Create a [`String`] describing the important parts of this [`PrecisionSequence`]
    pub fn info(&self) -> String {
        format!(
            r#"Query Count: {}
Total Duration: {}

{:?}"#,
            self.count_queries(),
            self.duration(),
            self.to_sequence(),
        )
    }

    pub fn overhead(&self, other: &Self) -> Overhead {
        let queries_baseline = min(
            self.count_queries() as isize,
            other.count_queries() as isize,
        );
        let queries = (self.count_queries() as isize - other.count_queries() as isize).abs();
        let time_baseline = min(self.duration(), other.duration());
        let mut time = self.duration() - other.duration();
        if time < Duration::zero() {
            time = Duration::zero() - time;
        }
        Overhead {
            queries_baseline,
            queries,
            time_baseline,
            time,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrecisionSequenceEvent {
    time: NaiveDateTime,
    size: u32,
    is_dummy_event: bool,
}

impl From<AbstractQueryResponse> for PrecisionSequenceEvent {
    fn from(aqr: AbstractQueryResponse) -> Self {
        Self {
            time: aqr.time,
            size: aqr.size,
            is_dummy_event: false,
        }
    }
}

impl From<&AbstractQueryResponse> for PrecisionSequenceEvent {
    fn from(aqr: &AbstractQueryResponse) -> Self {
        Self {
            time: aqr.time,
            size: aqr.size,
            is_dummy_event: false,
        }
    }
}

impl From<PrecisionSequenceEvent> for AbstractQueryResponse {
    fn from(pse: PrecisionSequenceEvent) -> Self {
        Self {
            time: pse.time,
            size: pse.size,
        }
    }
}

impl From<&PrecisionSequenceEvent> for AbstractQueryResponse {
    fn from(pse: &PrecisionSequenceEvent) -> Self {
        Self {
            time: pse.time,
            size: pse.size,
        }
    }
}

#[serde_as]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Overhead {
    pub queries_baseline: isize,
    pub queries: isize,
    #[serde_as(as = "DurationSecondsWithFrac<String, Flexible>")]
    pub time_baseline: Duration,
    #[serde_as(as = "DurationSecondsWithFrac<String, Flexible>")]
    pub time: Duration,
}

impl Overhead {
    pub fn new() -> Self {
        Self {
            queries_baseline: 0,
            queries: 0,
            time_baseline: Duration::zero(),
            time: Duration::zero(),
        }
    }
}

impl std::ops::Add for Overhead {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Self {
            queries_baseline: self.queries_baseline + other.queries_baseline,
            queries: self.queries + other.queries,
            time_baseline: self.time_baseline + other.time_baseline,
            time: self.time + other.time,
        }
    }
}

impl Default for Overhead {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Overhead {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Queries: ???? {}\nTime: ???? {}.{:>06}???s",
            self.queries,
            self.time.num_seconds(),
            self.time.num_microseconds().unwrap() % 1_000_000
        )
    }
}
