mod adaptive_padding;

use self::adaptive_padding::AdaptivePadding;
use crate::{utils::Probability, AbstractQueryResponse, LoadSequenceConfig, Sequence};
use chrono::{Duration, NaiveDateTime};
use failure::{bail, Error};
use fnv::FnvHasher;
use misc_utils::{fs, path::PathExt};
use rand::{distributions::Open01, Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use serde::{Deserialize, Serialize};
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
                Some("dnstap") => return crate::load_sequence::dnstap_to_precision_sequence(path),
                #[cfg(feature = "read_pcap")]
                Some("pcap") => {
                    let records = crate::pcap::process_pcap(path, None, false, false)?;
                    return crate::pcap::build_precision_sequence(records, path.to_string_lossy())
                        .ok_or_else(|| {
                            failure::format_err!(
                                "Could not build a sequence from the list of filtered records."
                            )
                        });
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
        self.0.iter().count()
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

impl Into<AbstractQueryResponse> for PrecisionSequenceEvent {
    fn into(self) -> AbstractQueryResponse {
        AbstractQueryResponse {
            time: self.time,
            size: self.size,
        }
    }
}

impl Into<AbstractQueryResponse> for &PrecisionSequenceEvent {
    fn into(self) -> AbstractQueryResponse {
        AbstractQueryResponse {
            time: self.time,
            size: self.size,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct Overhead {
    pub queries_baseline: isize,
    pub queries: isize,
    #[serde(with = "serde_duration")]
    pub time_baseline: Duration,
    #[serde(with = "serde_duration")]
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
            "Queries: 𝚫 {}\nTime: 𝚫 {}.{:>06} s",
            self.queries,
            self.time.num_seconds(),
            self.time.num_microseconds().unwrap() % 1_000_000
        )
    }
}

mod serde_duration {
    use chrono::Duration;
    use serde::{
        de::{Deserializer, Error, Unexpected, Visitor},
        ser::Serializer,
    };

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper;
        impl<'de> Visitor<'de> for Helper {
            type Value = Duration;

            fn expecting(&self, formatter: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                formatter.write_str("Invalid duration. Must be an integer, float, or string with optional subsecond precision.")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                Ok(Duration::seconds(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                if value <= i64::max_value() as u64 {
                    Ok(Duration::seconds(value as i64))
                } else {
                    Err(Error::custom(format!(
                        "Invalid or out of range value '{}' for Duration",
                        value
                    )))
                }
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let seconds = value.trunc() as i64;
                let nsecs = (value.fract() * 1_000_000_000_f64).abs() as u32;
                Ok(Duration::seconds(seconds) + Duration::nanoseconds(i64::from(nsecs)))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let parts: Vec<_> = value.split('.').collect();

                match *parts.as_slice() {
                    [seconds] => {
                        if let Ok(seconds) = i64::from_str_radix(seconds, 10) {
                            Ok(Duration::seconds(seconds))
                        } else {
                            Err(Error::invalid_value(Unexpected::Str(value), &self))
                        }
                    }
                    [seconds, subseconds] => {
                        if let Ok(seconds) = i64::from_str_radix(seconds, 10) {
                            let subseclen = subseconds.chars().count() as u32;
                            if subseclen > 9 {
                                return Err(Error::custom(format!(
                                    "Duration only support nanosecond precision but '{}' has more than 9 digits.",
                                    value
                                )));
                            }

                            if let Ok(mut subseconds) = u32::from_str_radix(subseconds, 10) {
                                // convert subseconds to nanoseconds (10^-9), require 9 places for nanoseconds
                                subseconds *= 10u32.pow(9 - subseclen);
                                Ok(Duration::seconds(seconds)
                                    + Duration::nanoseconds(i64::from(subseconds)))
                            } else {
                                Err(Error::invalid_value(Unexpected::Str(value), &self))
                            }
                        } else {
                            Err(Error::invalid_value(Unexpected::Str(value), &self))
                        }
                    }

                    _ => Err(Error::invalid_value(Unexpected::Str(value), &self)),
                }
            }
        }

        deserializer.deserialize_any(Helper)
    }

    pub fn serialize<S>(d: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let sec = d.num_seconds();
        let nsec = (*d - Duration::seconds(sec)).num_nanoseconds().unwrap();
        let s = format!("{}.{:>09}", sec, nsec);
        serializer.serialize_str(&*s)
    }
}
