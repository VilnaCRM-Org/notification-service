//! Example bounded context, domain layer: the `ExampleApiResource` aggregate, its value objects,
//! its domain event, its domain errors and the repository port.
//!
//! This crate is framework-free; replace it with the aggregates of the real module.

pub mod aggregate;
pub mod error;
pub mod event;
pub mod repository;
pub mod value_object;

pub use aggregate::ExampleApiResource;
pub use error::InvalidExampleApiResource;
pub use event::ExampleApiResourceCreated;
pub use repository::{ExampleApiResourceRepository, RepositoryError};
pub use value_object::description::ExampleApiResourceDescription;
pub use value_object::id::ExampleApiResourceId;
pub use value_object::name::ExampleApiResourceName;
