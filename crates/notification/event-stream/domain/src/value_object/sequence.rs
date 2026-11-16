//! The wire id of an event.

use std::fmt;

use crate::error::InvalidEvent;

/// The wire id of an event, 1 or greater: the Kafka offset of its record plus 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sequence(u64);

impl Sequence {
    /// Accepts any value of 1 or greater.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidEvent::SequenceZero`] when the value is 0.
    pub const fn new(value: u64) -> Result<Self, InvalidEvent> {
        if value == 0 {
            Err(InvalidEvent::SequenceZero)
        } else {
            Ok(Self(value))
        }
    }

    /// The sequence that follows `value`, saturating at `u64::MAX`; never 0.
    pub(crate) const fn following(value: u64) -> Self {
        Self(value.saturating_add(1))
    }

    /// The number.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Sequence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

#[cfg(test)]
mod tests;
