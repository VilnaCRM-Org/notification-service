//! The GraphQL transports: `POST /api/graphql` as JSON or as an event stream, `GET /api/graphql` as
//! an event stream for `EventSource` clients, and the development explorer (AD-18).
//!
//! Besides the problem-aware extractors, the three handlers that can answer an event stream take
//! the `HeaderMap`, which is infallible: it holds `Accept`, which chooses the transport, and
//! `Last-Event-ID`, the cursor of a subscription. The two GET handlers also take the `RawQuery`,
//! also infallible: only the query string as received can be bounded and read strictly, and a
//! problem-aware query extractor would refuse a malformed query string before the handler could
//! keep today's answer of a GET that does not negotiate the event stream (the recorded exceptions
//! of the streaming handlers, NFR-13).

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::http::header::{ALLOW, CONTENT_SECURITY_POLICY};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse as _, Response};
use serde_json::{Map, Value};
use shared_application::http::{ApiJson, Problem};

use crate::graphql::negotiation::{negotiates_event_stream, negotiates_event_stream_on_get};
use crate::graphql::schema::ApiSchema;
use crate::graphql::{event_stream, query_string};

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
    let request = to_request(body).ok_or_else(not_a_graphql_request)?;
    if negotiates_event_stream(&headers) {
        return Ok(event_stream::respond(&schema, request, last_event_id(&headers)).await);
    }
    Ok(Json(schema.execute(request).await).into_response())
}

/// Answers a GET outside `dev`: the event stream when `Accept` admits it (FR-2, GET rule), else the
/// 405 problem with `Allow: POST`, the answer of today (FR-4, FR-5).
///
/// With the event stream, the operation is the one of the query string (`query`, `operationName`,
/// `variables`, `extensions`), refused before any stream with the 414, 400 or 422 problem of
/// FR-4, and the `Last-Event-ID` header is the cursor of a subscription, as on POST.
pub async fn subscribe(
    State(schema): State<ApiSchema>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    if negotiates_event_stream_on_get(&headers) {
        return stream_query_string(&schema, &headers, query.as_deref()).await;
    }
    (
        [(ALLOW, HeaderValue::from_static("POST"))],
        Problem::new(StatusCode::METHOD_NOT_ALLOWED),
    )
        .into_response()
}

/// Answers a GET in `dev`: the event stream when `Accept` admits it, as [`subscribe`] does, else
/// the explorer page.
pub async fn subscribe_or_explore(
    State(schema): State<ApiSchema>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Response {
    if negotiates_event_stream_on_get(&headers) {
        return stream_query_string(&schema, &headers, query.as_deref()).await;
    }
    explorer()
}

/// The event stream of the operation in the query string `query` of a GET that negotiated it, or
/// the problem that refuses the query string: the 500 problem of a schema without the
/// `QueryStringLimit`, then the 414, 400 and 422 problems of `query_string::read` (FR-4, FR-6).
async fn stream_query_string(
    schema: &ApiSchema,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Response {
    match get_request(schema, query) {
        Ok(request) => event_stream::respond(schema, request, last_event_id(headers)).await,
        Err(problem) => problem.into_response(),
    }
}

/// The request of the query string `query`, built by the same [`to_request`] as a POST body.
fn get_request(schema: &ApiSchema, query: Option<&str>) -> Result<async_graphql::Request, Problem> {
    let limit = query_string::limit(schema)?;
    let body = query_string::read(query, limit.max_bytes)?;
    to_request(body).ok_or_else(not_a_graphql_request)
}

/// The cursor a request carries in its `Last-Event-ID` header (FR-11).
fn last_event_id(headers: &HeaderMap) -> Option<HeaderValue> {
    headers.get(LAST_EVENT_ID).cloned()
}

/// The 422 problem of a body that is not a GraphQL request.
fn not_a_graphql_request() -> Problem {
    Problem::new(StatusCode::UNPROCESSABLE_ENTITY)
        .with_detail("The request body is not a GraphQL request.")
}

/// The request for a body that has a string member `query` and otherwise converts.
pub(crate) fn to_request(body: Map<String, Value>) -> Option<async_graphql::Request> {
    if !body.get("query").is_some_and(Value::is_string) {
        return None;
    }
    serde_json::from_value(Value::Object(body)).ok()
}

/// The development explorer page with its own content security policy, the answer of a GET in
/// `dev` that does not negotiate the event stream.
fn explorer() -> Response {
    (
        [(CONTENT_SECURITY_POLICY, EXPLORER_CONTENT_SECURITY_POLICY)],
        Html(EXPLORER_PAGE),
    )
        .into_response()
}

#[cfg(test)]
mod tests;
