// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Fixed-capacity ring buffer for short-window history (live graphs).

use std::collections::VecDeque;

/// Keeps the most recent `capacity` values, dropping the oldest when full.
#[derive(Debug, Clone, PartialEq)]
pub struct RingBuffer<T> {
    buf: VecDeque<T>,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// Creates an empty buffer. A `capacity` of 0 is raised to 1.
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self { buf: VecDeque::with_capacity(capacity), capacity }
    }

    /// Appends a value, evicting the oldest one if the buffer is full.
    pub fn push(&mut self, value: T) {
        if self.buf.len() == self.capacity {
            self.buf.pop_front();
        }
        self.buf.push_back(value);
    }

    /// Number of stored values.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Whether the buffer holds no values.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Maximum number of stored values.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The most recently pushed value.
    pub fn latest(&self) -> Option<&T> {
        self.buf.back()
    }

    /// Iterates from oldest to newest.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buf.iter()
    }

    /// Removes all values.
    pub fn clear(&mut self) {
        self.buf.clear();
    }
}
