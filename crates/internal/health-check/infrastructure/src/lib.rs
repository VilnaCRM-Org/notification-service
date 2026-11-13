//! Health-check context, infrastructure layer: the probes that check PostgreSQL, the cache and the
//! SQS broker connection and implement the `HealthProbe` port of the domain crate.
//!
//! This crate never names an application crate: the probes are wired in by the composition root.

pub mod probe;

pub use probe::cache::CacheProbe;
pub use probe::postgres::PostgresProbe;
pub use probe::sqs::SqsProbe;
