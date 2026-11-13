//! UUID generator double.

use std::sync::atomic::{AtomicU64, Ordering};

use shared_domain::{Uuid, UuidGenerator};

/// The counter occupies the low 62 bits; the two bits above it are the variant.
const COUNTER_MASK: u64 = (1 << 62) - 1;

/// Returns version-7-shaped UUIDs with increasing counters.
#[derive(Debug)]
pub struct SequenceUuidGenerator(AtomicU64);

impl SequenceUuidGenerator {
    /// A generator whose first counter is `first`.
    #[must_use]
    pub const fn starting_at(first: u64) -> Self {
        Self(AtomicU64::new(first))
    }
}

impl UuidGenerator for SequenceUuidGenerator {
    fn generate(&self) -> Uuid {
        let counter = self.0.fetch_add(1, Ordering::SeqCst) & COUNTER_MASK;
        let version = 0x7000_u128 << 64;
        let variant = 0x8000_u128 << 48;
        Uuid::from_uuid(uuid::Uuid::from_u128(
            version | variant | u128::from(counter),
        ))
    }
}

#[cfg(test)]
mod tests;
