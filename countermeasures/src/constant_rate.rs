use crate::{error::Error, Payload};
use futures::{Async, Poll, Stream};
use std::{fmt::Debug, time::Duration};
use tokio_timer::Interval;

pub struct ConstantRate<S, T>
where
    S: Stream<Item = T>,
{
    interval: Interval,
    stream: S,
}

impl<S, T, > ConstantRate<S, T>
where
    S: Stream<Item = T>,
{
    pub fn new(interval: Duration, stream: S) -> Self {
        Self {
            interval: Interval::new_interval(interval),
            stream,
        }
    }
}

impl<S, T> Stream for ConstantRate<S, T>
where
    S: Stream<Item = T>,
    S::Error: Into<Error>,
    T: Debug,
{
    type Item = Payload<T>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.interval.poll()? {
            Async::Ready(Some(_)) => {
                // Time to send a new packet
                match self.stream.poll() {
                    Ok(x) => match x {
                        Async::Ready(Some(t)) => Ok(Async::Ready(Some(Payload::Payload(t)))),
                        Async::Ready(None) => Ok(Async::Ready(None)),
                        Async::NotReady => {
                            // No packet to send, send dummy
                            Ok(Async::Ready(Some(Payload::Dummy)))
                        }
                    },
                    Err(err) => Err(err.into()),
                }
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

    #[test]
    fn test_constant_time_ensure_time_gap() {
        let one_ms = Duration::new(0, 1_000_000);
        let dur = Duration::new(0, 100_000_000);
        let items = stream::iter_ok::<_, ()>(0..10);
        let cr = ConstantRate::new(dur, items);
        let mut last = Instant::now();

        let fut = cr.map_err(|_err| ()).for_each(move |x| {
            let now = Instant::now();
            println!("{:?}: {:?}", now - last, x);
            // The precision of the timer wheel is only up to 1 ms
            assert!(now - last > (dur - one_ms));
            last = now;
            future::ok(())
        });

        tokio::run(fut);
    }

    #[test]
    fn test_constant_time_insert_dummy() {
        let one_ms = Duration::new(0, 1_000_000);
        let dur_short = Duration::new(0, 33_000_000);
        let dur_long = Duration::new(0, 100_000_000);

        let items = stream::iter_ok::<_, Error>(0..10);
        let cr_slow = ConstantRate::new(dur_long, items);
        let cr = ConstantRate::new(dur_short, cr_slow);

        let mut last = Instant::now();
        let mut elements_between_dummies = 0;
        let fut = cr.map_err(|_err| ()).for_each(move |x| {
            // Remove one layer of the douple payload
            let x = x.flatten();
            let now = Instant::now();
            println!("{:?}: {:?}", now - last, x);
            // The precision of the timer wheel is only up to 1 ms
            assert!(now - last > (dur_short - one_ms));
            last = now;
            if x == Payload::Dummy {
                elements_between_dummies = 0
            } else {
                elements_between_dummies += 1;
                assert!(elements_between_dummies <= 3);
            }
            future::ok(())
        });

        tokio::run(fut);
    }
}
