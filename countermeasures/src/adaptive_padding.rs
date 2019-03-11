use crate::{error::Error, Payload};
use futures::{future::Future, stream, Async, Poll, Stream};
use log::debug;
use rand::{
    distributions::{Distribution, WeightedError, WeightedIndex},
    thread_rng,
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tokio_timer::Delay;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
enum Event<T> {
    Timeout,
    Payload(T),
    PayloadEnd,
}

pub struct AdaptivePadding<T> {
    stream: Box<dyn Stream<Item = Event<T>, Error = Error> + Send + 'static>,
    eipi: Duration,
    deadline: Delay,
    distribution: BTreeMap<Duration, u16>,
    last_created_item: Instant,
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
            eipi: Duration::from_millis(0),
            deadline: Delay::new(Instant::now()),
            distribution: BTreeMap::default(),
            last_created_item: Instant::now(),
        };
        res.refill_distribution();
        res.sample_eipi();
        res
    }

    fn sample_eipi(&mut self) {
        // Build a distribution based on the counts in self.distribution
        let dist = match WeightedIndex::new(self.distribution.iter().map(|item| item.1)) {
            Ok(dist) => dist,
            Err(WeightedError::NoItem) | Err(WeightedError::AllWeightsZero) => {
                self.refill_distribution();
                WeightedIndex::new(self.distribution.iter().map(|item| item.1)).unwrap()
            }
            Err(WeightedError::NegativeWeight) => {
                panic!("Negative weights are impossible due to the type being u16")
            }
        };
        // Get the index of the value
        let idx = dist.sample(&mut thread_rng());
        // Retrieve the matching element from the distribution
        let (value, count) = self.distribution.iter_mut().nth(idx).unwrap();
        *count -= 1;
        self.eipi = *value;

        let now = Instant::now();
        let deadline = now + self.eipi;
        self.deadline.reset(deadline);

        debug!("Sampled {:?} as EIPI", self.eipi);
    }

    fn refill_distribution(&mut self) {
        let dist = [(8, 4), (16, 3), (32, 2), (64, 1)];
        for &(value, count) in &dist {
            *self
                .distribution
                .entry(Duration::from_millis(value))
                .or_insert(0) += count;
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
                        debug!("EIPI timeout expired");
                        self.sample_eipi();
                        Some(Payload::Dummy)
                    }
                    Event::Payload(p) => {
                        debug!("Payload received");

                        // Calculate real duration
                        let dur = Instant::now() - self.last_created_item;
                        debug!("Real duration is {:?}", dur);
                        // Put token back into bucket
                        if let Some(x) = self.distribution.get_mut(&self.eipi) {
                            *x += 1;
                        }
                        // Find next bucket larger with count larger zero and remove token
                        if let Some((_duration, count)) = self
                            .distribution
                            .iter_mut()
                            .find(|(duration, count)| **duration >= dur && **count > 0)
                        {
                            *count -= 1;
                        } else {
                            self.refill_distribution();
                            if let Some((_duration, count)) = self
                                .distribution
                                .iter_mut()
                                .find(|(duration, count)| **duration >= dur && **count > 0)
                            {
                                *count -= 1;
                            }
                        }
                        // Sample new token
                        self.sample_eipi();

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
