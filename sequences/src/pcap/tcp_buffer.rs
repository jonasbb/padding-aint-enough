//! Types for TCP stream reassembly

use super::BoundedBuffer;
use failure::{bail, Error};
use std::collections::BTreeMap;

/// Type for TCP stream reassembly
pub struct TcpBuffer {
    seen_sequence_numbers: BoundedBuffer<u32>,
    next_sequence_number: Option<u32>,
    buffer: Vec<u8>,
    unprocessed_data: BTreeMap<u32, Vec<u8>>,
}

impl TcpBuffer {
    pub fn new() -> Self {
        Self {
            seen_sequence_numbers: BoundedBuffer::new(20),
            next_sequence_number: None,
            buffer: Vec::with_capacity(4096),
            unprocessed_data: BTreeMap::new(),
        }
    }

    /// Add some data to the buffer
    pub fn add_data(&mut self, sequence_number: u32, data: &[u8]) {
        // Return early on duplicate data, indentified by a known sequence number
        if self.seen_sequence_numbers.contains(&sequence_number) {
            return;
        }

        if self.next_sequence_number.is_none() || Some(sequence_number) == self.next_sequence_number
        {
            // First packet, simply accept
            // Or this is the next packet according to the sequence number, then also accept
            self.next_sequence_number = Some(sequence_number.wrapping_add(data.len() as u32));
            self.seen_sequence_numbers.add(sequence_number);
            self.buffer.extend_from_slice(data);

            // Check if we need to accept old buffered data
            if !self.unprocessed_data.is_empty() {
                // If we find some data, i.e., remove return Some, recursivly add them.
                // The recursion ensures we do this until we run out of new data to add.
                let next_sequence_number = self.next_sequence_number.unwrap();
                if let Some(data) = self.unprocessed_data.remove(&next_sequence_number) {
                    self.add_data(next_sequence_number, &*data);
                }
            }
        } else {
            self.unprocessed_data.insert(sequence_number, data.to_vec());
        }
    }

    /// View the data currently stored in the buffer
    pub fn view_data(&self) -> &[u8] {
        &self.buffer
    }

    /// Clear all the data stored in the buffer
    #[allow(dead_code)]
    pub fn clear_data(&mut self) {
        self.buffer.clear();
        self.unprocessed_data.clear();
    }

    /// Consumes `numb` of data from the start of the buffer
    pub fn consume(&mut self, numb: usize) -> Result<(), Error> {
        if numb > self.buffer.len() {
            bail!(
                "Requested to consume {} bytes of data, but the buffer only contains {}.",
                numb,
                self.buffer.len()
            );
        }
        self.buffer.drain(..numb);
        Ok(())
    }

    /// Returns `true` if there is data in the buffers
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty() && self.unprocessed_data.is_empty()
    }
}

impl Default for TcpBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_reassembly() {
        let mut buffer = TcpBuffer::new();
        buffer.add_data(1, &[0, 1, 2]);
        buffer.add_data(4, &[3, 4, 5]);
        buffer.add_data(7, &[6, 7, 8]);
        assert_eq!(&[0, 1, 2, 3, 4, 5, 6, 7, 8], buffer.view_data());
        assert_eq!(false, buffer.is_empty(), "Buffer must contain data");
        assert!(buffer.consume(9).is_ok());
        assert_eq!(true, buffer.is_empty(), "Buffer is not empty");
    }

    #[test]
    fn test_duplicate_packet() {
        let mut buffer = TcpBuffer::new();
        buffer.add_data(1, &[0, 1, 2]);
        buffer.add_data(1, &[0, 1, 2]);
        buffer.add_data(1, &[0, 1, 2]);
        buffer.add_data(4, &[3, 4, 5]);
        buffer.add_data(4, &[3, 4, 5]);
        buffer.add_data(1, &[0, 1, 2]);
        buffer.add_data(1, &[0, 1, 2]);
        buffer.add_data(7, &[6, 7, 8]);
        buffer.add_data(1, &[0, 1, 2]);
        buffer.add_data(1, &[0, 1, 2]);
        buffer.add_data(4, &[3, 4, 5]);
        buffer.add_data(4, &[3, 4, 5]);
        assert_eq!(&[0, 1, 2, 3, 4, 5, 6, 7, 8], buffer.view_data());
        assert_eq!(false, buffer.is_empty(), "Buffer must contain data");
        assert!(buffer.consume(9).is_ok());
        assert_eq!(true, buffer.is_empty(), "Buffer is not empty");
    }

    #[test]
    fn test_out_of_order() {
        let mut buffer = TcpBuffer::new();
        // The first packet must still be in order, such that the correct sequence number gets set
        buffer.add_data(1, &[0, 1, 2]);

        buffer.add_data(10, &[9, 10, 11]);
        buffer.add_data(7, &[6, 7, 8]);
        buffer.add_data(4, &[3, 4, 5]);
        assert_eq!(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], buffer.view_data());
        assert_eq!(false, buffer.is_empty(), "Buffer must contain data");
        assert!(buffer.consume(12).is_ok());
        assert_eq!(true, buffer.is_empty(), "Buffer is not empty");
    }

    #[test]
    fn test_reassembly_overflowing_sequence_number() {
        let mut buffer = TcpBuffer::new();
        buffer.add_data(u32::max_value() - 1, &[0]);
        buffer.add_data(u32::max_value(), &[1]);
        buffer.add_data(0, &[2]);
        buffer.add_data(1, &[3]);
        buffer.add_data(2, &[4]);
        assert_eq!(&[0, 1, 2, 3, 4], buffer.view_data());
    }
}
