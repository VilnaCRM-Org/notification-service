use std::any::TypeId;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use async_graphql::{EmptyMutation, EmptySubscription, Request, Schema};
use event_stream_domain::{Cursor, EventType};
use rstest::rstest;
use serde_json::{Value, json};
use shared_domain::{ErasedBusError, ErasedMessage, QueryBus, QueryHandler as _};
use test_support::bus::FakeQueryBus;
use test_support::fixture::stored_event;
use tokio::sync::watch;

use super::{EventStreamQuery, EventStreamStatusObject, EventTypeObject};
use crate::catalog::EventCatalog;
use crate::http::EventStreamState;
use crate::http::dto::EventStreamStatusResponse;
use crate::http::hub::{Hub, HubSettings};
use crate::query::EventTypeView;
use crate::query::list_event_types::{ListEventTypes, ListEventTypesHandler};
use crate::status::StatusSource;
use crate::tail::{TailState, TopicFault};
use crate::window::Window;

/// The bound of every awaited query of these tests.
const WAIT: Duration = Duration::from_secs(5);

type TestSchema = Schema<EventStreamQuery, EmptyMutation, EmptySubscription>;

const SETTINGS: HubSettings = HubSettings {
    max_subscribers: 4,
    buffer: 2,
};

const ALL_FIELDS: &str = "state reason brokerReachable position endOffset lag windowFirst \
    windowLast subscribers rejectedRecords retentionLosses replayCapped";

/// Asks the real handler, as the in-memory bus does.
#[derive(Debug)]
struct HandlerQueryBus(ListEventTypesHandler);

#[async_trait::async_trait]
impl QueryBus for HandlerQueryBus {
    async fn ask_erased(
        &self,
        _query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError> {
        let query = query
            .downcast::<ListEventTypes>()
            .map_err(|_other| ErasedBusError::NotRegistered { message: name })?;
        match self.0.handle(*query).await {
            Ok(output) => Ok(Box::new(output)),
            Err(error) => Err(ErasedBusError::Handler(Box::new(error))),
        }
    }
}

fn catalog_bus(names: &[&str]) -> Arc<dyn QueryBus> {
    let types = names
        .iter()
        .map(|name| EventType::new((*name).to_owned()).unwrap())
        .collect::<BTreeSet<_>>();
    Arc::new(HandlerQueryBus(ListEventTypesHandler::new(Arc::new(
        EventCatalog::new(types),
    ))))
}

fn state_with(
    query_bus: Arc<dyn QueryBus>,
    tail: TailState,
    window: Arc<Window>,
    hub: Arc<Hub>,
) -> EventStreamState {
    let (_sender, receiver) = watch::channel(tail);
    EventStreamState {
        query_bus,
        status: Arc::new(StatusSource::new(receiver, window, Arc::<Hub>::clone(&hub))),
        hub,
    }
}

fn starting(query_bus: Arc<dyn QueryBus>) -> EventStreamState {
    state_with(
        query_bus,
        TailState::default(),
        Arc::new(Window::new(10, usize::MAX)),
        Hub::new(SETTINGS),
    )
}

fn schema(state: EventStreamState) -> TestSchema {
    Schema::build(EventStreamQuery, EmptyMutation, EmptySubscription)
        .data(state)
        .finish()
}

async fn run(schema: &TestSchema, query: &str) -> Value {
    let response = tokio::time::timeout(WAIT, schema.execute(Request::new(query)))
        .await
        .expect("the query answers in time");
    serde_json::to_value(response).unwrap()
}

fn three() -> TestSchema {
    schema(starting(catalog_bus(&[
        "user.deleted",
        "order.paid",
        "user.created",
    ])))
}

fn names(response: &Value) -> Value {
    assert_eq!(response.get("errors"), None, "{response}");
    response["data"]["eventTypes"].clone()
}

// ---- eventTypes ----

#[rstest]
#[case::defaults("{ eventTypes { name } }", json!([{"name": "order.paid"}, {"name": "user.created"}, {"name": "user.deleted"}]))]
#[case::second_of_one("{ eventTypes(page: 2, itemsPerPage: 1) { name } }", json!([{"name": "user.created"}]))]
#[case::first_of_one("{ eventTypes(itemsPerPage: 1) { name } }", json!([{"name": "order.paid"}]))]
#[case::third_of_one("{ eventTypes(page: 3, itemsPerPage: 1) { name } }", json!([{"name": "user.deleted"}]))]
#[case::beyond_the_last("{ eventTypes(page: 4, itemsPerPage: 1) { name } }", json!([]))]
#[case::largest_size("{ eventTypes(itemsPerPage: 100) { name } }", json!([{"name": "order.paid"}, {"name": "user.created"}, {"name": "user.deleted"}]))]
#[case::second_page_of_the_default_size("{ eventTypes(page: 2) { name } }", json!([]))]
#[tokio::test]
async fn event_types_returns_the_page_of_the_rest_operation(
    #[case] query: &str,
    #[case] expected: Value,
) {
    let response = run(&three(), query).await;

    assert_eq!(names(&response), expected);
}

#[tokio::test]
async fn event_types_without_arguments_asks_the_first_page_of_30() {
    let queries = Arc::new(FakeQueryBus::answering(|query: ListEventTypes| {
        assert_eq!((query.page, query.items_per_page), (1, 30));
        Ok(vec![EventTypeView {
            name: "order.paid".to_owned(),
        }])
    }));
    let schema = schema(starting(Arc::<FakeQueryBus>::clone(&queries)));

    let response = run(&schema, "{ eventTypes { name } }").await;

    assert_eq!(names(&response), json!([{"name": "order.paid"}]));
    assert_eq!(queries.asked(), vec!["ListEventTypes"]);
}

#[rstest]
#[case::too_many_items(
    "itemsPerPage: 101",
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
#[case::no_items(
    "itemsPerPage: 0",
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
#[case::negative_items(
    "itemsPerPage: -3",
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
#[case::page_zero("page: 0", "page", "page must be 1 or greater")]
#[case::page_negative("page: -1", "page", "page must be 1 or greater")]
#[tokio::test]
async fn event_types_outside_the_bounds_is_the_error_of_the_422_problem(
    #[case] arguments: &str,
    #[case] property: &str,
    #[case] message: &str,
) {
    let response = run(
        &three(),
        &format!("{{ eventTypes({arguments}) {{ name }} }}"),
    )
    .await;

    assert_eq!(response["data"], Value::Null);
    let error = &response["errors"][0];
    assert_eq!(error["message"], "The request failed validation.");
    assert_eq!(error["path"], json!(["eventTypes"]));
    assert_eq!(error["extensions"]["code"], "VALIDATION_FAILED");
    assert_eq!(
        error["extensions"]["violations"],
        json!([{"propertyPath": property, "message": message, "code": "range"}])
    );
}

#[test]
fn an_event_type_object_carries_the_name_of_the_view() {
    let object = EventTypeObject::from(EventTypeView {
        name: "user.created".to_owned(),
    });

    assert_eq!(
        object,
        EventTypeObject {
            name: "user.created".to_owned()
        }
    );
}

// ---- eventStreamStatus ----

#[tokio::test]
async fn event_stream_status_of_a_starting_node_has_the_nulls_of_an_unknown_position() {
    let response = run(
        &three(),
        &format!("{{ eventStreamStatus {{ {ALL_FIELDS} }} }}"),
    )
    .await;

    assert_eq!(response.get("errors"), None, "{response}");
    assert_eq!(
        response["data"]["eventStreamStatus"],
        json!({
            "state": "starting",
            "reason": "catching-up",
            "brokerReachable": false,
            "position": null,
            "endOffset": null,
            "lag": null,
            "windowFirst": null,
            "windowLast": null,
            "subscribers": 0,
            "rejectedRecords": "0",
            "retentionLosses": "0",
            "replayCapped": "0"
        })
    );
}

#[tokio::test]
async fn event_stream_status_returns_the_values_of_the_rest_answer() {
    let window = Arc::new(Window::new(10, usize::MAX));
    window.restart(Cursor::new(30));
    window.push(Arc::new(stored_event(35, "user.created", "{}")));
    let hub = Hub::new(SETTINGS);
    hub.start(Cursor::new(35));
    let types = BTreeSet::from([EventType::new("user.created".to_owned()).unwrap()]);
    let _subscriber = hub.register(&types).unwrap();
    let tail = TailState {
        started: true,
        broker_reachable: true,
        position: Some(Cursor::new(u64::MAX)),
        end: Some(Cursor::new(u64::MAX)),
        rejected: 3,
        retention_losses: 2,
        ..TailState::default()
    };
    let state = state_with(catalog_bus(&[]), tail, window, hub);
    let rest =
        serde_json::to_value(EventStreamStatusResponse::from(state.status.snapshot())).unwrap();

    let response = run(
        &schema(state),
        &format!("{{ eventStreamStatus {{ {ALL_FIELDS} }} }}"),
    )
    .await;

    assert_eq!(response.get("errors"), None, "{response}");
    let status = &response["data"]["eventStreamStatus"];
    assert_eq!(status, &rest);
    assert_eq!(status["state"], "ready");
    assert_eq!(status["position"], "18446744073709551615");
    assert_eq!(status["subscribers"], 1);
    assert_eq!(status["rejectedRecords"], "3");
}

#[rstest]
#[case::unreachable(Some(TopicFault::Unreachable), "broker-unreachable")]
#[case::missing(Some(TopicFault::Missing), "topic-not-found")]
#[case::partitions(Some(TopicFault::Partitions(2)), "partition-count")]
#[tokio::test]
async fn event_stream_status_names_the_reason(
    #[case] fault: Option<TopicFault>,
    #[case] reason: &str,
) {
    let tail = TailState {
        fault,
        ..TailState::default()
    };
    let state = state_with(
        catalog_bus(&[]),
        tail,
        Arc::new(Window::new(10, usize::MAX)),
        Hub::new(SETTINGS),
    );

    let response = run(&schema(state), "{ eventStreamStatus { state reason } }").await;

    assert_eq!(
        response["data"]["eventStreamStatus"],
        json!({"state": "starting", "reason": reason})
    );
}

#[tokio::test]
async fn event_stream_status_of_a_closed_hub_is_shutting_down() {
    let hub = Hub::new(SETTINGS);
    hub.close();
    let state = state_with(
        catalog_bus(&[]),
        TailState::default(),
        Arc::new(Window::new(10, usize::MAX)),
        hub,
    );

    let response = run(&schema(state), "{ eventStreamStatus { state reason } }").await;

    assert_eq!(
        response["data"]["eventStreamStatus"],
        json!({"state": "shutting-down", "reason": null})
    );
}

fn response_with(subscribers: usize) -> EventStreamStatusResponse {
    EventStreamStatusResponse {
        state: "ready".to_owned(),
        reason: None,
        broker_reachable: true,
        position: Some("4".to_owned()),
        end_offset: Some("5".to_owned()),
        lag: Some("1".to_owned()),
        window_first: Some("2".to_owned()),
        window_last: Some("3".to_owned()),
        subscribers,
        rejected_records: "6".to_owned(),
        retention_losses: "7".to_owned(),
        replay_capped: "8".to_owned(),
    }
}

#[test]
fn the_status_object_copies_every_field_of_the_rest_answer() {
    let object = EventStreamStatusObject::from(response_with(9));

    assert_eq!(
        object,
        EventStreamStatusObject {
            state: "ready".to_owned(),
            reason: None,
            broker_reachable: true,
            position: Some("4".to_owned()),
            end_offset: Some("5".to_owned()),
            lag: Some("1".to_owned()),
            window_first: Some("2".to_owned()),
            window_last: Some("3".to_owned()),
            subscribers: 9,
            rejected_records: "6".to_owned(),
            retention_losses: "7".to_owned(),
            replay_capped: "8".to_owned(),
        }
    );
}

#[rstest]
#[case::largest_int(2_147_483_647, i32::MAX)]
#[case::one_above(2_147_483_648, i32::MAX)]
#[case::largest_count(usize::MAX, i32::MAX)]
#[case::one_below(2_147_483_646, 2_147_483_646)]
fn a_count_above_the_graphql_int_is_capped(#[case] subscribers: usize, #[case] expected: i32) {
    let object = EventStreamStatusObject::from(response_with(subscribers));

    assert_eq!(object.subscribers, expected);
}

// ---- schema and missing state ----

#[rstest]
#[case::event_types("{ eventTypes { name } }", "eventTypes")]
#[case::event_stream_status("{ eventStreamStatus { state } }", "eventStreamStatus")]
#[tokio::test]
async fn a_schema_without_the_state_answers_an_internal_error(
    #[case] query: &str,
    #[case] field: &str,
) {
    let schema = Schema::build(EventStreamQuery, EmptyMutation, EmptySubscription).finish();

    let response = run(&schema, query).await;

    let error = &response["errors"][0];
    assert_eq!(error["message"], "Internal server error");
    assert_eq!(error["path"], json!([field]));
    assert_eq!(error["extensions"]["code"], "INTERNAL_SERVER_ERROR");
}

#[test]
fn the_sdl_declares_both_queries_and_both_types() {
    let sdl = Schema::build(EventStreamQuery, EmptyMutation, EmptySubscription)
        .finish()
        .sdl_with_options(
            async_graphql::SDLExportOptions::new()
                .sorted_fields()
                .sorted_arguments()
                .prefer_single_line_descriptions(),
        );

    for expected in [
        "eventTypes(itemsPerPage: Int! = 30, page: Int! = 1): [EventType!]!",
        "eventStreamStatus: EventStreamStatus!",
        "type EventType {",
        "name: String!",
        "type EventStreamStatus {",
        "state: String!",
        "reason: String\n",
        "brokerReachable: Boolean!",
        "position: String\n",
        "endOffset: String\n",
        "lag: String\n",
        "windowFirst: String\n",
        "windowLast: String\n",
        "subscribers: Int!",
        "rejectedRecords: String!",
        "retentionLosses: String!",
        "replayCapped: String!",
    ] {
        assert!(sdl.contains(expected), "the SDL lacks `{expected}`:\n{sdl}");
    }
    for absent in [
        "node",
        "Connection",
        "update",
        "delete",
        "type Subscription",
    ] {
        assert!(!sdl.contains(absent), "the SDL contains `{absent}`");
    }
}
