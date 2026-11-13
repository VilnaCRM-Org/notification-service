//! UUID value object.

/// A syntactically valid UUID of any version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Uuid(uuid::Uuid);

/// The text is not a UUID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{value}` is not a valid UUID")]
pub struct InvalidUuid {
    /// The rejected text.
    pub value: String,
}

impl Uuid {
    /// Parses a UUID of any version in any form that `uuid::Uuid::try_parse`
    /// accepts (simple, hyphenated, braced or URN).
    ///
    /// # Errors
    ///
    /// Returns [`InvalidUuid`] when the text is not a UUID.
    pub fn parse(value: &str) -> Result<Self, InvalidUuid> {
        uuid::Uuid::try_parse(value)
            .map(Self)
            .map_err(|_error| InvalidUuid {
                value: value.to_owned(),
            })
    }

    /// Wraps a UUID.
    #[must_use]
    pub const fn from_uuid(value: uuid::Uuid) -> Self {
        Self(value)
    }

    /// Returns the wrapped UUID.
    #[must_use]
    pub const fn as_uuid(&self) -> uuid::Uuid {
        self.0
    }
}

impl std::fmt::Display for Uuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests;
