use crate::Payload;
use futures::Stream;
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub struct PassThrough<S, T>
where
    S: Stream<Item = T> + Unpin,
{
    stream: S,
}

impl<S, T> PassThrough<S, T>
where
    S: Stream<Item = T> + Unpin,
{
    pub fn new(stream: S) -> Self {
        PassThrough { stream }
    }
}

impl<S, T> Stream for PassThrough<S, T>
where
    S: Stream<Item = T> + Unpin,
{
    type Item = Payload<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Ready(payload) => Poll::Ready(payload.map(Payload::Payload)),
            Poll::Pending => Poll::Pending,
        }
    }
}
