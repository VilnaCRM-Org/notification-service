//! Liveness: `GET /ping` answers `pong` and touches no dependency (AD-12).

use axum::http::StatusCode;
use shared_application::http::Problem;

/// Answers `pong` as `text/plain; charset=utf-8`.
///
/// The handler takes no state, so it keeps answering while every dependency is down. It is an
/// operation of the `OpenAPI` document, outside `/api` (ADR 0022).
#[utoipa::path(
    get,
    path = "/ping",
    operation_id = "ping_get",
    tag = "HealthCheck",
    summary = "Checks that the process is alive.",
    description = "Answers `pong` without touching any dependency, so it keeps answering while the cache, the message broker or the event stream is down; the container health check requests it. Readiness is `GET /api/health`.",
    responses(
        (status = 200, description = "The process is alive.", body = String, content_type = "text/plain",
            example = json!("pong")),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json",
            example = json!(Problem::new(StatusCode::INTERNAL_SERVER_ERROR))),
    )
)]
pub async fn ping() -> &'static str {
    "pong"
}

#[cfg(test)]
mod tests;
