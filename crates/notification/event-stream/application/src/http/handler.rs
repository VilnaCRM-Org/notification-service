//! REST handlers of the event stream, and the use-case calls that the GraphQL resolvers share.

use axum::Json;
use axum::extract::State;
use shared_application::http::{Problem, ValidatedQuery};

use super::EventStreamState;
use super::dto::{EventStreamStatusResponse, EventTypeResponse, ListEventTypesParams};
use crate::query::EventTypeView;

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

#[cfg(test)]
mod tests;
