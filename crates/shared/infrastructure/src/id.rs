//! UUID version 7 generator adapter.

use shared_domain::{Uuid, UuidGenerator};

/// Generates time-ordered UUIDs (version 7).
#[derive(Debug, Clone, Copy, Default)]
pub struct UuidV7Generator;

impl UuidGenerator for UuidV7Generator {
    #[expect(
        clippy::disallowed_methods,
        reason = "the adapter that owns identifier generation"
    )]
    fn generate(&self) -> Uuid {
        Uuid::from_uuid(uuid::Uuid::now_v7())
    }
}
