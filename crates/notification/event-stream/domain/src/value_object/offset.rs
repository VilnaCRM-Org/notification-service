//! The Kafka offset of a record.

use crate::value_object::sequence::Sequence;

/// The Kafka offset of a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset(u64);

impl Offset {
    /// The offset `value`.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The number.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// The wire id of the record at this offset: offset + 1. An offset is at most `i64::MAX`, so
    /// the sum never overflows; it is still written saturating.
    #[must_use]
    pub const fn sequence(self) -> Sequence {
        Sequence::following(self.0)
    }
}

#[cfg(test)]
mod tests;
