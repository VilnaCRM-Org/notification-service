//! Domain errors of the example context.

/// A value that breaks an invariant of `ExampleApiResource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidExampleApiResource {
    /// The name has fewer than 1 or more than 255 characters.
    #[error("This value must contain between 1 and 255 characters.")]
    NameLength,
    /// The name contains U+0000.
    #[error("This value must not contain the NUL character.")]
    NameContainsNul,
    /// The description has fewer than 1 or more than 1024 characters.
    #[error("This value must contain between 1 and 1024 characters.")]
    DescriptionLength,
    /// The description contains U+0000.
    #[error("This value must not contain the NUL character.")]
    DescriptionContainsNul,
}

impl InvalidExampleApiResource {
    /// The property that carries the invalid value.
    #[must_use]
    pub const fn property_path(self) -> &'static str {
        match self {
            Self::NameLength | Self::NameContainsNul => "name",
            Self::DescriptionLength | Self::DescriptionContainsNul => "description",
        }
    }

    /// The machine-readable violation code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NameLength | Self::DescriptionLength => "length",
            Self::NameContainsNul | Self::DescriptionContainsNul => "does_not_contain",
        }
    }
}

#[cfg(test)]
mod tests;
