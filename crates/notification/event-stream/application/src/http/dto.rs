//! Wire shapes of the event catalog (the response and the list parameters) and of the status of
//! this node.

use serde::{Deserialize, Serialize};
use shared_domain::PageRequest;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::query::EventTypeView;
use crate::query::list_event_types::ListEventTypes;
use crate::status::{NodeState, NodeStatus, StartReason};

/// The defaults of the shared kernel, signed like the parameters; both fit in an `i32`.
const DEFAULT_PAGE: i32 = PageRequest::DEFAULT_PAGE.cast_signed();
const DEFAULT_ITEMS_PER_PAGE: i32 = PageRequest::DEFAULT_ITEMS_PER_PAGE.cast_signed();

/// A subscribable event type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[schema(as = EventType)]
pub struct EventTypeResponse {
    /// Name of the event type.
    #[schema(
        min_length = 1,
        max_length = 128,
        pattern = r"^[a-z][a-z0-9]*([._-][a-z0-9]+)*$",
        example = "user.created"
    )]
    pub name: String,
}

impl From<EventTypeView> for EventTypeResponse {
    fn from(view: EventTypeView) -> Self {
        Self { name: view.name }
    }
}

/// Query parameters of the catalog; signed so that a negative value is a 422, not a parse error.
/// The bounds are checked by `PageRequest`, which reports the first violated one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, IntoParams, Validate)]
#[serde(rename_all = "camelCase", default)]
#[into_params(parameter_in = Query)]
pub struct ListEventTypesParams {
    /// Page number, starting at 1.
    #[param(minimum = 1, default = 1)]
    pub page: i32,
    /// Number of items per page.
    #[param(minimum = 1, maximum = 100, default = 30)]
    pub items_per_page: i32,
}

impl Default for ListEventTypesParams {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            items_per_page: DEFAULT_ITEMS_PER_PAGE,
        }
    }
}

impl ListEventTypesParams {
    /// The query for these parameters; a value below 1 becomes 0, which `PageRequest` refuses.
    #[must_use]
    pub fn into_query(self) -> ListEventTypes {
        ListEventTypes {
            page: u32::try_from(self.page).unwrap_or(0),
            items_per_page: u32::try_from(self.items_per_page).unwrap_or(0),
        }
    }
}

/// The status of the event stream on the answering instance: its reader, its memory window and
/// its subscribers. The 64-bit counters and positions are decimal strings; an unknown value is
/// `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as = EventStreamStatus)]
pub struct EventStreamStatusResponse {
    /// Life stage: `starting`, `ready` or `shutting-down`.
    #[schema(example = "ready")]
    pub state: String,
    /// Why the service is not ready yet, only while starting: `broker-unreachable`,
    /// `topic-not-found`, `partition-count` or `catching-up`.
    #[schema(example = "catching-up")]
    pub reason: Option<String>,
    /// Whether the last read of the topic watermarks succeeded.
    pub broker_reachable: bool,
    /// Id of the last event read from the topic; `null` before the first read.
    #[schema(example = "42")]
    pub position: Option<String>,
    /// End of the topic at the last successful watermark read; `null` before it.
    #[schema(example = "42")]
    pub end_offset: Option<String>,
    /// Events between the position and the end of the topic; `null` while either is unknown.
    #[schema(example = "0")]
    pub lag: Option<String>,
    /// Floor of the memory window: events after it can be resumed from memory.
    #[schema(example = "0")]
    pub window_first: Option<String>,
    /// Id of the newest event held in the memory window.
    #[schema(example = "42")]
    pub window_last: Option<String>,
    /// Number of open subscriptions served here.
    #[schema(example = 0)]
    pub subscribers: usize,
    /// Invalid records skipped by the reader.
    #[schema(example = "0")]
    pub rejected_records: String,
    /// Jumps to the start of the topic because retention removed unread events.
    #[schema(example = "0")]
    pub retention_losses: String,
    /// Replays that reached their cap.
    #[schema(example = "0")]
    pub replay_capped: String,
}

impl From<NodeStatus> for EventStreamStatusResponse {
    fn from(status: NodeStatus) -> Self {
        Self {
            state: state_name(status.state).to_owned(),
            reason: status.reason.map(|reason| reason_name(reason).to_owned()),
            broker_reachable: status.broker_reachable,
            position: status.position.map(|cursor| cursor.to_string()),
            end_offset: status.end_offset.map(|cursor| cursor.to_string()),
            lag: status.lag.map(|lag| lag.to_string()),
            window_first: status.window_first.map(|cursor| cursor.to_string()),
            window_last: status.window_last.map(|sequence| sequence.to_string()),
            subscribers: status.subscribers,
            rejected_records: status.rejected_records.to_string(),
            retention_losses: status.retention_losses.to_string(),
            replay_capped: status.replay_capped.to_string(),
        }
    }
}

/// The wire name of a life stage.
const fn state_name(state: NodeState) -> &'static str {
    match state {
        NodeState::Starting => "starting",
        NodeState::Ready => "ready",
        NodeState::ShuttingDown => "shutting-down",
    }
}

/// The wire name of a start reason.
const fn reason_name(reason: StartReason) -> &'static str {
    match reason {
        StartReason::BrokerUnreachable => "broker-unreachable",
        StartReason::TopicNotFound => "topic-not-found",
        StartReason::PartitionCount => "partition-count",
        StartReason::CatchingUp => "catching-up",
    }
}

#[cfg(test)]
mod tests;
