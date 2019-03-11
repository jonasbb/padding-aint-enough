use crate::{error::Error, Payload};
use futures::{Async, Poll, Stream};

pub struct PassThrough<S, T>
where
    S: Stream<Item = T>,
{
    stream: S,
}

impl<S, T> PassThrough<S, T>
where
    S: Stream<Item = T>,
{
    pub fn new(stream: S) -> Self {
        PassThrough { stream }
    }
}

impl<S, T> Stream for PassThrough<S, T>
where
    S: Stream<Item = T>,
    Error: From<S::Error>,
{
    type Item = Payload<T>;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.stream.poll()? {
            Async::Ready(Some(payload)) => Ok(Async::Ready(Some(Payload::Payload(payload)))),
            // The timer instance is done, this should never happen
            Async::Ready(None) => panic!("Timer instance is done. This should never happen."),
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}
