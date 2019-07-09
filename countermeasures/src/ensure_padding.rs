use crate::Error;
use futures::{Poll, Stream};
use trust_dns_proto::{
    op::message::Message,
    rr::rdata::opt::{EdnsCode, EdnsOption},
    serialize::binary::BinDecodable,
};
use std::pin::Pin;
use futures::task::Context;
use std::io;

const BLOCK_SIZE: usize = 128;
static PADDING_BYTES: [u8; 2 * BLOCK_SIZE] = [0; 2 * BLOCK_SIZE];

/// Ensure that each message gets padded appropriatly
pub struct EnsurePadding<S>
where
    S: Stream<Item = Result<Vec<u8>, io::Error>> + Unpin, {
    /// Underlying reader to read a byte stream.
    stream: S,
}

impl<S> EnsurePadding<S>
    where
    S: Stream<Item = Result<Vec<u8>, io::Error>> + Unpin, {
    pub fn new(stream: S) -> Self{
        Self { stream }
    }
}

impl<S> Stream for EnsurePadding<S>
where
    S: Stream<Item = Result<Vec<u8>, io::Error>> + Unpin,
{
    // The same as our future above:
    type Item = Result<Message, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.stream).poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                let len = bytes.len();
                // Round to next multiple of BLOCK_SIZE
                let padded_len = (len + BLOCK_SIZE - 1) / BLOCK_SIZE * BLOCK_SIZE;
                let mut missing_padding = padded_len - len;

                // Even an empty padding option is at least 4B in size,
                // as this is the overhead for length and type of each EDNS option
                if missing_padding < 4 {
                    missing_padding += BLOCK_SIZE;
                }
                // substract overhead
                missing_padding -= 4;

                let mut msg = Message::from_bytes(&bytes)?;
                if msg.edns().is_none() {
                    // The size of the EDNS opt is 11B
                    if missing_padding < 11 {
                        missing_padding += BLOCK_SIZE;
                    }
                    // substract overhead
                    missing_padding -= 11;
                }

                if let Some(EdnsOption::Unknown(12, padding)) =
                    msg.edns_mut().option(EdnsCode::Padding)
                {
                    // Add the size of the padding we already have, since we replace that now
                    missing_padding += 4 + padding.len();
                    missing_padding %= BLOCK_SIZE;
                }

                // Set the missing padding option
                msg.edns_mut().set_option(EdnsOption::from((
                    EdnsCode::Padding,
                    &PADDING_BYTES[0..missing_padding],
                )));

                Poll::Ready(Some(Ok(msg)))
            }
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),

            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
