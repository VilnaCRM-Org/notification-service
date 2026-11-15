//! Bounded context `event-stream`, application layer: the catalog of subscribable event types, its
//! query and handler, the application errors and the API-facing adapter (`http`).
//!
//! This crate depends on the domain layer of its own context and of the shared kernel, never on the
//! infrastructure layer. Module rule: the use-case modules (`catalog`, `query`, `error`) do not
//! import `axum`, `utoipa` or `async_graphql`; only `http` (and a `graphql` module) may.

pub mod catalog;
pub mod error;
pub mod http;
pub mod query;

pub use catalog::EventCatalog;
pub use error::EventStreamError;
pub use query::EventTypeView;
pub use query::list_event_types::{ListEventTypes, ListEventTypesHandler};
