//! The resume position of a subscriber.

use std::fmt;

use crate::error::InvalidCursor;
use crate::value_object::offset::Offset;
use crate::value_object::sequence::Sequence;

/// "Everything up to this id was seen". A cursor `c` is also the Kafka position `c`: the next
/// offset to read is `c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cursor(u64);

impl Cursor {
    /// The cursor before every event.
    pub const START: Self = Self(0);

    /// The cursor `value`.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Reads ASCII digits only (no sign, no blank, not empty) as an unsigned 64-bit integer.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidCursor`] when the text holds anything but digits, is empty, or is above
    /// `u64::MAX`.
    pub fn parse(text: &str) -> Result<Self, InvalidCursor> {
        if !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(InvalidCursor);
        }
        text.parse().ok().map(Self).ok_or(InvalidCursor)
    }

    /// The number.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }

    /// The offset to read next for this cursor: the same number.
    #[must_use]
    pub const fn next_offset(self) -> Offset {
        Offset::new(self.0)
    }

    /// The cursor `n` positions lower, saturating at 0.
    #[must_use]
    pub const fn rewound_by(self, n: u64) -> Self {
        Self(self.0.saturating_sub(n))
    }

    /// The cursor `n` positions higher, saturating at `u64::MAX`.
    #[must_use]
    pub const fn advanced_by(self, n: u64) -> Self {
        Self(self.0.saturating_add(n))
    }

    /// The number of positions from `lower` up to this cursor; 0 when `lower` is not below it.
    #[must_use]
    pub const fn distance_from(self, lower: Self) -> u64 {
        self.0.saturating_sub(lower.0)
    }
}

impl From<Sequence> for Cursor {
    fn from(sequence: Sequence) -> Self {
        Self(sequence.value())
    }
}

impl fmt::Display for Cursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests;
