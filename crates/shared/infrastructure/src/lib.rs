//! Shared kernel, infrastructure layer: the in-process bus implementations and the adapters of the
//! technical ports (cache, clock, UUID generator), plus the PostgreSQL pool factory with the
//! embedded migrations and the SQS client factory.
//!
//! Every port implemented here lives in a domain crate; this crate never names an application crate.

pub mod bus;
pub mod cache;
pub mod clock;
pub mod id;
pub mod messaging;
pub mod persistence;
