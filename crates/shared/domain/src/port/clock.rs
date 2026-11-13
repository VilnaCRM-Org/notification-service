//! Clock port.

/// Source of the current time.
pub trait Clock: std::fmt::Debug + Send + Sync {
    /// The current time.
    fn now(&self) -> std::time::SystemTime;
}
