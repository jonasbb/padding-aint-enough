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

    const DUR_TOLERANCE: Duration = Duration::from_millis(3);

    #[test]
    fn test_constant_time_insert_dummy() {
        let dur_short = Duration::new(0, 33_000_000);
        let dur_long = Duration::new(0, 100_000_000);
        let rt = tokio::runtime::Runtime::new().unwrap();

        // This test is non-deterministic, so run it multiple times
        for _ in 0..20 {
            let items = stream::iter(0..10);
            let cr_slow = ConstantRate::new(items, dur_long);
            let cr = ConstantRate::new(cr_slow, dur_short);

            let begin = Instant::now();
            let mut element_count = 0;
            let mut elements_between_dummies = 0;
            let mut dummies_between_elements = 0;
            {
                let element_count = &mut element_count;
                let fut = cr.for_each(move |x| {
                    // Remove one layer of the douple payload
                    let x = dbg!(x.flatten());
                    *element_count += 1;
                    if x == Payload::Dummy {
                        elements_between_dummies = 0;
                        dummies_between_elements += 1;
                        assert!(dummies_between_elements <= 3);
                    } else {
                        elements_between_dummies += 1;
                        dummies_between_elements = 0;
                        assert_eq!(elements_between_dummies, 1);
                    }
                    future::ready(())
                });
                rt.block_on(fut);
            }

            let end = Instant::now();
            // The precision of the timer wheel is only up to 1 ms
            // The average time should be around 33ms, ensure that it lies in the tolerance range
            let avg_gap = (end - begin) / element_count;
            assert!(
                avg_gap > dur_short - DUR_TOLERANCE,
                "The average gap is {:?}, but should be larger than {:?}",
                avg_gap,
                dur_short - DUR_TOLERANCE
            );
            assert!(
                avg_gap < dur_short + DUR_TOLERANCE,
                "The average gap is {:?}, but should be smaller than {:?}",
                avg_gap,
                dur_short + DUR_TOLERANCE
            );
        }
    }
}
