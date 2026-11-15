//! The name of an event type.

use std::fmt;

use crate::error::InvalidEvent;

/// 1 to 128 characters matching `^[a-z][a-z0-9]*([._-][a-z0-9]+)*$` (checked by hand, no regex
/// crate). Names order by their bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventType(String);

impl EventType {
    /// The largest number of characters.
    pub const MAX_CHARS: usize = 128;

    /// Validates the name; the length is counted with `chars().count()` and checked first.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidEvent::TypeLength`] when the value is empty or longer than
    /// [`Self::MAX_CHARS`], and [`InvalidEvent::TypePattern`] when it does not match the grammar.
    pub fn new(value: String) -> Result<Self, InvalidEvent> {
        if !(1..=Self::MAX_CHARS).contains(&value.chars().count()) {
            return Err(InvalidEvent::TypeLength);
        }
        if !matches_grammar(&value) {
            return Err(InvalidEvent::TypePattern);
        }
        Ok(Self(value))
    }

    /// The name as given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A lower-case letter first, then runs of lower-case letters and digits joined by single
/// separators `.`, `_` or `-`, never ending with a separator.
fn matches_grammar(value: &str) -> bool {
    let mut bytes = value.bytes();
    if !bytes.next().is_some_and(|first| first.is_ascii_lowercase()) {
        return false;
    }
    let mut after_separator = false;
    for byte in bytes {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            after_separator = false;
        } else if is_separator(byte) && !after_separator {
            after_separator = true;
        } else {
            return false;
        }
    }
    !after_separator
}

const fn is_separator(byte: u8) -> bool {
    matches!(byte, b'.' | b'_' | b'-')
}

#[cfg(test)]
mod tests;
