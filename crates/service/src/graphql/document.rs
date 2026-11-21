//! The documentation types of the two `GraphQL` operations of the `OpenAPI` document (FR-26).
//!
//! They describe the body of the POST form, the parameters of the GET form, the `Last-Event-ID`
//! header of both, the execution result and one event of the event stream, with their
//! descriptions and examples. They are used by the annotations of the handlers only. The
//! transports read the request and write the answer themselves, so the bytes on the wire do not
//! depend on these types.

use axum::http::StatusCode;
use event_stream_application::HubError;
use serde_json::{Value, json};
use shared_application::http::{Problem, Violation};
use utoipa::openapi::path::{Parameter, ParameterBuilder, ParameterIn};
use utoipa::openapi::schema::{
    AdditionalProperties, AnyOfBuilder, ArrayBuilder, Object, ObjectBuilder, Ref, Schema,
    SchemaType, Type,
};
use utoipa::openapi::{RefOr, Required};
use utoipa::{IntoParams, PartialSchema, ToSchema};

use crate::graphql::handler::{MAX_OPERATION_NAME_CHARS, MAX_QUERY_CHARS};

/// The event type of the examples: the type the load-test publisher writes (NFR-20).
pub(crate) const EXAMPLE_TYPE: &str = "load.test";

/// The event id of the examples.
pub(crate) const EXAMPLE_ID: &str = "1043";

/// The query of the examples.
pub(crate) const EXAMPLE_QUERY: &str = "{ eventTypes { name } }";

/// The subscription of the examples: the events of the type the load-test publisher writes.
pub(crate) const EXAMPLE_SUBSCRIPTION: &str =
    r#"subscription LoadTest { events(types: ["load.test"]) { id type data } }"#;

/// The operation name of the example subscription.
pub(crate) const EXAMPLE_OPERATION: &str = "LoadTest";

/// The detail of the 400 problem of a POST body that is not JSON, here the body `x`.
pub(crate) const NOT_JSON_DETAIL: &str =
    "Failed to parse the request body as JSON: expected value at line 1 column 1";

/// The detail of the 413 problem of a POST body larger than `HTTP_MAX_BODY_BYTES`.
pub(crate) const TOO_LARGE_DETAIL: &str =
    "Failed to buffer the request body: length limit exceeded";

/// The detail of the 415 problem of a POST body that is not `application/json`.
pub(crate) const MEDIA_TYPE_DETAIL: &str = "Expected request with `Content-Type: application/json`";

/// The detail of the 400 problem of a GET without `query`.
pub(crate) const MISSING_QUERY_DETAIL: &str =
    "Failed to deserialize query string: missing field `query`";

/// The detail of the 414 problem of a GET whose query string is too long.
pub(crate) const TOO_LONG_DETAIL: &str = "The query string is too long.";

/// One execution result of the example subscription: the event object of the other streaming
/// routes under `data`.
pub(crate) const EXAMPLE_SUBSCRIPTION_RESULT: &str = r#"{"data":{"events":{"id":"1043","type":"load.test","data":{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"load.test","occurredAt":"2026-11-14T10:00:00Z","data":{"run":1}}}}}"#;

/// The JSON-encoded empty object, the example of `variables` and `extensions`.
const EMPTY_OBJECT: &str = "{}";

/// The grammar of an event id, the `id` of a subscription result: a decimal number of 1 or
/// greater, without a leading zero.
const EVENT_ID_PATTERN: &str = "^[1-9][0-9]*$";

/// The digits of the largest event id, 18446744073709551615.
const EVENT_ID_DIGITS: usize = 20;

/// The description of [`GraphQLRequest`].
const REQUEST_TEXT: &str = "One GraphQL operation, the JSON body of GraphQL over HTTP \
    (section 4.4). Members other than the four listed are accepted and ignored; a JSON array, a \
    batch, is refused. `query` and `operationName` are bounded by their `maxLength`, the whole \
    body by `HTTP_MAX_BODY_BYTES`, whose default holds both at their bound in any encoding.";

/// The description of [`GraphQLResponse`].
const RESPONSE_TEXT: &str = "The execution result of one GraphQL operation. `data` is always \
    written, `null` when a request error stopped the operation; `errors` and `extensions` only \
    when they are not empty. An operation error, a document error or an invalid argument is a \
    result, not a problem.";

/// The description of [`GraphQLEvent`].
const EVENT_TEXT: &str = "One event of the `text/event-stream` answer of the GraphQL path \
    (graphql-sse, distinct connections mode), as the fields of its block. A `next` block carries \
    one execution result; a subscription writes one per event, with the event id, and never \
    `complete`; a single result is followed by `complete`, whose `data` line is empty. The comment \
    line `:` followed by an empty line is the keep-alive and carries no event.";

/// The description of `query`.
const QUERY_TEXT: &str = "The GraphQL document. A missing `query`, a parameter given twice or a \
    value that is not percent-encoded UTF-8 is the 400 problem; a document longer than its \
    `maxLength` is the 422 problem.";

/// The description of `operationName`.
const OPERATION_TEXT: &str = "The name of the operation of the document to run; needed only when \
    the document holds more than one. A name longer than its `maxLength` is the 422 problem.";

/// The description of the member `query` of [`GraphQLRequest`].
const BODY_QUERY_TEXT: &str =
    "The GraphQL document. A document longer than its `maxLength` is the 422 problem.";

/// The description of `variables`.
const VARIABLES_TEXT: &str = "The values of the variables of the operation: a JSON-encoded \
    object. Any other value is the 422 problem with one `format` violation on `variables`.";

/// The description of `extensions`.
const EXTENSIONS_TEXT: &str = "Protocol extensions: a JSON-encoded object, accepted and ignored. \
    Any other value is the 422 problem with one `format` violation on `extensions`.";

/// The description of the `Last-Event-ID` header of the `GraphQL` operations.
const CURSOR_TEXT: &str = "The cursor of a subscription: the id of the last event received, a \
    decimal unsigned 64-bit integer. It wins over the `lastEventId` argument of `events`. A \
    browser `EventSource` sends it by itself when it reconnects. A value that is not a cursor \
    fails the subscription inside the accepted stream with one `VALIDATION_FAILED` result on \
    `Last-Event-ID`, and is ignored by every other operation.";

/// A GraphQL request, the body of `POST /api/graphql`. A documentation type: the handler reads the
/// body as a JSON object.
#[derive(Debug)]
pub struct GraphQLRequest;

impl PartialSchema for GraphQLRequest {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::Object)
            .description(Some(REQUEST_TEXT))
            .property("query", bounded(BODY_QUERY_TEXT, MAX_QUERY_CHARS))
            .required("query")
            .property(
                "operationName",
                nullable(Type::String, OPERATION_TEXT).max_length(Some(MAX_OPERATION_NAME_CHARS)),
            )
            .property(
                "variables",
                nullable(
                    Type::Object,
                    "The values of the variables of the operation.",
                ),
            )
            .property(
                "extensions",
                nullable(Type::Object, "Protocol extensions, accepted and ignored."),
            )
            .additional_properties(Some(AdditionalProperties::<Schema>::FreeForm(true)))
            .into()
    }
}

impl ToSchema for GraphQLRequest {}

/// The execution result of one GraphQL operation, the JSON answer of the POST form and the `data`
/// of a `next` block. A documentation type: the transports encode async-graphql's result.
#[derive(Debug)]
pub struct GraphQLResponse;

impl PartialSchema for GraphQLResponse {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::Object)
            .description(Some(RESPONSE_TEXT))
            .property(
                "data",
                ObjectBuilder::new()
                    .schema_type(SchemaType::AnyValue)
                    .description(Some(
                        "The selected fields under their response names, or `null`.",
                    )),
            )
            .required("data")
            .property(
                "errors",
                ArrayBuilder::new()
                    .description(Some("The errors of the operation, each with its message."))
                    .items(error_schema()),
            )
            .property(
                "extensions",
                ObjectBuilder::new()
                    .schema_type(Type::Object)
                    .description(Some("Protocol extensions of the result.")),
            )
            .into()
    }
}

impl ToSchema for GraphQLResponse {}

/// One event of the `text/event-stream` answer of the GraphQL path, as the fields of its block. A
/// documentation type: the transport writes the blocks itself.
#[derive(Debug)]
pub struct GraphQLEvent;

impl PartialSchema for GraphQLEvent {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::Object)
            .description(Some(EVENT_TEXT))
            .property(
                "event",
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .description(Some(
                        "`next` for an execution result, `complete` after a single result.",
                    ))
                    .enum_values(Some(["next", "complete"])),
            )
            .required("event")
            .property("id", event_id())
            .property("data", event_data())
            .required("data")
            .into()
    }
}

impl ToSchema for GraphQLEvent {}

/// The query parameters of `GET /api/graphql`, the GET form of GraphQL over HTTP (section 4.3). A
/// documentation type: the handlers read the query string as received.
#[derive(Debug)]
pub struct GetParams;

impl IntoParams for GetParams {
    /// `query`, required, then `operationName`, `variables` and `extensions`.
    fn into_params(_parameter_in: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        vec![
            query_parameter(
                "query",
                Required::True,
                QUERY_TEXT,
                bounded_string(MAX_QUERY_CHARS),
            )
            .example(Some(json!(EXAMPLE_SUBSCRIPTION)))
            .build(),
            query_parameter(
                "operationName",
                Required::False,
                OPERATION_TEXT,
                bounded_string(MAX_OPERATION_NAME_CHARS),
            )
            .example(Some(json!(EXAMPLE_OPERATION)))
            .build(),
            json_object_parameter("variables", VARIABLES_TEXT),
            json_object_parameter("extensions", EXTENSIONS_TEXT),
        ]
    }
}

/// The `Last-Event-ID` header of both `GraphQL` operations. A documentation type: the handlers
/// read the header from the `HeaderMap`.
#[derive(Debug)]
pub struct CursorHeader;

impl IntoParams for CursorHeader {
    /// The one optional header parameter `Last-Event-ID`.
    fn into_params(_parameter_in: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        vec![
            ParameterBuilder::new()
                .name("Last-Event-ID")
                .parameter_in(ParameterIn::Header)
                .required(Required::False)
                .description(Some(CURSOR_TEXT))
                .schema(Some(string()))
                .example(Some(json!(EXAMPLE_ID)))
                .build(),
        ]
    }
}

/// The execution result of the example query.
pub(crate) fn query_result_example() -> Value {
    json!({"data": {"eventTypes": [{"name": EXAMPLE_TYPE}]}})
}

/// The example `next` block of a subscription result.
pub(crate) fn next_event_example() -> Value {
    json!({"event": "next", "id": EXAMPLE_ID, "data": EXAMPLE_SUBSCRIPTION_RESULT})
}

/// The example `complete` block, which follows a single result.
pub(crate) fn complete_event_example() -> Value {
    json!({"event": "complete", "data": ""})
}

/// The example problem of `status` with `detail`.
pub(crate) fn problem_example(status: StatusCode, detail: &str) -> Value {
    json!(Problem::new(status).with_detail(detail))
}

/// The example 422 problem of a GET whose `variables` is not a JSON-encoded object, the answer of
/// the query-string reader.
pub(crate) fn not_an_object_example() -> Value {
    json!(Problem::validation_failed(vec![Violation {
        property_path: "variables".to_owned(),
        message: "variables must be a JSON-encoded object".to_owned(),
        code: "format".to_owned(),
    }]))
}

/// The example 503 problem of a node that is starting, the answer of the hub's refusal, built
/// without its log entry.
pub(crate) fn unavailable_example() -> Value {
    problem_example(
        StatusCode::SERVICE_UNAVAILABLE,
        &HubError::NotReady.to_string(),
    )
}

/// Any string.
fn string() -> Object {
    ObjectBuilder::new().schema_type(Type::String).build()
}

/// A string with a description.
fn text(description: &str) -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .description(Some(description))
        .build()
}

/// A string of at most `bound` characters.
fn bounded_string(bound: usize) -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .max_length(Some(bound))
        .build()
}

/// A string of at most `bound` characters, with a description.
fn bounded(description: &str, bound: usize) -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .description(Some(description))
        .max_length(Some(bound))
        .build()
}

/// A value of `kind` or `null`, with a description.
fn nullable(kind: Type, description: &str) -> ObjectBuilder {
    ObjectBuilder::new()
        .schema_type(SchemaType::from_iter([kind, Type::Null]))
        .description(Some(description))
}

/// A query parameter of the GET form, without its example.
fn query_parameter(
    name: &str,
    required: Required,
    description: &str,
    schema: Object,
) -> ParameterBuilder {
    ParameterBuilder::new()
        .name(name)
        .parameter_in(ParameterIn::Query)
        .required(required)
        .description(Some(description))
        .schema(Some(schema))
}

/// An optional query parameter whose value is a JSON-encoded object, described by
/// `contentMediaType` and `contentSchema`; its schema example, the empty object, is the value a
/// generated request sends.
fn json_object_parameter(name: &str, description: &str) -> Parameter {
    let schema = ObjectBuilder::new()
        .schema_type(Type::String)
        .content_media_type("application/json")
        .content_schema(Some(ObjectBuilder::new().schema_type(Type::Object)))
        .examples([EMPTY_OBJECT])
        .build();
    query_parameter(name, Required::False, description, schema)
        .example(Some(json!(EMPTY_OBJECT)))
        .build()
}

/// One error of a result: its message, and the locations, the path and the extensions
/// async-graphql adds.
fn error_schema() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::Object)
        .property("message", text("What went wrong."))
        .required("message")
        .property(
            "locations",
            ArrayBuilder::new()
                .description(Some("The places of the document the error refers to."))
                .items(
                    ObjectBuilder::new()
                        .schema_type(Type::Object)
                        .property("line", ObjectBuilder::new().schema_type(Type::Integer))
                        .property("column", ObjectBuilder::new().schema_type(Type::Integer)),
                ),
        )
        .property(
            "path",
            ArrayBuilder::new()
                .description(Some(
                    "The response names and list indexes that led to the field.",
                ))
                .items(
                    ObjectBuilder::new()
                        .schema_type(SchemaType::from_iter([Type::String, Type::Integer])),
                ),
        )
        .property(
            "extensions",
            ObjectBuilder::new()
                .schema_type(Type::Object)
                .description(Some(
                    "`code` (`VALIDATION_FAILED` or `INTERNAL_SERVER_ERROR`) and the `violations` \
                     of a validation error.",
                )),
        )
        .build()
}

/// The `id` field of [`GraphQLEvent`]: the event id of a subscription result.
fn event_id() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .description(Some(
            "The event id of a subscription result in decimal, the `id` of the event; a single \
             result and `complete` have none. An `EventSource` sends the last one back as \
             `Last-Event-ID`.",
        ))
        .min_length(Some(1))
        .max_length(Some(EVENT_ID_DIGITS))
        .pattern(Some(EVENT_ID_PATTERN))
        .examples([EXAMPLE_ID])
        .build()
}

/// The `data` field of [`GraphQLEvent`]: empty for `complete`, else the execution result as one
/// line of compact JSON. The empty branch comes first, so a validator that decodes the JSON
/// branch never reads the empty data of `complete`.
fn event_data() -> RefOr<Schema> {
    AnyOfBuilder::new()
        .description(Some(
            "The execution result as one line of compact JSON for `next`; empty for `complete`.",
        ))
        .item(
            ObjectBuilder::new()
                .schema_type(Type::String)
                .max_length(Some(0)),
        )
        .item(
            ObjectBuilder::new()
                .schema_type(Type::String)
                .min_length(Some(1))
                .content_media_type("application/json")
                .content_schema(Some(Ref::from_schema_name("GraphQLResponse"))),
        )
        .into()
}

#[cfg(test)]
mod tests;
