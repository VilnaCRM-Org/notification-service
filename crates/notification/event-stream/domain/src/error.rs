//! Domain errors of the event stream.

/// A value that breaks an invariant of an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidEvent {
    /// The event type has fewer than 1 or more than 128 characters.
    #[error("An event type must contain between 1 and 128 characters.")]
    TypeLength,
    /// The event type does not match the grammar of event type names.
    #[error("An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$.")]
    TypePattern,
    /// The sequence is 0; the wire ids of events start at 1.
    #[error("A sequence must be 1 or greater.")]
    SequenceZero,
}

impl InvalidEvent {
    /// The machine-readable violation code: `length`, `pattern` or `range`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TypeLength => "length",
            Self::TypePattern => "pattern",
            Self::SequenceZero => "range",
        }
    }
}

/// A text that is not a decimal unsigned 64-bit integer, read as a cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("A cursor must be a decimal unsigned 64-bit integer.")]
pub struct InvalidCursor;

#[cfg(test)]
mod tests;
