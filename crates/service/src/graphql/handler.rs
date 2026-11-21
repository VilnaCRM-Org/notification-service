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
//!
//! Each handler carries its operation of the `OpenAPI` document; the two GET handlers carry the
//! same one, so the document does not depend on the environment (FR-26, ADR 0022).

use axum::Json;
use axum::extract::{RawQuery, State};
use axum::http::header::{ALLOW, CONTENT_SECURITY_POLICY};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse as _, Response};
use serde_json::{Map, Value};
use shared_application::http::{ApiJson, Problem};

use crate::graphql::document::{
    CursorHeader, EXAMPLE_OPERATION, EXAMPLE_QUERY, EXAMPLE_SUBSCRIPTION, GetParams, GraphQLEvent,
    GraphQLRequest, GraphQLResponse, MEDIA_TYPE_DETAIL, MISSING_QUERY_DETAIL, NOT_JSON_DETAIL,
    TOO_LARGE_DETAIL, TOO_LONG_DETAIL, complete_event_example, next_event_example,
    not_an_object_example, problem_example, query_result_example, unavailable_example,
};
use crate::graphql::negotiation::{negotiates_event_stream, negotiates_event_stream_on_get};
use crate::graphql::schema::ApiSchema;
use crate::graphql::{event_stream, query_string};
use crate::http::router::GRAPHQL_PATH;

/// Policy of the explorer page: the one third-party origin it loads from, and nothing else.
pub const EXPLORER_CONTENT_SECURITY_POLICY: &str = "default-src 'none'; script-src https://unpkg.com 'unsafe-inline'; style-src https://unpkg.com 'unsafe-inline'; img-src data:; font-src data:; connect-src 'self'; frame-ancestors 'none'";

const EXPLORER_PAGE: &str = include_str!("explorer.html");

/// The request header that carries the cursor of a subscription (FR-11).
const LAST_EVENT_ID: HeaderName = HeaderName::from_static("last-event-id");

/// The description of `POST /api/graphql`.
pub(crate) const POST_TEXT: &str = "Executes one GraphQL operation. Without an `Accept` that \
    ranks `text/event-stream` above both JSON types the answer is the execution result as \
    `application/json`, as it always was. With one, the operation runs as graphql-sse's distinct \
    connections mode: the subscription `events` delivers one `next` block per event, and a query \
    one `next` block, then `complete`. The transport problems come before any stream; document \
    errors, a breach of the one-root-field rule and invalid arguments are results. \
    docs/streaming-protocol.md describes the event stream.";

/// The description of `GET /api/graphql`.
pub(crate) const GET_TEXT: &str = "Runs the GraphQL operation of the query string as \
    graphql-sse's distinct connections mode, for clients such as a browser `EventSource` that \
    can only send a GET. The event stream is chosen when an `Accept` field admits \
    `text/event-stream`, also through `*/*` or `text/*`, ranked above `text/html` and both JSON \
    types. A GET without `Accept`, or one that ranks `text/html` or a JSON type at least as high, \
    gets the explorer page in `dev` and the 405 problem elsewhere. The query-string problems come \
    before any stream. docs/streaming-protocol.md describes the event stream.";

/// The description of the 200 answer of `POST /api/graphql`.
const POST_OK_TEXT: &str = "The execution result as `application/json`, operation errors \
    included; or, when `Accept` negotiates it, the event stream: the comment line `:` first, \
    then one `next` block per execution result, each subscription result with the event id; \
    `complete` follows a single result only. A subscription stream stays open, writes the \
    comment whenever `STREAM_KEEP_ALIVE` passes without a block, and ends without `complete` \
    when the node shuts down or a replay cannot read Kafka. The schema of the event stream \
    describes one event.";

/// The description of the 200 answer of `GET /api/graphql`.
const GET_OK_TEXT: &str = "The event stream, as on POST: the comment line `:` first, then one \
    `next` block per execution result, each subscription result with the event id; `complete` \
    follows a single result only. Its schema describes one event. In `dev` only, a GET that does \
    not negotiate the event stream gets the explorer page as `text/html`.";

/// The description of the 400 problem of `GET /api/graphql`.
const GET_BAD_REQUEST_TEXT: &str = "The query string cannot be read: `query` is missing, a known \
    parameter is given twice, or a name or a value is not percent-encoded UTF-8. The detail names \
    only the four known parameters.";

/// The example of the explorer page, the `text/html` answer of a GET in `dev`.
const EXPLORER_EXAMPLE: &str = "<!doctype html>";

/// The description of the 503 problem of both `GraphQL` operations.
const UNAVAILABLE_TEXT: &str = "A subscription the node cannot take, because it is shutting down, \
    starting or at its subscriber capacity, in this precedence; or a request that exceeded its \
    time budget before the head. Safe to retry on any node.";

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
#[utoipa::path(
    post,
    path = GRAPHQL_PATH,
    operation_id = "api_graphql_post",
    tag = "GraphQL",
    summary = "Executes a GraphQL operation, answered as JSON or as an event stream.",
    description = POST_TEXT,
    params(CursorHeader),
    request_body(
        content = GraphQLRequest,
        content_type = "application/json",
        description = "One GraphQL operation.",
        examples(
            ("query" = (summary = "The catalog query", value = json!({"query": EXAMPLE_QUERY}))),
            ("subscription" = (summary = "The subscription to load.test",
                value = json!({"query": EXAMPLE_SUBSCRIPTION, "operationName": EXAMPLE_OPERATION}))),
        )
    ),
    responses(
        (status = 200, description = POST_OK_TEXT, content(
            (GraphQLResponse = "application/json", example = json!(query_result_example())),
            (GraphQLEvent = "text/event-stream", examples(
                ("next" = (summary = "A subscription result", value = json!(next_event_example()))),
                ("complete" = (summary = "The end of a single result", value = json!(complete_event_example()))),
            )),
        )),
        (status = 400, description = "The body is not JSON.", body = Problem, content_type = "application/problem+json",
            example = json!(problem_example(StatusCode::BAD_REQUEST, NOT_JSON_DETAIL))),
        (status = 413, description = "The body is larger than `HTTP_MAX_BODY_BYTES`.", body = Problem, content_type = "application/problem+json",
            example = json!(problem_example(StatusCode::PAYLOAD_TOO_LARGE, TOO_LARGE_DETAIL))),
        (status = 415, description = "The body is not `application/json`.", body = Problem, content_type = "application/problem+json",
            example = json!(problem_example(StatusCode::UNSUPPORTED_MEDIA_TYPE, MEDIA_TYPE_DETAIL))),
        (status = 422, description = "The body is JSON but not a GraphQL request: not an object, without a string `query`, or with a member of the wrong type.",
            body = Problem, content_type = "application/problem+json", example = json!(not_a_graphql_request())),
        (status = 503, description = UNAVAILABLE_TEXT, body = Problem, content_type = "application/problem+json",
            example = json!(unavailable_example())),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
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
#[utoipa::path(
    get,
    path = GRAPHQL_PATH,
    operation_id = "api_graphql_get",
    tag = "GraphQL",
    summary = "Runs the GraphQL operation of the query string as an event stream.",
    description = GET_TEXT,
    params(GetParams, CursorHeader),
    responses(
        (status = 200, description = GET_OK_TEXT, content(
            (GraphQLEvent = "text/event-stream", examples(
                ("next" = (summary = "A subscription result", value = json!(next_event_example()))),
                ("complete" = (summary = "The end of a single result", value = json!(complete_event_example()))),
            )),
            (String = "text/html", example = json!(EXPLORER_EXAMPLE)),
        )),
        (status = 400, description = GET_BAD_REQUEST_TEXT, body = Problem, content_type = "application/problem+json",
            example = json!(problem_example(StatusCode::BAD_REQUEST, MISSING_QUERY_DETAIL))),
        (status = 405, description = "A GET that does not negotiate the event stream, outside `dev`.", body = Problem, content_type = "application/problem+json",
            headers(("Allow" = String, description = "`POST`.")),
            example = json!(Problem::new(StatusCode::METHOD_NOT_ALLOWED))),
        (status = 414, description = "The query string is longer than `HTTP_MAX_BODY_BYTES` as received.", body = Problem, content_type = "application/problem+json",
            example = json!(problem_example(StatusCode::URI_TOO_LONG, TOO_LONG_DETAIL))),
        (status = 422, description = "`variables` or `extensions` is not a JSON-encoded object: one `format` violation on that parameter.",
            body = Problem, content_type = "application/problem+json", example = json!(not_an_object_example())),
        (status = 503, description = UNAVAILABLE_TEXT, body = Problem, content_type = "application/problem+json",
            example = json!(unavailable_example())),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
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
#[utoipa::path(
    get,
    path = GRAPHQL_PATH,
    operation_id = "api_graphql_get",
    tag = "GraphQL",
    summary = "Runs the GraphQL operation of the query string as an event stream.",
    description = GET_TEXT,
    params(GetParams, CursorHeader),
    responses(
        (status = 200, description = GET_OK_TEXT, content(
            (GraphQLEvent = "text/event-stream", examples(
                ("next" = (summary = "A subscription result", value = json!(next_event_example()))),
                ("complete" = (summary = "The end of a single result", value = json!(complete_event_example()))),
            )),
            (String = "text/html", example = json!(EXPLORER_EXAMPLE)),
        )),
        (status = 400, description = GET_BAD_REQUEST_TEXT, body = Problem, content_type = "application/problem+json",
            example = json!(problem_example(StatusCode::BAD_REQUEST, MISSING_QUERY_DETAIL))),
        (status = 405, description = "A GET that does not negotiate the event stream, outside `dev`.", body = Problem, content_type = "application/problem+json",
            headers(("Allow" = String, description = "`POST`.")),
            example = json!(Problem::new(StatusCode::METHOD_NOT_ALLOWED))),
        (status = 414, description = "The query string is longer than `HTTP_MAX_BODY_BYTES` as received.", body = Problem, content_type = "application/problem+json",
            example = json!(problem_example(StatusCode::URI_TOO_LONG, TOO_LONG_DETAIL))),
        (status = 422, description = "`variables` or `extensions` is not a JSON-encoded object: one `format` violation on that parameter.",
            body = Problem, content_type = "application/problem+json", example = json!(not_an_object_example())),
        (status = 503, description = UNAVAILABLE_TEXT, body = Problem, content_type = "application/problem+json",
            example = json!(unavailable_example())),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
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
