//! REST handlers of the event stream, and the use-case calls that the GraphQL resolvers share.
//!
//! The streaming routes are not operations of the `OpenAPI` document. Besides the problem-aware
//! extractors they take the `HeaderMap` for the `Last-Event-ID` header, and the WebSocket route
//! takes the result of the upgrade extractor; neither can be rejected (the recorded exception of
//! the streaming routes).

use std::collections::BTreeSet;
use std::convert::Infallible;

use axum::Json;
use axum::extract::State;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::http::header::HeaderName;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse as _, Response};
use futures_util::stream;
use shared_application::http::{Problem, ValidatedQuery};

use super::EventStreamState;
use super::dto::{
    EventStreamStatusResponse, EventTypeResponse, ListEventTypesParams, StreamParams, WsParams,
    parse_cursor, parse_types,
};
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
