//! Identifier of an `ExampleApiResource`.

use shared_domain::Uuid;

/// The identifier of an `ExampleApiResource`; it never changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExampleApiResourceId(Uuid);

impl ExampleApiResourceId {
    /// Wraps a UUID.
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the wrapped UUID.
    #[must_use]
    pub const fn value(self) -> Uuid {
        self.0
    }
}

impl std::fmt::Display for ExampleApiResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests;
