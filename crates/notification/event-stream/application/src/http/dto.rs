//! Wire shapes of the event catalog, of the status of this node, of a subscription request and of
//! the commands of a WebSocket.
//!
//! The catalog has its response and its list parameters; a subscription request has the
//! parameters of each streaming route and the parsers of its event types and its cursor (FR-12,
//! FR-16); a WebSocket command adds or removes event types at run time (FR-10). The parameters,
//! the headers and the frame of the streaming routes are documented here as well (FR-26).

use std::collections::BTreeSet;

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use event_stream_domain::{Cursor, EventType, InvalidEvent};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared_application::http::{Problem, Violation};
use shared_domain::PageRequest;
use utoipa::openapi::Required;
use utoipa::openapi::path::{Parameter, ParameterBuilder, ParameterIn, ParameterStyle};
use utoipa::openapi::schema::{
    AdditionalProperties, ArrayBuilder, KnownFormat, Object, ObjectBuilder, Schema, SchemaFormat,
    SchemaType, Type,
};
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

use crate::http::subscription::Subscription;
use crate::query::EventTypeView;
use crate::query::list_event_types::ListEventTypes;
use crate::status::{NodeState, NodeStatus, StartReason};

/// The header that carries the cursor of a resume; it wins over a valid [`CURSOR_PARAMETER`].
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
/// is one 422 violation on `types`. The document describes them by [`IntoParams`].
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct StreamParams {
    /// Comma-separated event type names.
    pub types: String,
    /// The cursor of a resume; checked also beside the `Last-Event-ID` header, which wins.
    pub last_event_id: Option<String>,
}

impl IntoParams for StreamParams {
    /// `types`, required, then `lastEventId`.
    fn into_params(_parameter_in: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        vec![
            types_parameter(Required::True, STREAM_TYPES),
            cursor_parameter(),
        ]
    }
}

/// Query parameters of the WebSocket route.
///
/// Both values are optional: without `types` the connection starts with no event type and
/// subscribes with a command (FR-8, FR-10). A `types` that is present is parsed like the one of
/// the SSE route, so an empty value is the 422 violation `required`. The document describes them
/// by [`IntoParams`].
#[derive(Debug, Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct WsParams {
    /// Comma-separated event type names.
    pub types: Option<String>,
    /// The cursor of a resume; checked also beside the `Last-Event-ID` header, which wins.
    pub last_event_id: Option<String>,
}

impl IntoParams for WsParams {
    /// `types`, optional, then `lastEventId`.
    fn into_params(_parameter_in: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        vec![
            types_parameter(Required::False, WS_TYPES),
            cursor_parameter(),
        ]
    }
}

/// The `Last-Event-ID` header of both streaming routes, as a parameter of the document. A
/// documentation type: the handlers read the header from the `HeaderMap`.
#[derive(Debug)]
pub struct CursorHeader;

impl IntoParams for CursorHeader {
    /// The one optional header parameter `Last-Event-ID`.
    fn into_params(_parameter_in: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        vec![
            ParameterBuilder::new()
                .name(CURSOR_HEADER)
                .parameter_in(ParameterIn::Header)
                .required(Required::False)
                .description(Some(CURSOR_HEADER_TEXT))
                .schema(Some(cursor_schema()))
                .example(Some(json!(EXAMPLE_CURSOR)))
                .build(),
        ]
    }
}

/// The handshake headers of the WebSocket route, as parameters of the document: each schema
/// admits exactly the values the upgrade accepts. A documentation type: the upgrade extractor
/// reads the headers.
#[derive(Debug)]
pub struct HandshakeHeaders;

impl IntoParams for HandshakeHeaders {
    /// `Connection`, `Upgrade`, `Sec-WebSocket-Version` and `Sec-WebSocket-Key`, all required.
    fn into_params(_parameter_in: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        vec![
            handshake_header(
                "Connection",
                text_schema(Some(CONNECTION_PATTERN)),
                "Upgrade",
                CONNECTION_TEXT,
            ),
            handshake_header(
                "Upgrade",
                text_schema(Some(UPGRADE_PATTERN)),
                "websocket",
                UPGRADE_TEXT,
            ),
            handshake_header(
                "Sec-WebSocket-Version",
                version_schema(),
                WEBSOCKET_VERSION,
                VERSION_TEXT,
            ),
            handshake_header(
                "Sec-WebSocket-Key",
                text_schema(None),
                EXAMPLE_KEY,
                KEY_TEXT,
            ),
        ]
    }
}

/// One event of the `text/event-stream` answer of the SSE route, as the fields of its frame.
///
/// The frame is the lines `id: <id>`, `event: <type>` and `data: <event object>`, then an empty
/// line; a comment line `:` followed by an empty line is the keep-alive and carries no event. A
/// documentation type: the route writes the frame itself.
#[derive(Debug, ToSchema)]
pub struct SseFrame {
    /// The event type, an event type name.
    #[schema(schema_with = frame_event)]
    pub event: String,
    /// The event id in decimal.
    #[schema(schema_with = frame_id)]
    pub id: String,
    /// The event object as one line of compact JSON.
    #[schema(schema_with = frame_data)]
    pub data: String,
}

/// The grammar of an event type name, the `pattern` of every schema of a name.
const NAME_PATTERN: &str = "^[a-z][a-z0-9]*([._-][a-z0-9]+)*$";

/// The grammar of a cursor: the decimal digits of an unsigned 64-bit integer, at most
/// 18446744073709551615, after any number of leading zeros; the rule of `Cursor::parse`.
const CURSOR_PATTERN: &str = concat!(
    "^0*([0-9]{1,19}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}",
    "|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}",
    "|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}",
    "|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{4}|18446744073709550[0-9]{3}",
    "|18446744073709551[0-5][0-9]{2}|1844674407370955160[0-9]|1844674407370955161[0-5])$",
);

/// A `Connection` header the upgrade accepts: one holding `upgrade` in any letter case.
const CONNECTION_PATTERN: &str = "[Uu][Pp][Gg][Rr][Aa][Dd][Ee]";

/// An `Upgrade` header the upgrade accepts: `websocket` in any letter case.
const UPGRADE_PATTERN: &str = "^[Ww][Ee][Bb][Ss][Oo][Cc][Kk][Ee][Tt]$";

/// The one `Sec-WebSocket-Version` the upgrade accepts, that of RFC 6455.
const WEBSOCKET_VERSION: &str = "13";

/// The event type of the examples: the type the load-test publisher writes (NFR-20).
pub(crate) const EXAMPLE_TYPE: &str = "load.test";

/// The cursor of the examples, also the id of the example frames.
pub(crate) const EXAMPLE_CURSOR: &str = "1043";

/// The grammar of a frame id: a decimal number of 1 or greater, without a leading zero.
const FRAME_ID_PATTERN: &str = "^[1-9][0-9]*$";

/// The digits of the largest frame id, 18446744073709551615.
const FRAME_ID_DIGITS: usize = 20;

/// The description of the `event` field of [`SseFrame`].
const FRAME_EVENT_TEXT: &str = "The event type; a browser `EventSource` hands the frame to the \
    listener registered for this name, and its `onmessage` handler receives nothing.";

/// The description of the `id` field of [`SseFrame`].
const FRAME_ID_TEXT: &str = "The event id in decimal: the Kafka offset plus one, the same on \
    every node. The ids of one stream strictly increase and have gaps; a client resumes with the \
    last one it received.";

/// The `Sec-WebSocket-Key` of the examples: sixteen zero bytes in base64.
const EXAMPLE_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAA==";

/// The event object of the examples, as both streaming routes write it.
pub(crate) const EXAMPLE_EVENT: &str = r#"{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"load.test","occurredAt":"2026-11-14T10:00:00Z","data":{"run":1}}"#;

/// The description of `types` on the SSE route.
const STREAM_TYPES: &str = "The event types to deliver: 1 to 100 distinct event type \
    names separated by commas, each of 1 to 128 characters matching \
    `^[a-z][a-z0-9]*([._-][a-z0-9]+)*$`. A repeated name counts once, so a list of more than 100 \
    names of which at most 100 are distinct is accepted too, which the schema, counting every \
    name, cannot express. A missing `types` or a parameter given twice is the 400 problem, an \
    invalid list the 422 problem with one violation on `types`.";

/// The description of `types` on the WebSocket route.
const WS_TYPES: &str = "The event types to deliver from the start, as on the SSE \
    route: 1 to 100 distinct event type names separated by commas. Optional: without it the \
    connection starts with no type and the client subscribes with a command. A repeated name \
    counts once, so a list of more than 100 names of which at most 100 are distinct is accepted \
    too, which the schema, counting every name, cannot express. A parameter given twice is the \
    400 problem, an invalid list the 422 problem with one violation on `types`.";

/// The description of `lastEventId`.
const CURSOR_PARAMETER_TEXT: &str = "The cursor of a resume: the id of the last event \
    received, a decimal unsigned 64-bit integer (digits only, at most 18446744073709551615, \
    leading zeros accepted). The stream first delivers the retained events of the types of the \
    request above it, then live events; a valid `Last-Event-ID` header wins over it. A browser \
    `WebSocket`, which cannot set headers, resumes with it. Any other value is the 400 problem, \
    also beside a valid `Last-Event-ID` header.";

/// The description of the `Last-Event-ID` header.
const CURSOR_HEADER_TEXT: &str = "The cursor of a resume, with the grammar of \
    `lastEventId`; it wins over a valid `lastEventId`, and a malformed `lastEventId` beside it \
    is still the 400 problem of `lastEventId`. A browser `EventSource` sends it by itself when it \
    reconnects. Any other value, a value that is not visible ASCII text included, is the 400 \
    problem.";

/// The description of the `Connection` handshake header.
const CONNECTION_TEXT: &str = "Must hold `upgrade` in any letter case, as `Upgrade` \
    or `keep-alive, Upgrade`; any other value, or none, is the 400 problem.";

/// The description of the `Upgrade` handshake header.
const UPGRADE_TEXT: &str =
    "Must be `websocket` in any letter case; any other value, or none, is the 400 problem.";

/// The description of the `Sec-WebSocket-Version` handshake header.
const VERSION_TEXT: &str =
    "Must be `13`, the version of RFC 6455; any other value, or none, is the 400 problem.";

/// The description of the `Sec-WebSocket-Key` handshake header.
const KEY_TEXT: &str = "The key from which the server derives \
    `Sec-WebSocket-Accept`. RFC 6455 asks for 16 random bytes in base64; any value is accepted. \
    Without it the route answers the 400 problem.";

/// The `types` parameter: the comma-separated names, as an array of names that is not exploded.
fn types_parameter(required: Required, description: &str) -> Parameter {
    ParameterBuilder::new()
        .name("types")
        .parameter_in(ParameterIn::Query)
        .required(required)
        .description(Some(description))
        .style(Some(ParameterStyle::Form))
        .explode(Some(false))
        .schema(Some(
            ArrayBuilder::new()
                .items(name_schema())
                .min_items(Some(1))
                .max_items(Some(Subscription::MAX_TYPES)),
        ))
        .example(Some(json!([EXAMPLE_TYPE])))
        .build()
}

/// The `lastEventId` parameter.
fn cursor_parameter() -> Parameter {
    ParameterBuilder::new()
        .name(CURSOR_PARAMETER)
        .parameter_in(ParameterIn::Query)
        .required(Required::False)
        .description(Some(CURSOR_PARAMETER_TEXT))
        .schema(Some(cursor_schema()))
        .example(Some(json!(EXAMPLE_CURSOR)))
        .build()
}

/// A required handshake header.
fn handshake_header(name: &str, schema: Object, example: &str, description: &str) -> Parameter {
    ParameterBuilder::new()
        .name(name)
        .parameter_in(ParameterIn::Header)
        .required(Required::True)
        .description(Some(description))
        .schema(Some(schema))
        .example(Some(json!(example)))
        .build()
}

/// A string, with the pattern when one is given.
fn text_schema(pattern: Option<&str>) -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .pattern(pattern)
        .build()
}

/// The schema of a cursor, in the parameter and in the header.
fn cursor_schema() -> Object {
    text_schema(Some(CURSOR_PATTERN))
}

/// The schema of `Sec-WebSocket-Version`: the one version accepted.
fn version_schema() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .enum_values(Some([WEBSOCKET_VERSION]))
        .build()
}

/// The schema of an event type name.
fn name_schema() -> ObjectBuilder {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .min_length(Some(1))
        .max_length(Some(EventType::MAX_CHARS))
        .pattern(Some(NAME_PATTERN))
}

/// The `event` field of [`SseFrame`]: an event type name.
fn frame_event() -> Object {
    name_schema()
        .description(Some(FRAME_EVENT_TEXT))
        .examples([EXAMPLE_TYPE])
        .build()
}

/// The `id` field of [`SseFrame`]: the decimal id of the event, which a cursor names.
fn frame_id() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .description(Some(FRAME_ID_TEXT))
        .min_length(Some(1))
        .max_length(Some(FRAME_ID_DIGITS))
        .pattern(Some(FRAME_ID_PATTERN))
        .examples([EXAMPLE_CURSOR])
        .build()
}

/// The `data` field of [`SseFrame`]: a string of JSON whose content is the event object.
fn frame_data() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .description(Some(
            "The event object as one line of compact JSON: `id`, `type`, `occurredAt` and \
             `data`, in this order.",
        ))
        .content_media_type("application/json")
        .content_schema(Some(event_object()))
        .examples([EXAMPLE_EVENT])
        .build()
}

/// The event object, the same on both streaming transports: the four members of the envelope
/// and no other.
fn event_object() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::Object)
        .description(Some(
            "The event object, the same on both streaming transports.",
        ))
        .property(
            "id",
            ObjectBuilder::new()
                .schema_type(Type::String)
                .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
                .description(Some(
                    "The envelope id the producer wrote, a UUID in lower case. It identifies the \
                     event, not its position: a client deduplicates by it.",
                )),
        )
        .required("id")
        .property(
            "type",
            name_schema().description(Some("The event type name.")),
        )
        .required("type")
        .property(
            "occurredAt",
            ObjectBuilder::new()
                .schema_type(Type::String)
                .format(Some(SchemaFormat::KnownFormat(KnownFormat::DateTime)))
                .description(Some(
                    "The time the producer wrote, an RFC 3339 time in UTC with the suffix `Z` or \
                     `+00:00`, unchanged.",
                )),
        )
        .required("occurredAt")
        .property(
            "data",
            ObjectBuilder::new()
                .schema_type(SchemaType::AnyValue)
                .description(Some("The producer's data, any JSON value.")),
        )
        .required("data")
        .additional_properties(Some(AdditionalProperties::<Schema>::FreeForm(false)))
        .build()
}

/// A command a WebSocket client sends in a text frame (FR-10): `{"action":"subscribe",
/// "types":[..]}` or `{"action":"unsubscribe","types":[..]}`. Any other member, action or shape
/// does not parse; the names are checked by [`parse_names`] afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Command {
    /// Adds the types to the set of the connection.
    Subscribe {
        /// The names of the types.
        types: Vec<String>,
    },
    /// Removes the types from the set of the connection.
    Unsubscribe {
        /// The names of the types.
        types: Vec<String>,
    },
}

/// The message of a list without a name or with more than [`Subscription::MAX_TYPES`] distinct
/// names, both as a violation and as the reason of an error frame.
const TYPES_COUNT: &str = "types must hold 1 to 100 event type names";

/// The reason of the error frame of a name that is not an event type name, an empty one included.
const INVALID_NAME: &str = "invalid event type name";

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
            Self::Empty | Self::TooMany => TYPES_COUNT.to_owned(),
            Self::Blank => "types must not hold an empty event type name".to_owned(),
            Self::Name(reason) => reason.to_string(),
        }
    }

    /// The reason of the WebSocket error frame of the fault (FR-11); it never holds a name of the
    /// command.
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::Empty | Self::TooMany => TYPES_COUNT,
            Self::Blank | Self::Name(_) => INVALID_NAME,
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
pub(crate) fn types_problem(error: TypesError) -> Problem {
    Problem::validation_failed(vec![Violation {
        property_path: "types".to_owned(),
        message: error.message(),
        code: error.code().to_owned(),
    }])
}

/// A cursor carrier whose value is not a decimal unsigned 64-bit integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CursorFault {
    /// `Last-Event-ID` or `lastEventId`: the first carrier, the header then the parameter, whose
    /// value is not a cursor.
    carrier: &'static str,
}

impl CursorFault {
    /// The fault of the `Last-Event-ID` header.
    const HEADER: Self = Self {
        carrier: CURSOR_HEADER,
    };

    /// The fault of the `lastEventId` parameter; its 400 problem is the example of the SSE route.
    pub(crate) const PARAMETER: Self = Self {
        carrier: CURSOR_PARAMETER,
    };

    /// `<carrier> must be a decimal unsigned 64-bit integer`; it never holds the value.
    fn message(self) -> String {
        let carrier = self.carrier;
        format!("{carrier} must be a decimal unsigned 64-bit integer")
    }

    /// The 400 problem of the REST routes, unchanged (4/FR-16).
    pub(crate) fn bad_request(self) -> Problem {
        Problem::new(StatusCode::BAD_REQUEST).with_detail(self.message())
    }

    /// The 422 problem of the GraphQL field (FR-13): one violation whose `propertyPath` is the
    /// carrier, with the code `format` and the message of the fault.
    pub(crate) fn violation(self) -> Problem {
        Problem::validation_failed(vec![Violation {
            property_path: self.carrier.to_owned(),
            message: self.message(),
            code: "format".to_owned(),
        }])
    }
}

/// The cursor of a resume: the `Last-Event-ID` header when it is present, else the
/// `lastEventId` parameter, else none (4/FR-12). Every carrier that is present is checked, the
/// header first: a malformed parameter is refused also beside a valid header, which then wins.
///
/// # Errors
///
/// The [`CursorFault`] of the first carrier, the header then the parameter, whose value is not a
/// decimal unsigned 64-bit integer; a header value that is not visible ASCII text is such a value.
pub(crate) fn read_cursor(
    header: Option<&HeaderValue>,
    parameter: Option<&str>,
) -> Result<Option<Cursor>, CursorFault> {
    let header = header
        .map(|value| carried_cursor(value.to_str().ok(), CursorFault::HEADER))
        .transpose()?;
    let parameter = parameter
        .map(|text| carried_cursor(Some(text), CursorFault::PARAMETER))
        .transpose()?;
    Ok(header.or(parameter))
}

/// The cursor of one carrier's text; `None` stands for a header value that is not text.
fn carried_cursor(text: Option<&str>, fault: CursorFault) -> Result<Cursor, CursorFault> {
    text.and_then(|text| Cursor::parse(text).ok()).ok_or(fault)
}

/// The cursor of a resume by the rule of [`read_cursor`], for the REST routes.
///
/// # Errors
///
/// The 400 problem naming the carrier whose value is not a decimal unsigned 64-bit integer.
pub(crate) fn parse_cursor(
    headers: &HeaderMap,
    parameter: Option<&str>,
) -> Result<Option<Cursor>, Problem> {
    read_cursor(headers.get(CURSOR_HEADER), parameter).map_err(CursorFault::bad_request)
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
