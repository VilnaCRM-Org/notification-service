//! Example bounded context, infrastructure layer: the PostgreSQL implementation of the repository
//! port of `some-module-domain`, with its SQL, row mapping and error translation.
//!
//! This crate never names an application crate.

pub mod persistence;

pub use persistence::PgExampleApiResourceRepository;
