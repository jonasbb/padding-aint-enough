use crate::Error;
use tokio::prelude::*;
use trust_dns_proto::{
    op::message::Message,
    rr::rdata::opt::{EdnsCode, EdnsOption},
    serialize::binary::BinDecodable,
};

const BLOCK_SIZE: usize = 128;
static PADDING_BYTES: [u8; 2 * BLOCK_SIZE] = [0; 2 * BLOCK_SIZE];

/// Ensure that each message gets padded appropriatly
pub struct EnsurePadding<S> {
    /// Underlying reader to read a byte stream.
    stream: S,
}

impl<S> EnsurePadding<S> {
    pub fn new(stream: S) -> Self {
        Self { stream }
    }
}

impl<S> Stream for EnsurePadding<S>
where
    S: Stream<Item = Vec<u8>, Error = Error>,
{
    // The same as our future above:
    type Item = Message;
    type Error = Error;

    fn poll(&mut self) -> Result<Async<Option<Self::Item>>, Error> {
        match self.stream.poll() {
            Ok(Async::Ready(Some(bytes))) => {
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

                Ok(Async::Ready(Some(msg)))
            }
            Ok(Async::Ready(None)) => Ok(Async::Ready(None)),

            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}
