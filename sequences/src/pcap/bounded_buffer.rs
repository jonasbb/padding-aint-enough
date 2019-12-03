//! Buffer which keeps the last `max_size` elements added to it.

use std::collections::VecDeque;

/// Buffer which keeps the last `max_size` elements added to it.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct BoundedBuffer<T>
where
    T: PartialEq,
{
    buffer: VecDeque<T>,
    max_size: usize,
}

impl<T> BoundedBuffer<T>
where
    T: PartialEq,
{
    /// Create a new buffer with a given internal size
    ///
    /// # Panics
    ///
    /// `max_size` must be larger than 0 otherwise this constructor panics.
    pub fn new(max_size: usize) -> Self {
        assert!(max_size > 0);

        Self {
            buffer: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Add a new element to the buffer, removing older elements if the size grows too large.
    ///
    /// If the element is already in the buffer, then this is a no-op.
    /// Returns `true` if the element was successfully added and `false` if it already existed.
    pub fn add(&mut self, element: T) -> bool {
        if self.contains(&element) {
            false
        } else {
            // Make sure to remove the element first because otherwise the buffer will grow to `max_size` + 1.
            if self.buffer.len() == self.max_size {
                self.buffer.pop_front();
            }
            self.buffer.push_back(element);
            true
        }
    }

    /// Returns `true` if the buffer contains the `element`.
    pub fn contains(&self, element: &T) -> bool {
        self.buffer.contains(element)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_add() {
        let mut buffer = BoundedBuffer::new(3);
        assert_eq!(buffer.add(1), true);
        assert_eq!(buffer.add(2), true);

        // Duplicate adds
        assert_eq!(buffer.add(2), false);
        assert_eq!(buffer.add(1), false);

        assert_eq!(buffer.add(3), true);
        // This kicks out element `1`
        assert_eq!(buffer.add(4), true);
        assert_eq!(buffer.add(1), true);
    }

    #[test]
    fn test_add_contains() {
        let mut buffer = BoundedBuffer::new(3);
        assert_eq!(buffer.add(1), true);
        assert_eq!(buffer.add(2), true);
        assert_eq!(buffer.add(3), true);

        assert_eq!(buffer.contains(&1), true);
        assert_eq!(buffer.contains(&2), true);
        assert_eq!(buffer.contains(&3), true);

        assert_eq!(buffer.add(4), true);
        assert_eq!(buffer.contains(&1), false);
    }
}
