#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

extern crate byteorder;
extern crate bytes;
#[macro_use]
extern crate log;

mod constants;
mod decoder;
mod encoder;

pub use decoder::{DecodeError, DecoderReader, Frame};
pub use encoder::EncoderWriter;

#[cfg(test)]
mod tests;
