//! The dependencies whose health the service reports.

/// A dependency that the service probes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dependency {
    /// The PostgreSQL database.
    Database,
    /// The cache.
    Cache,
    /// The message broker.
    MessageBroker,
}

impl Dependency {
    /// The wire identifier of the dependency.
    #[must_use]
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Cache => "cache",
            Self::MessageBroker => "message_broker",
        }
    }

    /// The message that reports the dependency as failing; identical in every environment.
    #[must_use]
    pub const fn failure_message(self) -> &'static str {
        match self {
            Self::Database => "Database is not available",
            Self::Cache => "Cache is not working",
            Self::MessageBroker => "Message broker is not available",
        }
    }
}

#[cfg(test)]
mod tests;
