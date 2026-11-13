//! The description of an `ExampleApiResource`.

use crate::error::InvalidExampleApiResource;

/// A description of 1 to 1024 Unicode scalar values without U+0000; it is stored as given, untrimmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleApiResourceDescription(String);

impl ExampleApiResourceDescription {
    /// The largest number of Unicode scalar values.
    pub const MAX_CHARS: usize = 1024;

    /// Validates the description; lengths are counted with `chars().count()`.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidExampleApiResource::DescriptionLength`] when the value is empty or longer than
    /// [`Self::MAX_CHARS`], and [`InvalidExampleApiResource::DescriptionContainsNul`] when it contains U+0000.
    pub fn new(value: String) -> Result<Self, InvalidExampleApiResource> {
        if !(1..=Self::MAX_CHARS).contains(&value.chars().count()) {
            return Err(InvalidExampleApiResource::DescriptionLength);
        }
        if value.contains('\0') {
            return Err(InvalidExampleApiResource::DescriptionContainsNul);
        }
        Ok(Self(value))
    }

    /// The description as given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests;
