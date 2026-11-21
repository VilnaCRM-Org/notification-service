//! REST handlers of the event stream, and the use-case calls that the GraphQL resolvers share.
//!
//! Besides the problem-aware extractors the streaming routes take the `HeaderMap` for the
//! `Last-Event-ID` header, and the WebSocket route takes the result of the upgrade extractor;
//! neither can be rejected (the recorded exception of the streaming routes).

use std::collections::BTreeSet;
use std::convert::Infallible;

use axum::Json;
use axum::extract::State;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::ws::rejection::{
    ConnectionNotUpgradable, InvalidConnectionHeader, WebSocketUpgradeRejection,
};
use axum::http::header::HeaderName;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse as _, Response};
use futures_util::stream;
use shared_application::http::{Problem, ValidatedQuery};

use super::EventStreamState;
use super::dto::{
    CursorFault, CursorHeader, EXAMPLE_CURSOR, EXAMPLE_EVENT, EXAMPLE_TYPE,
    EventStreamStatusResponse, EventTypeResponse, HandshakeHeaders, ListEventTypesParams, SseFrame,
    StreamParams, TypesError, WsParams, parse_cursor, parse_types, types_problem,
};
use super::hub::HubError;
use super::session::Session;
use super::socket;
use super::subscription::{Subscription, SubscriptionRequest, log_replay_failed};
use crate::query::EventTypeView;

/// The response header that keeps a proxy from holding frames and keep-alives back (FR-5).
const X_ACCEL_BUFFERING: HeaderName = HeaderName::from_static("x-accel-buffering");

/// The largest inbound WebSocket message and the largest inbound frame, in bytes (FR-11, NFR-9);
/// a frame whose header declares more ends the connection before its payload is buffered.
pub(crate) const MAX_MESSAGE_BYTES: usize = 32_768;

/// The read buffer of a WebSocket connection, in bytes (NFR-9).
pub(crate) const READ_BUFFER_BYTES: usize = 4_096;

/// The description of the event stream of the SSE route.
const STREAM_OPENED: &str = "The event stream: one frame per event of the listed types, in \
    strictly increasing id order with gaps, and the comment line `:` followed by an empty line \
    when no frame was written for `STREAM_KEEP_ALIVE`. The schema describes one event. The \
    response stays open; it ends without a final frame when the node shuts down or a replay \
    below the memory window cannot read Kafka, and the client then reconnects with its last id.";

/// The description of the 400 problem of the SSE route.
const STREAM_BAD_REQUEST: &str = "The query string cannot be read (`types` missing, or a \
    parameter given twice), or the cursor is not a decimal unsigned 64-bit integer; the detail \
    names the parameter or the carrier of the cursor.";

/// The description of the 400 problem of the WebSocket route.
const WS_BAD_REQUEST: &str = "The query string cannot be read (a parameter given twice), the \
    cursor is not a decimal unsigned 64-bit integer, or the request is not a valid upgrade: a \
    handshake header is missing or holds a value the upgrade refuses. The detail names the \
    parameter, the carrier of the cursor or the header.";

/// The description of the 422 problem of both streaming routes.
const TYPES_INVALID: &str = "`types` is empty or holds an empty name (`required`), a name that \
    is not an event type name (`length` or `pattern`), or more than 100 distinct names \
    (`max_items`): one violation on `types`, which never echoes a name of the request.";

/// The description of the 503 problem of both streaming routes.
const NODE_UNAVAILABLE: &str = "The node is shutting down, starting, or at its subscriber \
    capacity, in this precedence; safe to retry on any node.";

/// The description of the 403 problem of the WebSocket route.
const ORIGIN_REFUSED: &str = "The handshake carries an `Origin` that `CORS_ALLOWED_ORIGINS` \
    does not allow; no subscriber is registered.";

/// The description of the 426 problem of the WebSocket route.
const NOT_UPGRADABLE_TEXT: &str =
    "A valid handshake on a connection that cannot be upgraded; no subscriber stays registered.";

/// The description of the 101 answer of the WebSocket route: the frames of both directions.
const WS_UPGRADED: &str = r#"Switching Protocols: the connection is a WebSocket.

**Server frames.** One text frame per event of the connection's types, in strictly increasing id order: `{"id":"<event id>","type":"<event type>","data":<event object>}`, the id a decimal string and the event object that of the SSE frame. For example:

`{"id":"1043","type":"load.test","data":{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"load.test","occurredAt":"2026-11-14T10:00:00Z","data":{"run":1}}}`

An invalid command is answered with one error frame `{"error":"<reason>"}` and changes nothing, for example `{"error":"invalid command"}`. The server sends a ping every `STREAM_KEEP_ALIVE`; a frame larger than 32 768 bytes ends the connection.

**Client commands.** Text frames of at most 16 384 bytes that add or remove types, each with 1 to 100 distinct names; a valid command gets no answer. For example:

`{"action":"subscribe","types":["load.test"]}`

`{"action":"unsubscribe","types":["load.test"]}`"#;

/// Reads one page of the catalog for the parameters.
pub(crate) async fn list_types(
    state: &EventStreamState,
    params: ListEventTypesParams,
) -> Result<Vec<EventTypeView>, Problem> {
    Ok(state.query_bus.ask(params.into_query()).await?)
}

/// Retrieves the collection of `EventType` resources.
///
/// # Errors
///
/// The 422 problem for a page or a page size outside its bounds.
#[utoipa::path(
    get,
    path = "/event-types",
    operation_id = "api_event_types_get",
    tag = "EventStream",
    summary = "Retrieves the collection of EventType resources.",
    description = "Returns one page of the subscribable event types, in byte order of their names.",
    params(ListEventTypesParams),
    responses(
        (status = 200, description = "EventType collection", body = Vec<EventTypeResponse>),
        (status = 400, description = "Invalid query parameter.", body = Problem, content_type = "application/problem+json"),
        (status = 422, description = "Validation failed.", body = Problem, content_type = "application/problem+json"),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn list_event_types(
    State(state): State<EventStreamState>,
    ValidatedQuery(params): ValidatedQuery<ListEventTypesParams>,
) -> Result<Json<Vec<EventTypeResponse>>, Problem> {
    let views = list_types(&state, params).await?;
    Ok(Json(
        views.into_iter().map(EventTypeResponse::from).collect(),
    ))
}

/// Retrieves the status of the event stream on the answering instance.
#[utoipa::path(
    get,
    path = "/events/status",
    operation_id = "api_events_status_get",
    tag = "EventStream",
    summary = "Retrieves the event stream status.",
    description = "Returns how far the answering instance has read the event topic, its memory window and its subscribers. It reads only the state the instance holds and answers in every life stage.",
    responses(
        (status = 200, description = "EventStreamStatus resource", body = EventStreamStatusResponse),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn event_stream_status(
    State(state): State<EventStreamState>,
) -> Json<EventStreamStatusResponse> {
    Json(state.status.snapshot().into())
}

/// Opens a `text/event-stream` subscription of the listed event types (FR-5 to FR-7).
///
/// It resumes from the cursor of the `Last-Event-ID` header or the `lastEventId` parameter
/// (FR-12). The types are checked first, then the cursor, then the hub takes the subscriber
/// (FR-16, FR-25); no answer depends on a broker read.
///
/// # Errors
///
/// The 400 problem of a missing `types` or an invalid cursor, the 422 problem of invalid types,
/// then the 503 problem of a node that is shutting down, starting or at its cap.
#[utoipa::path(
    get,
    path = "/events/stream",
    operation_id = "api_events_stream_get",
    tag = "EventStream",
    summary = "Opens an event stream of the listed event types.",
    description = "Opens a Server-Sent Events stream that delivers every event of the listed types as one frame, in increasing id order: after the cursor of `Last-Event-ID` or `lastEventId` when one is given, live otherwise. The checks run in this order: the query string (400), the types (422), the cursor (400), the node (503); no answer depends on a broker read. docs/streaming-protocol.md describes the protocol.",
    params(StreamParams, CursorHeader),
    responses(
        (status = 200, description = STREAM_OPENED, body = SseFrame, content_type = "text/event-stream",
            headers(
                ("cache-control" = String, description = "Always `no-cache`."),
                ("x-accel-buffering" = String, description = "Always `no`, so that a proxy does not hold frames and keep-alives back."),
            ),
            example = json!({"event": EXAMPLE_TYPE, "id": EXAMPLE_CURSOR, "data": EXAMPLE_EVENT})),
        (status = 400, description = STREAM_BAD_REQUEST, body = Problem, content_type = "application/problem+json",
            example = json!(CursorFault::PARAMETER.bad_request())),
        (status = 422, description = TYPES_INVALID, body = Problem, content_type = "application/problem+json",
            example = json!(types_problem(TypesError::Blank))),
        (status = 503, description = NODE_UNAVAILABLE, body = Problem, content_type = "application/problem+json",
            example = json!(starting_problem())),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn stream(
    State(state): State<EventStreamState>,
    headers: HeaderMap,
    ValidatedQuery(params): ValidatedQuery<StreamParams>,
) -> Result<Response, Problem> {
    let types = parse_types(&params.types)?;
    let cursor = parse_cursor(&headers, params.last_event_id.as_deref())?;
    let subscription = state
        .subscriptions
        .open(SubscriptionRequest { types, cursor })?;
    let events = Sse::new(stream::unfold(subscription, next_event))
        .keep_alive(KeepAlive::new().interval(state.settings.keep_alive));
    Ok((
        [(X_ACCEL_BUFFERING, HeaderValue::from_static("no"))],
        events,
    )
        .into_response())
}

/// Upgrades to a WebSocket subscription of the listed event types (FR-8 to FR-11).
///
/// `types` is optional: without it the connection starts with no type and subscribes with a
/// command. The checks run in the order of FR-16: the types and the cursor (400 or 422), then the
/// hub takes the subscriber (503), then the upgrade itself; no socket is opened before all of them
/// passed. Each event arrives as one text frame, the client changes its types at run time, a ping
/// is written every `STREAM_KEEP_ALIVE`, which is also the deadline of every write.
///
/// # Errors
///
/// The 400 problem of an invalid cursor, the 422 problem of invalid types, the 503 problem of a
/// node that is shutting down, starting or at its cap, then the 4xx problem of a request that is
/// not a valid upgrade.
#[utoipa::path(
    get,
    path = "/events/ws",
    operation_id = "api_events_ws_get",
    tag = "EventStream",
    summary = "Upgrades to a WebSocket subscription of the listed event types.",
    description = "Upgrades the connection to a WebSocket that delivers every event of its types as one text frame, in increasing id order; the client adds and removes types at run time with commands. The checks run in this order: an `Origin` outside `CORS_ALLOWED_ORIGINS` (403), the query string (400), the types (422), the cursor (400), the node (503), then the upgrade (400 or 426). docs/streaming-protocol.md describes the protocol.",
    params(WsParams, HandshakeHeaders, CursorHeader),
    responses(
        (status = 101, description = WS_UPGRADED,
            headers(
                ("Upgrade" = String, description = "`websocket`."),
                ("Connection" = String, description = "`upgrade`."),
                ("Sec-WebSocket-Accept" = String, description = "The key derived from `Sec-WebSocket-Key` as RFC 6455 states."),
            )),
        (status = 400, description = WS_BAD_REQUEST, body = Problem, content_type = "application/problem+json",
            example = json!(upgrade_problem(&InvalidConnectionHeader::default().into()))),
        (status = 403, description = ORIGIN_REFUSED, body = Problem, content_type = "application/problem+json",
            example = json!({"type": "about:blank", "title": "Forbidden", "status": 403, "detail": "the origin is not allowed"})),
        (status = 422, description = TYPES_INVALID, body = Problem, content_type = "application/problem+json",
            example = json!(types_problem(TypesError::Blank))),
        (status = 426, description = NOT_UPGRADABLE_TEXT, body = Problem, content_type = "application/problem+json",
            example = json!(upgrade_problem(&ConnectionNotUpgradable::default().into()))),
        (status = 503, description = NODE_UNAVAILABLE, body = Problem, content_type = "application/problem+json",
            example = json!(starting_problem())),
        (status = "default", description = "Problem response.", body = Problem, content_type = "application/problem+json"),
    )
)]
pub async fn ws(
    State(state): State<EventStreamState>,
    headers: HeaderMap,
    ValidatedQuery(params): ValidatedQuery<WsParams>,
    upgrade: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Result<Response, Problem> {
    let types = params
        .types
        .as_deref()
        .map_or_else(|| Ok(BTreeSet::new()), parse_types)?;
    let cursor = parse_cursor(&headers, params.last_event_id.as_deref())?;
    let subscription = state
        .subscriptions
        .open(SubscriptionRequest { types, cursor })?;
    let upgrade = upgrade.map_err(|rejection| upgrade_problem(&rejection))?;
    let keep_alive = state.settings.keep_alive;
    Ok(upgrade
        .max_message_size(MAX_MESSAGE_BYTES)
        .max_frame_size(MAX_MESSAGE_BYTES)
        .read_buffer_size(READ_BUFFER_BYTES)
        .on_upgrade(move |connection| {
            socket::serve(connection, Session::new(subscription), keep_alive)
        }))
}

/// The problem of a request that is not a valid WebSocket upgrade: the status and the text of the
/// rejection, which are all 4xx.
fn upgrade_problem(rejection: &WebSocketUpgradeRejection) -> Problem {
    Problem::new(rejection.status()).with_detail(rejection.body_text())
}

/// The 503 problem of a node that is starting, the example of both streaming routes: the answer to
/// [`HubError::NotReady`], built without the log entry of a refused subscriber.
fn starting_problem() -> Problem {
    Problem::new(StatusCode::SERVICE_UNAVAILABLE).with_detail(HubError::NotReady.to_string())
}

/// The next SSE event of the subscription; `None` ends the response without a final frame, at
/// shutdown after the queued frames and when a replay cannot read Kafka, which writes
/// `replay failed` with the chain of [`Subscription::failure`]; a closed hub ends silently.
async fn next_event(
    mut subscription: Subscription,
) -> Option<(Result<Event, Infallible>, Subscription)> {
    if let Some(frame) = subscription.next().await {
        return Some((Ok(frame.sse.clone()), subscription));
    }
    if let Some(error) = subscription.failure() {
        log_replay_failed("sse", error);
    }
    None
}

#[cfg(test)]
mod tests;
