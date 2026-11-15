//! Bounded context `event-stream`, domain layer: aggregates, value objects, domain events, domain
//! errors and the ports (such as repository traits) that the infrastructure layer implements.
//!
//! This crate is framework-free. It depends on domain crates only: its own context and the shared
//! kernel, never another bounded context.

pub mod error;
pub mod value_object;

pub use error::InvalidEvent;
pub use value_object::event_type::EventType;
