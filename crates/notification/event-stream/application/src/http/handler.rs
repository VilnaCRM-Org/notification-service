//! REST handlers of the event stream, and the use-case calls that the GraphQL resolvers share.
//!
//! The streaming route is not an operation of the `OpenAPI` document. Besides the problem-aware
//! extractors it takes the `HeaderMap`, which cannot be rejected, for the `Last-Event-ID` header.

use std::convert::Infallible;

use axum::Json;
use axum::extract::State;
use axum::http::header::HeaderName;
use axum::http::{HeaderMap, HeaderValue};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse as _, Response};
use futures_util::stream;
use shared_application::http::{Problem, ValidatedQuery};

use super::EventStreamState;
use super::dto::{
    EventStreamStatusResponse, EventTypeResponse, ListEventTypesParams, StreamParams, parse_cursor,
    parse_types,
};
use super::subscription::{Subscription, SubscriptionRequest};
use crate::query::EventTypeView;

/// The response header that keeps a proxy from holding frames and keep-alives back (FR-5).
const X_ACCEL_BUFFERING: HeaderName = HeaderName::from_static("x-accel-buffering");

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
        log_replay_failed(error);
    }
    None
}

fn log_replay_failed(error: &str) {
    tracing::warn!(transport = "sse", error = %error, "replay failed");
}

#[cfg(test)]
mod tests;
