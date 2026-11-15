//! Wire shapes of the event catalog, of the status of this node and of a subscription request.
//!
//! The catalog has its response and its list parameters; a subscription request has its
//! parameters and the parsers of its event types and its cursor (FR-12, FR-16).

use std::collections::BTreeSet;

use axum::http::{HeaderMap, StatusCode};
use event_stream_domain::{Cursor, EventType, InvalidEvent};
use serde::{Deserialize, Serialize};
use shared_application::http::{Problem, Violation};
use shared_domain::PageRequest;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::http::subscription::Subscription;
use crate::query::EventTypeView;
use crate::query::list_event_types::ListEventTypes;
use crate::status::{NodeState, NodeStatus, StartReason};

/// The header that carries the cursor of a resume; it wins over [`CURSOR_PARAMETER`].
const CURSOR_HEADER: &str = "Last-Event-ID";

/// The query parameter that carries the cursor of a resume.
const CURSOR_PARAMETER: &str = "lastEventId";

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
    #[schema(required = true, example = "catching-up")]
    pub reason: Option<String>,
    /// Whether the last read of the topic watermarks succeeded.
    pub broker_reachable: bool,
    /// Id of the last event read from the topic; `null` before the first read.
    #[schema(required = true, example = "42")]
    pub position: Option<String>,
    /// End of the topic at the last successful watermark read; `null` before it.
    #[schema(required = true, example = "42")]
    pub end_offset: Option<String>,
    /// Events between the position and the end of the topic; `null` while either is unknown.
    #[schema(required = true, example = "0")]
    pub lag: Option<String>,
    /// Floor of the memory window: events after it can be resumed from memory.
    #[schema(required = true, example = "0")]
    pub window_first: Option<String>,
    /// Id of the newest event held in the memory window.
    #[schema(required = true, example = "42")]
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

/// Query parameters of the SSE route.
///
/// `types` is required: without it the query string does not deserialize, which is the 400
/// problem naming `types`. Both values are parsed by the handler, so that every fault of the types
/// is one 422 violation on `types`.
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct StreamParams {
    /// Comma-separated event type names.
    pub types: String,
    /// The cursor of a resume when the `Last-Event-ID` header is absent.
    pub last_event_id: Option<String>,
}

/// The fault of a list of event type names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypesError {
    /// No name was given.
    Empty,
    /// A name is empty.
    Blank,
    /// A name is not an event type name.
    Name(InvalidEvent),
    /// More than [`Subscription::MAX_TYPES`] distinct names.
    TooMany,
}

impl TypesError {
    /// The violation code of the fault: `required`, `length`, `pattern` or `max_items`.
    const fn code(self) -> &'static str {
        match self {
            Self::Empty | Self::Blank => "required",
            Self::Name(reason) => reason.code(),
            Self::TooMany => "max_items",
        }
    }

    /// The violation message of the fault; it never holds a name of the request.
    fn message(self) -> String {
        match self {
            Self::Empty | Self::TooMany => "types must hold 1 to 100 event type names".to_owned(),
            Self::Blank => "types must not hold an empty event type name".to_owned(),
            Self::Name(reason) => reason.to_string(),
        }
    }
}

/// The one rule for a list of names: each name in order is checked for being empty (`Blank`),
/// then for being an event type name (`Name`), then counted, and the 101st distinct name is
/// `TooMany`; the first fault met is the answer. A repeated name counts once; no name at all is
/// `Empty`.
pub(crate) fn parse_names<'a, I: Iterator<Item = &'a str>>(
    names: I,
) -> Result<BTreeSet<EventType>, TypesError> {
    let mut types = BTreeSet::new();
    for name in names {
        if name.is_empty() {
            return Err(TypesError::Blank);
        }
        types.insert(EventType::new(name.to_owned()).map_err(TypesError::Name)?);
        if types.len() > Subscription::MAX_TYPES {
            return Err(TypesError::TooMany);
        }
    }
    if types.is_empty() {
        Err(TypesError::Empty)
    } else {
        Ok(types)
    }
}

/// The names of the `types` parameter, split at every comma.
///
/// # Errors
///
/// The 422 problem with one violation on `types`.
pub(crate) fn parse_types(raw: &str) -> Result<BTreeSet<EventType>, Problem> {
    parse_names(raw.split(',')).map_err(types_problem)
}

/// The 422 problem of a fault of the types: one violation on `types`.
fn types_problem(error: TypesError) -> Problem {
    Problem::validation_failed(vec![Violation {
        property_path: "types".to_owned(),
        message: error.message(),
        code: error.code().to_owned(),
    }])
}

/// The cursor of a resume: the `Last-Event-ID` header when it is present, else the
/// `lastEventId` parameter, else none. The other carrier is not read when the header is present.
///
/// # Errors
///
/// The 400 problem naming the carrier whose value is not a decimal unsigned 64-bit integer.
pub(crate) fn parse_cursor(
    headers: &HeaderMap,
    parameter: Option<&str>,
) -> Result<Option<Cursor>, Problem> {
    match (headers.get(CURSOR_HEADER), parameter) {
        (Some(value), _) => read_cursor(value.to_str().ok(), CURSOR_HEADER).map(Some),
        (None, Some(text)) => read_cursor(Some(text), CURSOR_PARAMETER).map(Some),
        (None, None) => Ok(None),
    }
}

/// Reads the cursor of one carrier; `None` is a value that is not text.
fn read_cursor(text: Option<&str>, carrier: &str) -> Result<Cursor, Problem> {
    text.and_then(|text| Cursor::parse(text).ok())
        .ok_or_else(|| {
            Problem::new(StatusCode::BAD_REQUEST).with_detail(format!(
                "{carrier} must be a decimal unsigned 64-bit integer"
            ))
        })
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
