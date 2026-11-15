//! REST adapter of the event stream: the state its handlers read, the router that registers
//! them, the frames of the streaming transports and the hub that hands them to the subscribers.

pub mod dto;
pub mod frame;
pub mod handler;
pub mod hub;

use std::sync::Arc;

use axum::extract::FromRef;
use shared_domain::QueryBus;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// What the handlers of the event stream need.
#[derive(Debug, Clone)]
pub struct EventStreamState {
    /// Answers the queries.
    pub query_bus: Arc<dyn QueryBus>,
}

/// The routes of the event stream, to be nested under `/api`.
pub fn router<S>() -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
    EventStreamState: FromRef<S>,
{
    OpenApiRouter::new().routes(routes!(handler::list_event_types))
}
