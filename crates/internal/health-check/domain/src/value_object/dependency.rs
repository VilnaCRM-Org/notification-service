//! The dependencies whose health the service reports.

/// A dependency that the service probes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dependency {
    /// The cache.
    Cache,
    /// The tail of the event topic of this node (its readiness, not a broker read).
    EventStream,
}

impl Dependency {
    /// The wire identifier of the dependency.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Cache => "cache",
            Self::EventStream => "event_stream",
        }
    }

    /// The message that reports the dependency as failing; identical in every environment.
    #[must_use]
    pub const fn failure_message(self) -> &'static str {
        match self {
            Self::Cache => "Cache is not working",
            Self::EventStream => "Event stream is not ready",
        }
    }
}

#[cfg(test)]
mod tests;
