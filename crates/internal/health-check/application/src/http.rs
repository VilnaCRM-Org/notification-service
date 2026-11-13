//! The HTTP adapter of the health check: state, router and the `GET /health` handler.

use std::sync::Arc;

use axum::extract::{FromRef, State};
use axum::http::StatusCode;
use shared_application::http::{DependencyFailure, Problem};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::service::HealthService;

/// State of the health-check routes.
#[derive(Debug, Clone)]
pub struct HealthCheckState {
    /// The service that probes the dependencies.
    pub service: Arc<HealthService>,
}

/// The routes of the health check; `S` is the application state that holds a [`HealthCheckState`].
pub fn router<S>() -> OpenApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
    HealthCheckState: FromRef<S>,
{
    OpenApiRouter::new().routes(routes!(health))
}

/// Answers 204 when every dependency responds.
///
/// # Errors
///
/// Returns the 500 problem that lists every failing dependency.
#[utoipa::path(
    get,
    path = "/health",
    operation_id = "api_health_get",
    tag = "HealthCheck",
    summary = "Checks the health of the service.",
    description = "Returns 204 when the database, the cache and the message broker respond. Returns a 500 problem document that lists every failing dependency otherwise.",
    responses(
        (status = 204, description = "Health check passed with no content to return."),
        (
            status = 500,
            description = "Health check failed.",
            body = Problem,
            content_type = "application/problem+json"
        ),
        (
            status = "default",
            description = "Problem response.",
            body = Problem,
            content_type = "application/problem+json"
        ),
    ),
)]
pub async fn health(State(state): State<HealthCheckState>) -> Result<StatusCode, Problem> {
    let report = state.service.check().await;
    if report.is_healthy() {
        return Ok(StatusCode::NO_CONTENT);
    }
    let failures = report
        .failures()
        .iter()
        .map(|failure| DependencyFailure {
            dependency: failure.dependency.identifier().to_owned(),
            message: failure.dependency.failure_message().to_owned(),
        })
        .collect();
    Err(Problem::dependency_unavailable(failures))
}

#[cfg(test)]
mod tests;
