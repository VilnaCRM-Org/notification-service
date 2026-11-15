//! Bounded context `event-stream`, application layer: the catalog of subscribable event types, its
//! query and handler, the memory window of recent events, the types of the tail, the node status,
//! the application errors and the API-facing adapter (`http`, with the frames of the streaming
//! transports and the hub of this node).
//!
//! This crate depends on the domain layer of its own context and of the shared kernel, never on the
//! infrastructure layer. Module rule: the use-case modules (`catalog`, `query`, `window`, `tail`,
//! `status`, `error`) do not import `axum`, `utoipa` or `async_graphql`; only `http` (and a
//! `graphql` module) may.

pub mod catalog;
pub mod error;
pub mod http;
pub mod query;
pub mod status;
pub mod tail;
pub mod window;

pub use catalog::EventCatalog;
pub use error::EventStreamError;
pub use http::frame::Frame;
pub use http::hub::{Delivery, Hub, HubError, HubSettings, Subscriber};
pub use query::EventTypeView;
pub use query::list_event_types::{ListEventTypes, ListEventTypesHandler};
pub use status::{NodeState, NodeStatus, StartReason, StatusSource, SubscriberCount};
pub use tail::{EventPublisher, RoundFailed, TailPorts, TailSettings, TailState, TopicFault};
pub use window::{Window, WindowRead};
