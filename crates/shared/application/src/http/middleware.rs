//! The request budget (AD-15).

use std::time::Duration;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};

use super::problem::Problem;

/// Runs the rest of the stack under `budget`; on expiry the response is the 503 problem.
pub async fn request_timeout(
    State(budget): State<Duration>,
    request: Request,
    next: Next,
) -> Response {
    tokio::time::timeout(budget, next.run(request))
        .await
        .unwrap_or_else(|_elapsed| time_budget_exceeded().into_response())
}

fn time_budget_exceeded() -> Problem {
    Problem::new(StatusCode::SERVICE_UNAVAILABLE)
        .with_detail("The request exceeded its time budget.")
}

#[cfg(test)]
mod tests;
