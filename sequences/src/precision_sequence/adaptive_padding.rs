use crate::utils::Probability;
use chrono::{Duration, NaiveDateTime};
use log::debug;
use once_cell::sync::Lazy;
use rand::distributions::{Distribution, Uniform, WeightedError, WeightedIndex};
use rand_xorshift::XorShiftRng;

static DURATION_MAX: Lazy<Duration> = Lazy::new(|| Duration::seconds(3600 * 24 * 365));
static DURATION_ONE_MS: Lazy<Duration> = Lazy::new(|| Duration::milliseconds(1));
static DISTRIBUTION_BASE_VALUE: Lazy<f64> = Lazy::new(|| 2f64.sqrt());
static DISTRIBUTION: Lazy<Vec<(Duration, u16)>> = Lazy::new(|| {
    [
        (0, 0),
        (1, 0),
        (2, 0),
        (3, 0),
        (4, 0),
        (5, 0),
        (6, 0),
        (7, 0),
        (8, 0),
        (9, 0),
        (10, 0),
        (11, 11),
        (12, 48),
        (13, 41),
        (14, 28),
        (15, 15),
        (16, 15),
        (17, 14),
        (18, 12),
        (19, 11),
        (20, 12),
        (21, 15),
        (22, 20),
        (23, 23),
        (24, 24),
        (25, 36),
        (26, 24),
        (27, 22),
        (28, 22),
        (29, 116),
        (30, 20),
        (31, 23),
        (32, 101),
        (33, 104),
        (34, 29),
        (35, 41),
        (36, 39),
        (37, 43),
        (38, 31),
        (39, 25),
        (40, 16),
        (41, 10),
        (42, 6),
        (43, 2),
        (44, 1),
    ]
    .iter()
    .map(|&(gap, count)| {
        (
            Duration::microseconds(DISTRIBUTION_BASE_VALUE.powi(gap as i32) as i64),
            count,
        )
    })
    .collect()
});

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
enum State {
    Idle,
    Burst,
    Gap,
}

#[derive(Debug)]
pub struct AdaptivePadding {
    rng: XorShiftRng,
    eipi: Duration,
    last_created_item: NaiveDateTime,
    pub deadline: NaiveDateTime,
    /// Relevant for Gap mode
    intra_burst_gaps: Vec<(Duration, u16)>,
    /// Relevant for Burst mode
    inter_burst_gaps: Vec<(Duration, u16)>,
    state: State,
    /// Median length of burst generated
    median_burst_length: u32,
    /// Probability of creating a fake burst
    probability_fake_burst: Probability,
}

impl AdaptivePadding {
    pub fn new(
        rng: XorShiftRng,
        now: NaiveDateTime,
        median_burst_length: u32,
        probability_fake_burst: Probability,
    ) -> Self {
        assert!(median_burst_length >= 2);
        let mut res = Self {
            rng,
            eipi: *DURATION_MAX,
            last_created_item: now,
            deadline: now + *DURATION_MAX,
            intra_burst_gaps: Vec::default(),
            inter_burst_gaps: Vec::default(),
            state: State::Idle,
            median_burst_length,
            probability_fake_burst,
        };
        res.refill_inter_distribution();
        res.refill_intra_distribution();
        res
    }

    /// Sample a token from one of the distributions
    ///
    /// The correct distribution is determined using `self.state`.
    ///
    /// # Panics
    ///
    /// The function panics, if `self.state == `[`State::Idle`], as there is no distribution for idle.
    fn sample_token(&mut self, now: NaiveDateTime) -> Duration {
        fn get_dist(s: &mut AdaptivePadding) -> &mut Vec<(Duration, u16)> {
            match s.state {
                State::Burst => &mut s.inter_burst_gaps,
                State::Gap => &mut s.intra_burst_gaps,
                State::Idle => {
                    panic!(
                        "Cannot put_back_token while in state idle, as there is no token sampled."
                    );
                }
            }
        }

        // Build a distribution based on the counts in self.distribution
        let dist = match WeightedIndex::new(get_dist(self).iter().map(|item| item.1)) {
            Ok(dist) => dist,
            Err(WeightedError::NoItem) | Err(WeightedError::AllWeightsZero) => {
                self.refill_current_distribution();
                WeightedIndex::new(get_dist(self).iter().map(|item| item.1)).unwrap()
            }
            Err(WeightedError::InvalidWeight) => {
                panic!("Negative weights are impossible due to the type being u16")
            }
            Err(WeightedError::TooMany) => panic!("We never have more than `u32::MAX` buckets"),
        };
        // Get the index of the value
        let idx = dist.sample(&mut self.rng);
        // Retrieve the matching element from the distribution
        let &mut (duration, ref mut count) = &mut get_dist(self)[idx];
        *count -= 1;

        // Now that we have a base duration, we need to pick a duration uniformly between this bucket and the next bucket
        if duration == *DURATION_MAX {
            debug!("Sampled infinity token");
            match self.state {
                State::Idle => unreachable!("We do not sample tokens in this state"),
                State::Burst => {
                    debug!("Infinity Token: Fallback to Idle");
                    // Make sure to disable the timeout
                    self.set_deadline(now, *DURATION_MAX);
                    self.state = State::Idle;
                    return duration;
                }
                State::Gap => {
                    debug!("Infinity Token: Fallback to Burst");
                    self.state = State::Burst;
                    return self.sample_token(now);
                }
            };
            // This is unreachable
        }
        let duration = duration.to_std().unwrap();
        let uniform = Uniform::new(duration, duration.mul_f64(*DISTRIBUTION_BASE_VALUE));
        let duration = uniform.sample(&mut self.rng);

        debug!("Sampled {:?} token", duration);
        Duration::from_std(duration).unwrap()
    }

    /// Refill the distribution needed for Burst mode
    fn refill_inter_distribution(&mut self) {
        if self.inter_burst_gaps.is_empty() {
            // Fill in the normal distribution
            self.inter_burst_gaps.extend(
                DISTRIBUTION
                    .iter()
                    .filter(|(gap, _)| *gap >= *DURATION_ONE_MS)
                    .cloned(),
            );
            self.inter_burst_gaps.push((*DURATION_MAX, 0));
            // Maybe safe a bit of space
            self.inter_burst_gaps.shrink_to_fit();
        } else {
            self.inter_burst_gaps
                .iter_mut()
                .zip(
                    DISTRIBUTION
                        .iter()
                        .filter(|(gap, _)| *gap >= *DURATION_ONE_MS)
                        .cloned(),
                )
                .for_each(|((_, old_count), (_, new_count))| *old_count += new_count);
        }

        // Fix the infinity bins
        // INTER
        // kn = Pn / (1 − Pn) * K
        // Pn = 1 - propability_fake_burst
        // Pn propability to choose bucket n
        // K: tokens for all other buckets
        let sum_tokens: u32 = self
            .inter_burst_gaps
            .iter()
            .filter(|(gap, _)| *gap != *DURATION_MAX)
            .map(|(_, count)| u32::from(*count))
            .sum();
        let kn = ((1. - self.probability_fake_burst.to_float())
            / self.probability_fake_burst.to_float()
            * sum_tokens as f32)
            .round() as u16;
        let len = self.inter_burst_gaps.len();
        self.inter_burst_gaps[len - 1].1 = kn;
    }

    /// Refill the distribution needed for Gap mode
    fn refill_intra_distribution(&mut self) {
        if self.intra_burst_gaps.is_empty() {
            // Fill in the normal distribution
            self.intra_burst_gaps.extend(
                DISTRIBUTION
                    .iter()
                    .filter(|(gap, _)| *gap < *DURATION_ONE_MS)
                    .cloned(),
            );
            self.intra_burst_gaps.push((*DURATION_MAX, 0));
            // Maybe safe a bit of space
            self.intra_burst_gaps.shrink_to_fit();
        } else {
            self.intra_burst_gaps
                .iter_mut()
                .zip(
                    DISTRIBUTION
                        .iter()
                        .filter(|(gap, _)| *gap < *DURATION_ONE_MS)
                        .cloned(),
                )
                .for_each(|((_, old_count), (_, new_count))| *old_count += new_count);
        }

        // Fix the infinity bins
        // INTRA
        // kn = (K + µL + 1) / (µL - 1)
        // K: tokens for all other buckets
        // µL: Median burst length
        let sum_tokens: u32 = self
            .intra_burst_gaps
            .iter()
            .filter(|(gap, _)| *gap != *DURATION_MAX)
            .map(|(_, count)| u32::from(*count))
            .sum();
        let kn = (f64::from(sum_tokens + self.median_burst_length + 1)
            / f64::from(self.median_burst_length - 1))
        .round() as u16;
        let len = self.intra_burst_gaps.len();
        self.intra_burst_gaps[len - 1].1 = kn;
    }

    /// Increase the token count for the token bucket matching `duration`
    ///
    /// # Panics
    ///
    /// The function panics, if `self.state == `[`State::Idle`], as there is no distribution for idle.
    fn put_back_token(&mut self, duration: Duration) {
        let dist = match self.state {
            State::Burst => &mut self.inter_burst_gaps,
            State::Gap => &mut self.intra_burst_gaps,
            State::Idle => {
                panic!("Cannot put_back_token while in state idle, as there is no token sampled.");
            }
        };
        // Put token back into bucket
        if let Some((_gap, count)) = dist
            .iter_mut()
            .find(|(gap, _count)| (*gap + *gap) > duration)
        {
            *count += 1;
        }
    }

    /// Refill the distribtion for the current state
    ///
    /// # Panics
    ///
    /// The function panics, if `self.state == `[`State::Idle`], as there is no distribution for idle.
    fn refill_current_distribution(&mut self) {
        match self.state {
            State::Burst => self.refill_inter_distribution(),
            State::Gap => self.refill_intra_distribution(),
            State::Idle => {
                panic!("Cannot refill since there is no associated distribution");
            }
        }
    }

    /// Remove a token from the current distribution with the bucket matching `duration`
    ///
    /// # Panics
    ///
    /// The function panics, if `self.state == `[`State::Idle`], as there is no distribution for idle.
    fn remove_token(&mut self, duration: Duration) {
        fn get_dist(s: &mut AdaptivePadding) -> &mut Vec<(Duration, u16)> {
            match s.state {
                State::Burst => &mut s.inter_burst_gaps,
                State::Gap => &mut s.intra_burst_gaps,
                State::Idle => {
                    panic!(
                        "Cannot put_back_token while in state idle, as there is no token sampled."
                    );
                }
            }
        }
        // Find next bucket larger with count larger zero and remove token
        if let Some((_gap, count)) = get_dist(self)
            .iter_mut()
            .find(|(gap, count)| *gap >= duration && *count > 0)
        {
            *count -= 1;
        } else {
            self.refill_current_distribution();
            if let Some((_duration, count)) = get_dist(self)
                .iter_mut()
                .find(|(gap, count)| *gap >= duration && *count > 0)
            {
                *count -= 1;
            }
        }
    }

    /// Callback if the stream has payload to transmit
    pub fn handle_application_payload(&mut self, now: NaiveDateTime) {
        if self.state != State::Idle {
            self.put_back_token(self.eipi);
            // Calculate real duration
            let dur = now - self.last_created_item;
            debug!("Real duration is {:?}", dur);
            self.remove_token(dur);
        }
        self.state = State::Burst;
        let duration = self.sample_token(now);
        self.set_deadline(now, duration);
        self.last_created_item = now;
    }

    /// Set the new deadline to `now + duration`
    fn set_deadline(&mut self, now: NaiveDateTime, duration: Duration) {
        self.eipi = duration;
        self.deadline = now + duration;

        debug!(
            "New Deadline {:?}, Duration {:?}, State {:?}",
            self.deadline, duration, self.state
        );
    }

    /// Callback if a timeout occured
    pub fn handle_timeout(&mut self, now: NaiveDateTime) {
        self.last_created_item = now;
        match self.state {
            State::Idle => unreachable!("We never choose a timeout in idle state"),
            State::Burst => {
                self.state = State::Gap;
                // Sample a new timeout fitting for the new state
                self.handle_timeout(now);
            }
            State::Gap => {
                let duration = self.sample_token(now);
                self.set_deadline(now, duration);
            }
        }
    }

    /// Returns `true` if AP has terminated, i.e., the state is `Idle`
    pub fn has_terminated(&self) -> bool {
        self.state == State::Idle
    }
}
