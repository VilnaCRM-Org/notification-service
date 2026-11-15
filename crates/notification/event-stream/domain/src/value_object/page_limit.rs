//! The bounds of one replay page.

/// Bounds of one replay page: a number of events and a number of bytes of `json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageLimit {
    /// The largest number of events of a page.
    pub rows: usize,
    /// The largest number of `json` bytes of a page.
    pub bytes: usize,
}

impl PageLimit {
    /// True when the events reach `rows`, or their json bytes reach `bytes`.
    #[must_use]
    pub const fn reached(&self, rows: usize, bytes: usize) -> bool {
        rows >= self.rows || bytes >= self.bytes
    }
}

#[cfg(test)]
mod tests;
