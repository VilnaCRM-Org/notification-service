use async_graphql::Request;

use super::*;

const EXPECTED_SDL: &str = r#""Input of the create mutation."
input CreateExampleApiResourceInput {
  "Description."
  description: String!
  "Name."
  name: String!
}

"The status of the event stream on the answering instance; 64-bit values are decimal strings."
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

"Template example resource."
type ExampleApiResource {
  "Description."
  description: String!
  "Identifier."
  id: ID!
  "Name."
  name: String!
}

"Mutation root."
type Mutation {
  "Creates an example API resource."
  createExampleApiResource(input: CreateExampleApiResourceInput!): ExampleApiResource!
}

"Query root."
type Query {
  "Retrieves the status of the event stream on the answering instance."
  eventStreamStatus: EventStreamStatus!
  "Retrieves one page of the subscribable event types, in byte order of their names."
  eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!
  "Retrieves an example API resource by id."
  exampleApiResource(id: ID!): ExampleApiResource
  "Retrieves one page of example API resources, newest first."
  exampleApiResources(
    "Number of items per page, from 1 to 100."
    itemsPerPage: Int! = 30,
    "Page number, from 1."
    page: Int! = 1
  ): [ExampleApiResource!]!
}

"Directs the executor to include this field or fragment only when the `if` argument is true."
directive @include(if: Boolean!) on FIELD | FRAGMENT_SPREAD | INLINE_FRAGMENT
"Directs the executor to skip this field or fragment when the `if` argument is true."
directive @skip(if: Boolean!) on FIELD | FRAGMENT_SPREAD | INLINE_FRAGMENT
schema {
  query: Query
  mutation: Mutation
}
"#;

#[test]
fn the_sdl_is_the_documented_schema() {
    assert_eq!(sdl(), EXPECTED_SDL);
}

#[test]
fn the_sdl_has_the_five_operations_and_nothing_else_to_call() {
    let text = sdl();

    assert!(text.contains("eventStreamStatus: EventStreamStatus!"));
    assert!(text.contains("eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!"));
    assert!(text.contains("exampleApiResource(id: ID!)"));
    assert!(text.contains("exampleApiResources("));
    assert!(text.contains("createExampleApiResource(input: CreateExampleApiResourceInput!)"));
    for absent in [
        "node",
        "Connection",
        "update",
        "delete",
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
async fn the_builder_gives_a_schema_with_the_two_roots() {
    let schema: ApiSchema = builder().finish();

    let response = schema.execute(Request::new("{ query: __typename }")).await;
    let mutation = schema
        .execute(Request::new("mutation { mutation: __typename }"))
        .await;

    assert_eq!(
        response.data.into_json().unwrap(),
        serde_json::json!({ "query": "Query" })
    );
    assert_eq!(
        mutation.data.into_json().unwrap(),
        serde_json::json!({ "mutation": "Mutation" })
    );
}
