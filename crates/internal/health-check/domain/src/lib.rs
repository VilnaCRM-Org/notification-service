//! Health-check context, domain layer: the `HealthProbe` port and the value objects that describe a
//! dependency and the outcome of probing it.
//!
//! This crate is framework-free and names no other bounded context.

pub mod port;
pub mod value_object;

pub use port::HealthProbe;
pub use value_object::dependency::Dependency;
pub use value_object::report::{HealthReport, ProbeFailure, ProbeFailureCause};
