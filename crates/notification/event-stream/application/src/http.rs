//! REST adapter of the event stream: the state its handlers read, the router that registers
//! them, the frames of the streaming transports and the hub that hands them to the subscribers.
//!
//! It also holds the subscriptions that read every connection by position: from the window, from
//! a Kafka replay below it, then live, and the session and the pump of a WebSocket connection.

pub mod dto;
pub mod frame;
pub mod handler;
pub mod hub;
mod session;
mod socket;
pub mod subscription;
#[cfg(test)]
pub(crate) mod test_node;

use std::sync::Arc;
use std::time::Duration;

use axum::extract::FromRef;
use axum::routing::get;
use shared_domain::QueryBus;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::status::StatusSource;
use hub::Hub;
use subscription::Subscriptions;

/// The timing of the streaming transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamSettings {
    /// The interval of the SSE keep-alive comment and of the WebSocket ping, and the deadline of
    /// every WebSocket write (`STREAM_KEEP_ALIVE`).
    pub keep_alive: Duration,
}

/// What the handlers of the event stream need.
#[derive(Debug, Clone)]
pub struct EventStreamState {
    /// Answers the queries.
    pub query_bus: Arc<dyn QueryBus>,
    /// Opens the subscriptions of the streaming routes; its refusals are their 503 answers.
    pub subscriptions: Arc<Subscriptions>,
    /// Computes the status of this node.
    pub status: Arc<StatusSource>,
    /// The hub of this node's subscribers.
    pub hub: Arc<Hub>,
    /// The timing of the streaming transports.
    pub settings: StreamSettings,
}

/// The routes of the event stream, to be nested under `/api`.
///
/// The streaming routes (SSE and WebSocket) are served but are not operations of the `OpenAPI`
/// document; no route carries a layer that needs a state value.
pub fn router<S>() -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
    EventStreamState: FromRef<S>,
{
    OpenApiRouter::new()
        .routes(routes!(handler::list_event_types))
        .routes(routes!(handler::event_stream_status))
        .route("/events/stream", get(handler::stream))
        .route("/events/ws", get(handler::ws))
}
