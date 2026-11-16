//! The two ends of the partition.

use crate::value_object::cursor::Cursor;

/// The two ends of the partition as positions, read at one instant: `earliest` is the log start
/// offset, `end` the last stable offset (read-committed). An empty topic has `earliest == end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Watermarks {
    /// The log start offset: the lowest position still retained.
    pub earliest: Cursor,
    /// The last stable offset: the position after the last committed record.
    pub end: Cursor,
}

#[cfg(test)]
mod tests;
