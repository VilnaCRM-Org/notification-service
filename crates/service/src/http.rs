//! HTTP assembly of the service: the application state, the router and the middleware stack.

use axum::extract::FromRef;
use health_check_application::HealthCheckState;
use some_module_application::http::SomeModuleState;

pub mod docs;
pub mod ping;
pub mod request_id;
pub mod router;
pub mod security_headers;
pub mod trace;

/// The state of the whole application: one field per bounded context, each extracted with
/// [`FromRef`] by the handlers of that context.
#[derive(Debug, Clone, FromRef)]
pub struct AppState {
    /// State of the health-check routes.
    pub health_check: HealthCheckState,
    /// State of the example context's routes.
    pub some_module: SomeModuleState,
}
