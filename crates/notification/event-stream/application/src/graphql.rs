//! GraphQL adapter of the event stream: the catalog and status object types and the query root.
//!
//! The resolvers call the same use case and read the same status as the REST handlers, so both
//! transports answer the same values; every failure becomes a problem first and a GraphQL error
//! second.

use async_graphql::{Context, Object, SimpleObject};
use shared_application::graphql::problem_to_error;
use shared_application::http::Problem;

use crate::http::EventStreamState;
use crate::http::dto::{EventStreamStatusResponse, ListEventTypesParams};
use crate::http::handler::list_types;
use crate::query::EventTypeView;

/// A subscribable event type.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
#[graphql(name = "EventType")]
pub struct EventTypeObject {
    /// Name of the event type.
    pub name: String,
}

impl From<EventTypeView> for EventTypeObject {
    fn from(view: EventTypeView) -> Self {
        Self { name: view.name }
    }
}

/// The status of the event stream on the answering instance; 64-bit values are decimal strings.
#[derive(Debug, Clone, PartialEq, Eq, SimpleObject)]
#[graphql(name = "EventStreamStatus")]
pub struct EventStreamStatusObject {
    /// Life stage: starting, ready or shutting-down.
    pub state: String,
    /// Why the stream is not ready yet, only while starting.
    pub reason: Option<String>,
    /// Whether the last read of the topic watermarks succeeded.
    pub broker_reachable: bool,
    /// Id of the last event read from the topic; null before the first read.
    pub position: Option<String>,
    /// End of the topic at the last successful watermark read.
    pub end_offset: Option<String>,
    /// Events between the position and the end of the topic.
    pub lag: Option<String>,
    /// Floor of the memory window.
    pub window_first: Option<String>,
    /// Id of the newest event held in the memory window.
    pub window_last: Option<String>,
    /// Number of open subscriptions served here.
    pub subscribers: i32,
    /// Invalid records skipped by the reader.
    pub rejected_records: String,
    /// Jumps to the start of the topic because retention removed unread events.
    pub retention_losses: String,
    /// Replays that reached their cap.
    pub replay_capped: String,
}

impl From<EventStreamStatusResponse> for EventStreamStatusObject {
    /// The same values as the REST answer; a count above `i32::MAX` is reported as `i32::MAX`.
    fn from(response: EventStreamStatusResponse) -> Self {
        Self {
            state: response.state,
            reason: response.reason,
            broker_reachable: response.broker_reachable,
            position: response.position,
            end_offset: response.end_offset,
            lag: response.lag,
            window_first: response.window_first,
            window_last: response.window_last,
            subscribers: i32::try_from(response.subscribers).unwrap_or(i32::MAX),
            rejected_records: response.rejected_records,
            retention_losses: response.retention_losses,
            replay_capped: response.replay_capped,
        }
    }
}

/// The schema was built without the `EventStreamState` data.
#[derive(Debug, thiserror::Error)]
#[error("the GraphQL schema has no EventStreamState data")]
struct MissingState;

fn state<'ctx>(ctx: &Context<'ctx>) -> async_graphql::Result<&'ctx EventStreamState> {
    ctx.data::<EventStreamState>()
        .map_err(|_error| problem_to_error(Problem::internal(&MissingState)))
}

async fn types(
    state: &EventStreamState,
    params: ListEventTypesParams,
) -> Result<Vec<EventTypeObject>, Problem> {
    let views = list_types(state, params).await?;
    Ok(views.into_iter().map(EventTypeObject::from).collect())
}

/// Query root of the event stream.
#[derive(Debug, Default)]
pub struct EventStreamQuery;

#[Object]
impl EventStreamQuery {
    /// Retrieves one page of the subscribable event types, in byte order of their names.
    async fn event_types(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 1)] page: i32,
        #[graphql(default = 30)] items_per_page: i32,
    ) -> async_graphql::Result<Vec<EventTypeObject>> {
        let params = ListEventTypesParams {
            page,
            items_per_page,
        };
        types(state(ctx)?, params).await.map_err(problem_to_error)
    }

    /// Retrieves the status of the event stream on the answering instance.
    async fn event_stream_status(
        &self,
        ctx: &Context<'_>,
    ) -> async_graphql::Result<EventStreamStatusObject> {
        let snapshot = state(ctx)?.status.snapshot();
        // The `Object` macro accepts only async resolvers; the snapshot is ready at once.
        std::future::ready(Ok(EventStreamStatusResponse::from(snapshot).into())).await
    }
}

#[cfg(test)]
mod tests;
