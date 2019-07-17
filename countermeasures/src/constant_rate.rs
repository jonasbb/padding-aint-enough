use crate::Payload;
use futures::{task::Context, Poll, Stream};
use std::{pin::Pin, time::Duration};
use tokio_timer::Interval;

pub struct ConstantRate<S, T>
where
    S: Stream<Item = T> + Unpin,
{
    interval: Interval,
    stream: S,
}

impl<S, T> ConstantRate<S, T>
where
    S: Stream<Item = T> + Unpin,
{
    pub fn new(stream: S, interval: Duration) -> Self {
        Self {
            interval: Interval::new_interval(interval),
            stream,
        }
    }
}

impl<S, T> Stream for ConstantRate<S, T>
where
    S: Stream<Item = T> + Unpin,
{
    type Item = Payload<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        match Pin::new(&mut this.interval).poll_next(cx) {
            Poll::Ready(Some(_)) => {
                // Time to send a new packet
                match Pin::new(&mut this.stream).poll_next(cx) {
                    Poll::Ready(Some(t)) => Poll::Ready(Some(Payload::Payload(t))),
                    Poll::Ready(None) => Poll::Ready(None),
                    Poll::Pending => {
                        // No packet to send, send dummy
                        Poll::Ready(Some(Payload::Dummy))
                    }
                }
            }
            // The timer instance is done, this should never happen
            Poll::Ready(None) => panic!("Timer instance is done. This should never happen."),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{future, stream, StreamExt};
    use std::time::Instant;

    /// [`Duration`] of exactly 5 ms
    const MS_5: Duration = Duration::from_millis(5);

    #[test]
    fn test_constant_time_ensure_time_gap() {
        let dur = Duration::new(0, 100_000_000);
        let items = stream::iter(0..10);
        let cr = ConstantRate::new(items, dur);
        let mut last = Instant::now();

        let fut = cr.for_each(move |x| {
            let now = Instant::now();
            eprintln!("{:?}: {:?}", now - last, x);
            // The precision of the timer wheel is only up to 1 ms
            assert!(now - last > (dur - MS_5), "Measured gap is lower than minimal value for constant-rate. Expected: {:?}, Found {:?}", dur-MS_5, now-last);
            last = now;
            future::ready(())
        });

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(fut);
    }

    #[test]
    fn test_constant_time_insert_dummy() {
        let dur_short = Duration::new(0, 33_000_000);
        let dur_long = Duration::new(0, 100_000_000);

        let items = stream::iter(0..10);
        let cr_slow = ConstantRate::new(items, dur_long);
        let cr = ConstantRate::new(cr_slow, dur_short);

        let mut last = Instant::now();
        let mut elements_between_dummies = 0;
        let fut = cr.for_each(move |x| {
            // Remove one layer of the douple payload
            let x = x.flatten();
            let now = Instant::now();
            eprintln!("{:?}: {:?}", now - last, x);
            // The precision of the timer wheel is only up to 1 ms
            assert!(now - last > (dur_short - MS_5), "Measured gap is lower than minimal value for constant-rate. Expected: {:?}, Found {:?}", dur_short-MS_5, now-last);
            last = now;
            if x == Payload::Dummy {
                elements_between_dummies = 0
            } else {
                elements_between_dummies += 1;
                assert!(elements_between_dummies <= 3);
            }
            future::ready(())
        });

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(fut);
    }
}
