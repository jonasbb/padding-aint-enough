use byteorder::{BigEndian, ByteOrder};
use log::trace;
use std::io;
use tokio::prelude::*;

/// Defines what element the stream is expecting to read next
enum DnsBytesReadState {
    /// Indicator to read the length header
    Length,
    /// Indicator to read the DNS message
    DnsMessage,
}

/// Stream which reads a DNS over TCP style communication.
pub struct DnsBytesStream<R> {
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

impl<R> DnsBytesStream<R> {
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

impl<R: AsyncRead> Stream for DnsBytesStream<R> {
    // The same as our future above:
    type Item = Vec<u8>;
    type Error = io::Error;

    // poll is very similar to our Future implementation, except that
    // it returns an `Option<u8>` instead of a `u8`. This is so that the
    // Stream can signal that it's finished by returning `None`:
    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, io::Error> {
        trace!(
            "Read {} bytes, expects {} bytes, missing {} bytes",
            self.buf.len(),
            self.expected_bytes,
            self.expected_bytes.saturating_sub(self.buf.len()),
        );
        if self.buf.len() < self.expected_bytes {
            match self.read.read_buf(&mut self.buf) {
                Ok(Async::Ready(n)) => {
                    // By convention, if an AsyncRead says that it read 0 bytes,
                    // we should assume that it has got to the end, so we signal that
                    // the Stream is done in this case by returning None:
                    if n == 0 {
                        return Ok(Async::Ready(None));
                    }
                }
                Ok(Async::NotReady) => return Ok(Async::NotReady),
                Err(e) => return Err(e),
            }
        }

        // now that we read more, we may be able to process it
        if self.buf.len() >= self.expected_bytes {
            match self.read_state {
                DnsBytesReadState::Length => {
                    let len = BigEndian::read_u16(&self.buf[0..self.expected_bytes]) as usize;
                    // remove the bytes
                    self.buf.drain(0..self.expected_bytes);
                    trace!("Read length field: {}", len);

                    // init next state
                    self.expected_bytes = len;
                    self.read_state = DnsBytesReadState::DnsMessage;
                    // poll again, since we might be able to make progress
                    self.poll()
                }
                DnsBytesReadState::DnsMessage => {
                    let ret = self.buf[0..self.expected_bytes].to_vec();
                    // remove the bytes
                    self.buf.drain(0..self.expected_bytes);
                    trace!("Read DNS message of {} bytes", ret.len());

                    // init next state
                    self.expected_bytes = 2;
                    self.read_state = DnsBytesReadState::Length;
                    Ok(Async::Ready(Some(ret)))
                }
            }
        } else {
            Ok(Async::NotReady)
        }
    }
}
