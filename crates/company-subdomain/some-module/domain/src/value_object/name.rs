//! The name of an `ExampleApiResource`.

use crate::error::InvalidExampleApiResource;

/// A name of 1 to 255 Unicode scalar values without U+0000; it is stored as given, untrimmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExampleApiResourceName(String);

impl ExampleApiResourceName {
    /// The largest number of Unicode scalar values.
    pub const MAX_CHARS: usize = 255;

    /// Validates the name; lengths are counted with `chars().count()`.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidExampleApiResource::NameLength`] when the value is empty or longer than
    /// [`Self::MAX_CHARS`], and [`InvalidExampleApiResource::NameContainsNul`] when it contains U+0000.
    pub fn new(value: String) -> Result<Self, InvalidExampleApiResource> {
        if !(1..=Self::MAX_CHARS).contains(&value.chars().count()) {
            return Err(InvalidExampleApiResource::NameLength);
        }
        if value.contains('\0') {
            return Err(InvalidExampleApiResource::NameContainsNul);
        }
        Ok(Self(value))
    }

    /// The name as given.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests;
