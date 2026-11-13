//! UUID generator port.

use crate::value_object::uuid::Uuid;

/// Source of new identifiers.
pub trait UuidGenerator: std::fmt::Debug + Send + Sync {
    /// A new identifier.
    fn generate(&self) -> Uuid;
}
