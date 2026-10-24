//! Fixed-capacity history buffer used by every chart.

use std::collections::VecDeque;

/// A ring buffer that keeps the most recent `capacity` samples, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingBuffer<T> {
    capacity: usize,
    items: VecDeque<T>,
}

impl<T> RingBuffer<T> {
    /// Create a buffer. A capacity of zero is raised to one: a chart with no data
    /// is a bug, not a configuration.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            items: VecDeque::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Push a value, dropping the oldest when full.
    pub fn push(&mut self, value: T) {
        if self.items.len() == self.capacity {
            self.items.pop_front();
        }
        self.items.push_back(value);
    }

    /// Change the capacity, keeping the newest values.
    pub fn resize(&mut self, capacity: usize) {
        self.capacity = capacity.max(1);
        while self.items.len() > self.capacity {
            self.items.pop_front();
        }
    }

    /// Oldest to newest.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator {
        self.items.iter()
    }

    pub fn last(&self) -> Option<&T> {
        self.items.back()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

impl<T: Copy> RingBuffer<T> {
    /// Minimum and maximum of the current contents, ignoring NaN.
    pub fn min_max(&self) -> Option<(T, T)>
    where
        T: PartialOrd,
    {
        let mut iter = self.items.iter().copied();
        let first = iter.next()?;
        let mut min = first;
        let mut max = first;
        for value in iter {
            if value < min {
                min = value;
            }
            if value > max {
                max = value;
            }
        }
        Some((min, max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_zero_becomes_one() {
        assert_eq!(RingBuffer::<u8>::new(0).capacity(), 1);
    }

    #[test]
    fn drops_the_oldest_when_full() {
        let mut buffer = RingBuffer::new(3);
        for value in 1..=5 {
            buffer.push(value);
        }
        assert_eq!(buffer.iter().copied().collect::<Vec<_>>(), vec![3, 4, 5]);
    }

    #[test]
    fn resize_keeps_newest() {
        let mut buffer = RingBuffer::new(5);
        for value in 1..=5 {
            buffer.push(value);
        }
        buffer.resize(2);
        assert_eq!(buffer.iter().copied().collect::<Vec<_>>(), vec![4, 5]);
        buffer.resize(4);
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn min_max_is_order_independent() {
        let mut buffer = RingBuffer::new(4);
        for value in [5.0_f32, 1.0, 9.0, 3.0] {
            buffer.push(value);
        }
        assert_eq!(buffer.min_max(), Some((1.0, 9.0)));
    }

    #[test]
    fn empty_min_max_is_none() {
        assert_eq!(RingBuffer::<f32>::new(4).min_max(), None);
    }
}
