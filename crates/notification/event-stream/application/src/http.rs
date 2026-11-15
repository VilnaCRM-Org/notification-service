//! REST adapter of the event stream: the state its handlers read, the router that registers
//! them, the frames of the streaming transports and the hub that hands them to the subscribers.
//!
//! It also holds the subscriptions that read every connection by position: from the window, from
//! a Kafka replay below it, then live.

pub mod dto;
pub mod frame;
pub mod handler;
pub mod hub;
pub mod subscription;

use std::sync::Arc;

use axum::extract::FromRef;
use shared_domain::QueryBus;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::status::StatusSource;
use hub::Hub;

/// What the handlers of the event stream need.
#[derive(Debug, Clone)]
pub struct EventStreamState {
    /// Answers the queries.
    pub query_bus: Arc<dyn QueryBus>,
    /// Computes the status of this node.
    pub status: Arc<StatusSource>,
    /// The hub of this node's subscribers.
    pub hub: Arc<Hub>,
}

/// The routes of the event stream, to be nested under `/api`.
pub fn router<S>() -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
    EventStreamState: FromRef<S>,
{
    OpenApiRouter::new()
        .routes(routes!(handler::list_event_types))
        .routes(routes!(handler::event_stream_status))
}
