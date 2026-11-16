//! Shared kernel, infrastructure layer: the in-process bus implementations and the adapters of the
//! technical ports (cache, clock, UUID generator), plus the SQS client factory.
//!
//! Every port implemented here lives in a domain crate; this crate never names an application crate.

pub mod bus;
pub mod cache;
pub mod clock;
pub mod id;
pub mod messaging;
