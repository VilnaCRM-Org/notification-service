use std::fmt::Write as _;
use std::time::Duration;

use async_graphql::{Request, Response};

use super::*;

const EXPECTED_SDL: &str = r#""The status of the event stream on the answering instance; 64-bit values are decimal strings."
type EventStreamStatus {
  "Whether the last read of the topic watermarks succeeded."
  brokerReachable: Boolean!
  "End of the topic at the last successful watermark read."
  endOffset: String
  "Events between the position and the end of the topic."
  lag: String
  "Id of the last event read from the topic; null before the first read."
  position: String
  "Why the stream is not ready yet, only while starting."
  reason: String
  "Invalid records skipped by the reader."
  rejectedRecords: String!
  "Replays that reached their cap."
  replayCapped: String!
  "Jumps to the start of the topic because retention removed unread events."
  retentionLosses: String!
  "Life stage: starting, ready or shutting-down."
  state: String!
  "Number of open subscriptions served here."
  subscribers: Int!
  "Floor of the memory window."
  windowFirst: String
  "Id of the newest event held in the memory window."
  windowLast: String
}

"A subscribable event type."
type EventType {
  "Name of the event type."
  name: String!
}

"Query root."
type Query {
  "Retrieves the status of the event stream on the answering instance."
  eventStreamStatus: EventStreamStatus!
  "Retrieves one page of the subscribable event types, in byte order of their names."
  eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!
}

"Directs the executor to include this field or fragment only when the `if` argument is true."
directive @include(if: Boolean!) on FIELD | FRAGMENT_SPREAD | INLINE_FRAGMENT
"Directs the executor to skip this field or fragment when the `if` argument is true."
directive @skip(if: Boolean!) on FIELD | FRAGMENT_SPREAD | INLINE_FRAGMENT
schema {
  query: Query
}
"#;

#[test]
fn the_sdl_is_the_documented_schema() {
    assert_eq!(sdl(), EXPECTED_SDL);
}

#[test]
fn the_sdl_has_the_two_queries_and_nothing_else_to_call() {
    let text = sdl();

    assert!(text.contains("eventStreamStatus: EventStreamStatus!"));
    assert!(text.contains("eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!"));
    for absent in [
        "node",
        "Connection",
        "update",
        "delete",
        "input ",
        "type Mutation",
        "mutation:",
        "type Subscription",
    ] {
        assert!(!text.contains(absent), "the SDL contains `{absent}`");
    }
}

#[test]
fn the_first_type_is_the_event_stream_status_with_twelve_single_line_fields() {
    let text = sdl();

    let first = text
        .split("\n\n")
        .find(|block| block.lines().any(|line| line.starts_with("type ")))
        .unwrap();
    let mut lines = first.lines().skip_while(|line| !line.starts_with("type "));
    assert_eq!(lines.next(), Some("type EventStreamStatus {"));
    let fields: Vec<&str> = lines
        .filter(|line| line.starts_with("  ") && !line.trim_start().starts_with('"'))
        .collect();
    assert_eq!(
        fields,
        [
            "  brokerReachable: Boolean!",
            "  endOffset: String",
            "  lag: String",
            "  position: String",
            "  reason: String",
            "  rejectedRecords: String!",
            "  replayCapped: String!",
            "  retentionLosses: String!",
            "  state: String!",
            "  subscribers: Int!",
            "  windowFirst: String",
            "  windowLast: String",
        ]
    );
    assert!(text.contains("type EventType {\n  \"Name of the event type.\"\n  name: String!\n}"));
}

#[test]
fn twenty_generations_are_byte_identical() {
    let first = sdl();

    for _ in 0..20 {
        assert_eq!(sdl(), first);
    }
}

#[tokio::test]
async fn the_builder_gives_a_schema_with_a_query_root_and_no_mutation_root() {
    let schema: ApiSchema = builder().finish();

    let response = schema.execute(Request::new("{ query: __typename }")).await;
    let mutation = schema
        .execute(Request::new("mutation { mutation: __typename }"))
        .await;

    assert_eq!(
        response.data.into_json().unwrap(),
        serde_json::json!({ "query": "Query" })
    );
    assert!(mutation.data.into_json().unwrap().is_null());
    let mut messages: Vec<&str> = mutation
        .errors
        .iter()
        .map(|error| error.message.as_str())
        .collect();
    messages.dedup();
    assert_eq!(messages, ["Schema is not configured for mutations."]);
}

/// Executes `query` on the bounded schema within five seconds.
async fn execute(query: &str) -> Response {
    let schema: ApiSchema = builder().finish();
    tokio::time::timeout(Duration::from_secs(5), schema.execute(Request::new(query)))
        .await
        .expect("the query answers in time")
}

/// A `__type` selection whose deepest field sits at `depth`.
fn nested(depth: usize) -> String {
    let inner = depth.saturating_sub(2);
    format!(
        "{{ __type(name: \"Query\") {{ {}name{} }} }}",
        "ofType { ".repeat(inner),
        " }".repeat(inner)
    )
}

/// Aliased `__type` selections of exactly `complexity` (at least two): each counts two
/// (`__type` and `name`), and an odd total gives the first one a third field (`kind`).
fn aliases(complexity: usize) -> String {
    let odd = complexity % 2 == 1;
    let count = complexity / 2;
    let mut document = String::from("{ ");
    for n in 0..count {
        let extra = if odd && n == 0 { " kind" } else { "" };
        write!(document, "a{n}: __type(name: \"Query\") {{ name{extra} }} ").unwrap();
    }
    document.push('}');
    document
}

fn refused_with(response: &Response, message: &str) {
    assert!(
        response.data.clone().into_json().unwrap().is_null(),
        "data is absent"
    );
    let messages: Vec<&str> = response
        .errors
        .iter()
        .map(|error| error.message.as_str())
        .collect();
    assert_eq!(messages, [message]);
}

#[tokio::test]
async fn a_document_at_the_depth_limit_is_answered() {
    let response = execute(&nested(MAX_DEPTH)).await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    assert_eq!(
        response.data.into_json().unwrap(),
        serde_json::json!({ "__type": { "ofType": null } })
    );
}

#[tokio::test]
async fn a_document_one_level_deeper_than_the_limit_is_refused_without_data() {
    let response = execute(&nested(MAX_DEPTH + 1)).await;

    refused_with(&response, "Query is nested too deep.");
}

#[tokio::test]
async fn a_twenty_level_introspection_document_is_refused_without_data() {
    let response = execute(&nested(20)).await;

    refused_with(&response, "Query is nested too deep.");
}

#[tokio::test]
async fn a_document_at_the_complexity_limit_is_answered() {
    let response = execute(&aliases(MAX_COMPLEXITY)).await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data.as_object().unwrap().len(), MAX_COMPLEXITY / 2);
    assert_eq!(data["a499"], serde_json::json!({ "name": "Query" }));
}

#[tokio::test]
async fn a_document_above_the_complexity_limit_is_refused_without_data() {
    let response = execute(&aliases(MAX_COMPLEXITY + 1)).await;

    refused_with(&response, "Query is too complex.");
}

#[tokio::test]
async fn the_standard_introspection_query_of_the_explorer_is_answered() {
    let response = execute(INTROSPECTION).await;

    assert!(response.errors.is_empty(), "{:?}", response.errors);
    let data = response.data.into_json().unwrap();
    assert_eq!(data["__schema"]["queryType"]["name"], "Query");
}

#[test]
fn the_limits_are_sixteen_levels_and_a_thousand_fields() {
    assert_eq!(MAX_DEPTH, 16);
    assert_eq!(MAX_COMPLEXITY, 1000);
}

/// The introspection query `GraphiQL` sends.
const INTROSPECTION: &str = "
query IntrospectionQuery {
  __schema {
    queryType { name }
    mutationType { name }
    subscriptionType { name }
    types { ...FullType }
    directives { name description locations args { ...InputValue } }
  }
}
fragment FullType on __Type {
  kind name description
  fields(includeDeprecated: true) {
    name description
    args { ...InputValue }
    type { ...TypeRef }
    isDeprecated deprecationReason
  }
  inputFields { ...InputValue }
  interfaces { ...TypeRef }
  enumValues(includeDeprecated: true) { name description isDeprecated deprecationReason }
  possibleTypes { ...TypeRef }
}
fragment InputValue on __InputValue {
  name description
  type { ...TypeRef }
  defaultValue
}
fragment TypeRef on __Type {
  kind name
  ofType { kind name ofType { kind name ofType { kind name ofType { kind name
    ofType { kind name ofType { kind name ofType { kind name ofType { kind name } } } } } } } }
}
";
