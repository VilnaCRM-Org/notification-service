//! The request span and the access log (AD-13).
//!
//! `make_span` opens the span `http.request`; `on_response` writes the one `request completed`
//! line of the request. Both are plugged into `TraceLayer` by the router.

use std::time::Duration;

use axum::extract::{MatchedPath, Request};
use axum::http::Response;
use tracing::Span;

use crate::http::request_id::REQUEST_ID_HEADER;

/// Route recorded for a request that matched no route.
const UNMATCHED: &str = "unmatched";

/// Opens the span `http.request` with the method, the route template (or `unmatched`), the path
/// and the request id.
///
/// No other header is recorded.
#[must_use]
pub fn make_span(request: &Request) -> Span {
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map_or(UNMATCHED, MatchedPath::as_str);
    let request_id = request
        .headers()
        .get(&REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let method = request.method().as_str();
    let path = request.uri().path();
    tracing::info_span!(
        "http.request",
        http.request.method = method,
        http.route = route,
        url.path = path,
        request_id,
    )
}

/// Writes the access-log line `request completed` at `INFO`.
///
/// Generic over the body type: with a concrete body the middleware stack does not compile.
pub fn on_response<B>(response: &Response<B>, latency: Duration, _span: &Span) {
    let status = response.status().as_u16();
    let duration_ms = u64::try_from(latency.as_millis()).unwrap_or(u64::MAX);
    log_request_completed(status, duration_ms);
}

/// The access-log event. The explicit target is the module path and keeps the call on one line.
fn log_request_completed(status: u16, duration_ms: u64) {
    tracing::info!(target: module_path!(), { http.response.status_code = status, duration_ms }, "request completed");
}

#[cfg(test)]
mod tests;
