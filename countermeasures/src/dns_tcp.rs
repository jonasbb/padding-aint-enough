use byteorder::{BigEndian, ByteOrder};
use futures::Stream;
use log::{debug, trace};
use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::prelude::*;

/// Defines what element the stream is expecting to read next
#[derive(Debug)]
enum DnsBytesReadState {
    /// Indicator to read the length header
    Length,
    /// Indicator to read the DNS message
    DnsMessage,
}

/// Stream which reads a DNS over TCP style communication.
pub struct DnsBytesStream<R>
where
    R: Unpin,
{
    /// Underlying reader to read a byte stream.
    read: R,
    /// Internal buffer to parse and assemble individual DNS messages.
    buf: Vec<u8>,
    /// How many bytes we want to read next.
    ///
    /// This is either the header length (2 bytes length prefix) or the lengths of the DNS message as given by the length prefix.
    expected_bytes: usize,
    /// What to read next.
    read_state: DnsBytesReadState,
}

impl<R> DnsBytesStream<R>
where
    R: Unpin,
{
    pub fn new(read: R) -> Self {
        Self {
            read,
            // This is larger than the maximal size of an IP packet and should suffice.
            buf: Vec::with_capacity(u16::max_value() as usize),
            expected_bytes: 2,
            read_state: DnsBytesReadState::Length,
        }
    }
}

impl<R> Stream for DnsBytesStream<R>
where
    R: AsyncRead + Unpin,
{
    // The same as our future above:
    type Item = Result<Vec<u8>, io::Error>;

    // poll is very similar to our Future implementation, except that
    // it returns an `Option<u8>` instead of a `u8`. This is so that the
    // Stream can signal that it's finished by returning `None`:
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;
        debug!(
            "Read {} bytes, expects {} bytes, missing {} bytes",
            this.buf.len(),
            this.expected_bytes,
            this.expected_bytes.saturating_sub(this.buf.len()),
        );

        if this.buf.len() < this.expected_bytes {
            match Pin::new(&mut this.read).poll_read_buf(cx, &mut this.buf) {
                Poll::Ready(Ok(n)) => {
                    // By convention, if an AsyncRead says that it read 0 bytes,
                    // we should assume that it has got to the end, so we signal that
                    // the Stream is done in this case by returning None:
                    if n == 0 {
                        return Poll::Ready(None);
                    }
                }
                Poll::Ready(Err(err)) => return Poll::Ready(Some(Err(err))),
                Poll::Pending => return Poll::Pending,
            }
        }

        // now that we read more, we may be able to process it
        if this.buf.len() >= this.expected_bytes {
            match &this.read_state {
                DnsBytesReadState::Length => {
                    let len = BigEndian::read_u16(&this.buf[0..this.expected_bytes]) as usize;
                    // remove the bytes
                    this.buf.drain(0..this.expected_bytes);
                    trace!("Read length field: {}", len);

                    // init next state
                    this.expected_bytes = len;
                    this.read_state = DnsBytesReadState::DnsMessage;
                    // poll again, since we might be able to make progress
                    Pin::new(this).poll_next(cx)
                }
                DnsBytesReadState::DnsMessage => {
                    let ret = this.buf[0..this.expected_bytes].to_vec();
                    // remove the bytes
                    this.buf.drain(0..this.expected_bytes);
                    trace!("Read DNS message of {} bytes", ret.len());

                    // init next state
                    this.expected_bytes = 2;
                    this.read_state = DnsBytesReadState::Length;
                    Poll::Ready(Some(Ok(ret)))
                }
            }
        } else {
            Poll::Pending
        }
    }
}
