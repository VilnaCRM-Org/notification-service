//! HTTP assembly of the service: the application state, the router and the middleware stack.

use axum::extract::FromRef;
use event_stream_application::http::EventStreamState;
use health_check_application::HealthCheckState;

pub mod docs;
pub mod origin;
pub mod ping;
pub mod request_id;
pub mod router;
pub mod security_headers;
pub mod trace;
pub mod vary;

/// The state of the whole application: one field per bounded context, each extracted with
/// [`FromRef`] by the handlers of that context.
#[derive(Debug, Clone, FromRef)]
pub struct AppState {
    /// State of the health-check routes.
    pub health_check: HealthCheckState,
    /// State of the event stream's routes.
    pub event_stream: EventStreamState,
}
