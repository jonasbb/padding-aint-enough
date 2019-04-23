use crate::{error::Error, Payload};
use futures::{future::Future, stream, Async, Poll, Stream};
use lazy_static::lazy_static;
use log::debug;
use rand::{
    distributions::{Distribution, Uniform, WeightedError, WeightedIndex},
    thread_rng,
};
use std::time::{Duration, Instant};
use tokio_timer::Delay;

const DURATION_MAX: Duration = Duration::from_secs(3600 * 24 * 365);
const DURATION_ONE_MS: Duration = Duration::from_millis(1);
const MEDIAN_BURST_LENGTH: u32 = 2;
const PROBABILITY_FAKE_BURST: f64 = 0.9;

lazy_static! {
    static ref DISTRIBUTION_BASE_VALUE: f64 = 2f64.sqrt();
    static ref DISTRIBUTION: Vec<(Duration, u16)> = [
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
    .map(|&(gap, count)| (
        Duration::from_micros(DISTRIBUTION_BASE_VALUE.powi(gap as i32) as u64),
        count
    ))
    .collect();
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
enum Event<T> {
    Timeout,
    Payload(T),
    PayloadEnd,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
enum State {
    Idle,
    Burst,
    Gap,
}

pub struct AdaptivePadding<T> {
    stream: Box<dyn Stream<Item = Event<T>, Error = Error> + Send + 'static>,
    eipi: Duration,
    deadline: Delay,
    /// Relevant for Gap mode
    intra_burst_gaps: Vec<(Duration, u16)>,
    /// Relevant for Burst mode
    inter_burst_gaps: Vec<(Duration, u16)>,
    last_created_item: Instant,
    state: State,
}

impl<T> AdaptivePadding<T>
where
    T: Send,
{
    pub fn new<S>(stream: S) -> Self
    where
        S: Stream<Item = T> + Send + 'static,
        S::Error: Into<Error>,
        T: 'static,
    {
        let stream = stream
            .map(Event::Payload)
            .map_err(Into::into)
            .chain(stream::once(Ok(Event::PayloadEnd)));
        let mut res = Self {
            stream: Box::new(stream),
            eipi: DURATION_MAX,
            deadline: Delay::new(Instant::now() + DURATION_MAX),
            intra_burst_gaps: Vec::default(),
            inter_burst_gaps: Vec::default(),
            last_created_item: Instant::now(),
            state: State::Idle,
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
    fn sample_token(&mut self) -> Duration {
        fn get_dist<T>(s: &mut AdaptivePadding<T>) -> &mut Vec<(Duration, u16)> {
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
            Err(WeightedError::NegativeWeight) => {
                panic!("Negative weights are impossible due to the type being u16")
            }
        };
        // Get the index of the value
        let idx = dist.sample(&mut thread_rng());
        // Retrieve the matching element from the distribution
        let &mut (duration, ref mut count) = &mut get_dist(self)[idx];
        *count -= 1;

        // Now that we have a base duration, we need to pick a duration uniformly between this bucket and the next bucket
        if duration == DURATION_MAX {
            debug!("Sampled infinity token");
            return duration;
        }
        let uniform = Uniform::new(duration, duration.mul_f64(*DISTRIBUTION_BASE_VALUE));
        let duration = uniform.sample(&mut rand::thread_rng());

        debug!("Sampled {:?} token", duration);
        duration
    }

    /// Refill the distribution needed for Burst mode
    fn refill_inter_distribution(&mut self) {
        if self.inter_burst_gaps.is_empty() {
            // Fill in the normal distribution
            self.inter_burst_gaps.extend(
                DISTRIBUTION
                    .iter()
                    .filter(|(gap, _)| *gap >= DURATION_ONE_MS)
                    .cloned(),
            );
            self.inter_burst_gaps.push((DURATION_MAX, 0));
            // Maybe safe a bit of space
            self.inter_burst_gaps.shrink_to_fit();
        } else {
            self.inter_burst_gaps
                .iter_mut()
                .zip(
                    DISTRIBUTION
                        .iter()
                        .filter(|(gap, _)| *gap >= DURATION_ONE_MS)
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
            .filter(|(gap, _)| *gap != DURATION_MAX)
            .map(|(_, count)| u32::from(*count))
            .sum();
        let kn = ((1. - PROBABILITY_FAKE_BURST) / PROBABILITY_FAKE_BURST * f64::from(sum_tokens))
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
                    .filter(|(gap, _)| *gap < DURATION_ONE_MS)
                    .cloned(),
            );
            self.intra_burst_gaps.push((DURATION_MAX, 0));
            // Maybe safe a bit of space
            self.intra_burst_gaps.shrink_to_fit();
        } else {
            self.intra_burst_gaps
                .iter_mut()
                .zip(
                    DISTRIBUTION
                        .iter()
                        .filter(|(gap, _)| *gap < DURATION_ONE_MS)
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
            .filter(|(gap, _)| *gap != DURATION_MAX)
            .map(|(_, count)| u32::from(*count))
            .sum();
        let kn = (f64::from(sum_tokens + MEDIAN_BURST_LENGTH + 1)
            / f64::from(MEDIAN_BURST_LENGTH - 1))
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
        if let Some((_gap, count)) = dist.iter_mut().find(|(gap, _count)| (2 * *gap) > duration) {
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
        fn get_dist<T>(s: &mut AdaptivePadding<T>) -> &mut Vec<(Duration, u16)> {
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
    fn handle_application_payload(&mut self) {
        if self.state != State::Idle {
            self.put_back_token(self.eipi);
            // Calculate real duration
            let dur = Instant::now() - self.last_created_item;
            debug!("Real duration is {:?}", dur);
            self.remove_token(dur);
        }
        self.state = State::Burst;
        let duration = self.sample_token();
        self.set_deadline(duration);
    }

    /// Set the new deadline to [`Instant::now`]` + duration`
    fn set_deadline(&mut self, duration: Duration) {
        self.eipi = duration;
        let now = Instant::now();
        let deadline = now + duration;
        self.deadline.reset(deadline);

        debug!(
            "New Deadline {:?}, Duration {:?}, State {:?}",
            deadline, duration, self.state
        );
    }

    /// Callback if a timeout occured
    fn handle_timeout(&mut self) {
        match self.state {
            State::Idle => unreachable!("We never choose a timeout in idle state"),
            State::Burst => {
                self.state = State::Gap;
                // Sample a new timeout fitting for the new state
                self.handle_timeout();
            }
            State::Gap => {
                let mut duration = self.sample_token();
                if duration == DURATION_MAX {
                    debug!("Infinity Token: Fallback to Burst");
                    self.state = State::Burst;
                    duration = self.sample_token();
                    if duration == DURATION_MAX {
                        debug!("Infinity Token: Fallback to Idle");
                        self.state = State::Idle;
                        // Make sure to disable the timeout
                        self.set_deadline(DURATION_MAX);
                        return;
                    }
                }
                self.set_deadline(duration);
            }
        }
    }
}

impl<T> Stream for AdaptivePadding<T>
where
    T: Send,
{
    type Item = Payload<T>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let delay_stream = (&mut self.deadline)
            .map(|_| Event::Timeout)
            .from_err()
            .into_stream();
        let mut stream = delay_stream.select(&mut self.stream);

        match stream.poll()? {
            Async::Ready(Some(event)) => {
                let res = match event {
                    Event::Timeout => {
                        debug!("Timeout received, State {:?}", self.state);
                        self.handle_timeout();
                        Some(Payload::Dummy)
                    }
                    Event::Payload(p) => {
                        debug!("Payload received");
                        self.handle_application_payload();
                        Some(Payload::Payload(p))
                    }
                    Event::PayloadEnd => {
                        debug!("PayloadEnd received");
                        None
                    }
                };

                if res.is_some() {
                    self.last_created_item = Instant::now();
                }

                Ok(Async::Ready(res))
            }
            // The timer instance is done, this should never happen
            Async::Ready(None) => panic!("Timer instance is done. This should never happen."),
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{future, stream};
    use std::time::Instant;
    use tokio_timer::throttle::Throttle;

    /// The minimum [`Duration`] which can be sampled for EIPI
    const MS_MIN: Duration = Duration::from_millis(8);
    /// [`Duration`] of exactly 1 ms
    const MS_1: Duration = Duration::from_millis(1);
    /// [`Duration`] of exactly 100 ms
    const MS_100: Duration = Duration::from_millis(100);

    #[test]
    fn test_adaptive_padding_ensure_fill_long_gaps() {
        let items = stream::iter_ok::<_, ()>(0..10);
        let throttle = Throttle::new(items, MS_100).map_err(|err| {
            if err.is_timer_error() {
                panic!("{}", err.into_timer_error().unwrap());
            } else {
                err.into_stream_error().unwrap()
            }
        });

        let cr = AdaptivePadding::new(throttle);
        let mut last = Instant::now();

        let fut = cr.map_err(|_err| ()).for_each(move |x| {
            let now = Instant::now();
            eprintln!("{:>5} µs: {:?}", (now - last).as_micros(), x);
            // Ensure that the adaptive padding produces items quicker than the throttle
            assert!(now - last < MS_100);
            last = now;
            future::ok(())
        });

        tokio::run(fut);
    }

    #[test]
    fn test_adaptive_padding_reset_gap_after_payload() {
        // Ensure that a new gap is sampled after each payload entry,
        // by checking that the time between payload and the first dummy is at least the minimum time (modulo timer resolution)
        let items = stream::iter_ok::<_, ()>(0..10);
        let throttle = Throttle::new(items, MS_100).map_err(|err| {
            if err.is_timer_error() {
                panic!("{}", err.into_timer_error().unwrap());
            } else {
                err.into_stream_error().unwrap()
            }
        });

        let cr = AdaptivePadding::new(throttle);
        let mut last_payload = Some(Instant::now());
        let fut = cr.map_err(|_err| ()).for_each(move |x| {
            match (x, { last_payload }) {
                (Payload::Payload(_), _) => {
                    last_payload = Some(Instant::now());
                }
                (Payload::Dummy, Some(last_p)) => {
                    let dur = Instant::now() - last_p;
                    eprintln!("{:>5} µs: {:?}", dur.as_micros(), x);
                    // Ensure that the adaptive padding produces items quicker than the throttle
                    assert!(dur > (MS_MIN - MS_1));
                    last_payload = None;
                }
                (Payload::Dummy, None) => {
                    // We do not care about this case
                }
            };
            future::ok(())
        });

        tokio::run(fut);
    }
}
