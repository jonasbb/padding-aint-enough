extern crate byteorder;
extern crate bytes;
extern crate failure;
#[macro_use]
extern crate failure_derive;
#[macro_use]
extern crate log;

mod constants;
mod decoder;
mod encoder;

pub use decoder::{DecodeError, DecoderReader, Frame};
pub use encoder::EncoderWriter;

#[cfg(test)]
mod tests;
