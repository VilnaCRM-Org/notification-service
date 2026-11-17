//! The GraphQL transports: `POST /api/graphql` as JSON or as an event stream, and the development
//! explorer (AD-18).
//!
//! Besides the problem-aware extractors, `execute` takes the `HeaderMap`, which is infallible: it
//! reads `Accept`, which chooses the transport, and `Last-Event-ID`, the cursor of a subscription
//! (the recorded exception of the streaming handlers, NFR-13).

use axum::Json;
use axum::extract::State;
use axum::http::header::CONTENT_SECURITY_POLICY;
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::response::{Html, IntoResponse as _, Response};
use serde_json::{Map, Value};
use shared_application::http::{ApiJson, Problem};

use crate::graphql::event_stream;
use crate::graphql::negotiation::negotiates_event_stream;
use crate::graphql::schema::ApiSchema;

/// Policy of the explorer page: the one third-party origin it loads from, and nothing else.
pub const EXPLORER_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src https://unpkg.com 'unsafe-inline'; style-src https://unpkg.com 'unsafe-inline'; img-src data:; font-src data:; connect-src 'self'; frame-ancestors 'none'";

const EXPLORER_PAGE: &str = include_str!("explorer.html");

/// The request header that carries the cursor of a subscription (FR-11).
const LAST_EVENT_ID: HeaderName = HeaderName::from_static("last-event-id");

/// Executes one GraphQL operation, answered as JSON or, when `Accept` negotiates it, as an event
/// stream (FR-2, FR-3, FR-5).
///
/// The body must be a JSON object with a string member `query`. Without an event-stream `Accept`
/// the answer is the execution result as `200 application/json`, operation errors included, as it
/// always was. With one, the operation runs as graphql-sse's distinct connections mode: the
/// event-stream head once the operation registered its subscriber or produced its first result,
/// or the 503 problem of a node that cannot take the subscriber.
///
/// # Errors
///
/// The 422 problem when the body is not a GraphQL request, on both transports. A body that is not
/// JSON, has the wrong media type or is too large is rejected by the extractor with its own
/// problem, before the transport is chosen.
pub async fn execute(
    State(schema): State<ApiSchema>,
    headers: HeaderMap,
    ApiJson(body): ApiJson<Map<String, Value>>,
) -> Result<Response, Problem> {
    let request = to_request(body).ok_or_else(|| {
        Problem::new(StatusCode::UNPROCESSABLE_ENTITY)
            .with_detail("The request body is not a GraphQL request.")
    })?;
    if negotiates_event_stream(&headers) {
        let last_event_id = headers.get(LAST_EVENT_ID).cloned();
        return Ok(event_stream::respond(&schema, request, last_event_id).await);
    }
    Ok(Json(schema.execute(request).await).into_response())
}

/// The request for a body that has a string member `query` and otherwise converts.
pub(crate) fn to_request(body: Map<String, Value>) -> Option<async_graphql::Request> {
    if !body.get("query").is_some_and(Value::is_string) {
        return None;
    }
    serde_json::from_value(Value::Object(body)).ok()
}

/// The development explorer page with its own content security policy.
pub async fn explorer() -> Response {
    (
        [(CONTENT_SECURITY_POLICY, EXPLORER_CONTENT_SECURITY_POLICY)],
        Html(EXPLORER_PAGE),
    )
        .into_response()
}

#[cfg(test)]
mod tests;
