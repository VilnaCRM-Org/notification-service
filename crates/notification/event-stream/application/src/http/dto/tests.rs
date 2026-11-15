use rstest::rstest;
use serde_json::{Value, json};
use shared_domain::PageRequest;
use utoipa::{PartialSchema as _, ToSchema as _};
use validator::Validate as _;

use event_stream_domain::{Cursor, Sequence};

use super::{EventStreamStatusResponse, EventTypeResponse, ListEventTypesParams};
use crate::query::EventTypeView;
use crate::status::{NodeState, NodeStatus, StartReason};

const fn params(page: i32, items_per_page: i32) -> ListEventTypesParams {
    ListEventTypesParams {
        page,
        items_per_page,
    }
}

#[test]
fn the_parameters_default_to_the_first_page_of_30() {
    let parsed: ListEventTypesParams = serde_json::from_value(json!({})).unwrap();

    assert_eq!(parsed, params(1, 30));
    assert_eq!(ListEventTypesParams::default(), params(1, 30));
}

#[test]
fn the_parameters_read_the_camel_case_names() {
    let parsed: ListEventTypesParams =
        serde_json::from_value(json!({"page": 4, "itemsPerPage": 9})).unwrap();

    assert_eq!(parsed, params(4, 9));
}

#[rstest]
#[case(0, 0)]
#[case(-1, 101)]
#[case(i32::MIN, i32::MIN)]
fn the_parameters_leave_the_bounds_to_the_page_request(#[case] page: i32, #[case] items: i32) {
    assert!(params(page, items).validate().is_ok());
}

#[rstest]
#[case(1, 30, 1, 30)]
#[case(7, 100, 7, 100)]
#[case(i32::MAX, 1, 2_147_483_647, 1)]
#[case(0, 0, 0, 0)]
#[case(-1, -5, 0, 0)]
#[case(i32::MIN, 101, 0, 101)]
#[case(3, i32::MIN, 3, 0)]
fn the_query_carries_both_values_and_turns_a_negative_value_into_0(
    #[case] page: i32,
    #[case] items: i32,
    #[case] expected_page: u32,
    #[case] expected_items: u32,
) {
    let query = params(page, items).into_query();

    assert_eq!(query.page, expected_page);
    assert_eq!(query.items_per_page, expected_items);
}

#[test]
fn the_default_parameters_are_the_page_request_defaults() {
    let query = ListEventTypesParams::default().into_query();

    assert_eq!(query.page, PageRequest::DEFAULT_PAGE);
    assert_eq!(query.items_per_page, PageRequest::DEFAULT_ITEMS_PER_PAGE);
}

#[test]
fn the_response_is_built_from_the_view_and_serializes_to_the_name() {
    let response = EventTypeResponse::from(EventTypeView {
        name: "user.created".to_owned(),
    });

    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        json!({"name": "user.created"})
    );
}

#[test]
fn the_response_schema_is_named_event_type_and_carries_the_grammar() {
    let schema: Value = serde_json::to_value(EventTypeResponse::schema()).unwrap();

    assert_eq!(EventTypeResponse::name(), "EventType");
    let name = &schema["properties"]["name"];
    assert_eq!(name["minLength"], 1);
    assert_eq!(name["maxLength"], 128);
    assert_eq!(name["pattern"], "^[a-z][a-z0-9]*([._-][a-z0-9]+)*$");
    assert_eq!(name["example"], "user.created");
    assert_eq!(schema["required"], json!(["name"]));
}

fn unknown_status() -> NodeStatus {
    NodeStatus {
        state: NodeState::Starting,
        reason: Some(StartReason::CatchingUp),
        broker_reachable: false,
        position: None,
        end_offset: None,
        lag: None,
        window_first: None,
        window_last: None,
        subscribers: 0,
        rejected_records: 0,
        retention_losses: 0,
        replay_capped: 0,
    }
}

fn known_status() -> NodeStatus {
    NodeStatus {
        state: NodeState::Ready,
        reason: None,
        broker_reachable: true,
        position: Some(Cursor::new(u64::MAX)),
        end_offset: Some(Cursor::new(40)),
        lag: Some(5),
        window_first: Some(Cursor::new(30)),
        window_last: Some(Sequence::new(35).unwrap()),
        subscribers: 7,
        rejected_records: 3,
        retention_losses: 2,
        replay_capped: 9,
    }
}

#[test]
fn a_known_status_serializes_every_field_with_its_wire_type() {
    let response = EventStreamStatusResponse::from(known_status());

    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        json!({
            "state": "ready",
            "reason": null,
            "brokerReachable": true,
            "position": "18446744073709551615",
            "endOffset": "40",
            "lag": "5",
            "windowFirst": "30",
            "windowLast": "35",
            "subscribers": 7,
            "rejectedRecords": "3",
            "retentionLosses": "2",
            "replayCapped": "9"
        })
    );
}

#[test]
fn an_unknown_position_serializes_to_nulls_and_zero_counters() {
    let response = EventStreamStatusResponse::from(unknown_status());

    assert_eq!(
        serde_json::to_value(&response).unwrap(),
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

#[rstest]
#[case::starting(NodeState::Starting, "starting")]
#[case::ready(NodeState::Ready, "ready")]
#[case::shutting_down(NodeState::ShuttingDown, "shutting-down")]
fn each_state_has_its_wire_name(#[case] state: NodeState, #[case] expected: &str) {
    let response = EventStreamStatusResponse::from(NodeStatus {
        state,
        ..known_status()
    });

    assert_eq!(response.state, expected);
}

#[rstest]
#[case::unreachable(StartReason::BrokerUnreachable, "broker-unreachable")]
#[case::missing(StartReason::TopicNotFound, "topic-not-found")]
#[case::partitions(StartReason::PartitionCount, "partition-count")]
#[case::catching_up(StartReason::CatchingUp, "catching-up")]
fn each_reason_has_its_wire_name(#[case] reason: StartReason, #[case] expected: &str) {
    let response = EventStreamStatusResponse::from(NodeStatus {
        reason: Some(reason),
        ..unknown_status()
    });

    assert_eq!(response.reason.as_deref(), Some(expected));
}

#[test]
fn the_status_schema_is_named_and_requires_the_twelve_fields() {
    let schema: Value = serde_json::to_value(EventStreamStatusResponse::schema()).unwrap();

    assert_eq!(EventStreamStatusResponse::name(), "EventStreamStatus");
    let properties = schema["properties"].as_object().unwrap();
    let mut names: Vec<&str> = properties.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "brokerReachable",
            "endOffset",
            "lag",
            "position",
            "reason",
            "rejectedRecords",
            "replayCapped",
            "retentionLosses",
            "state",
            "subscribers",
            "windowFirst",
            "windowLast"
        ]
    );
    assert_eq!(
        schema["required"],
        json!([
            "state",
            "brokerReachable",
            "subscribers",
            "rejectedRecords",
            "retentionLosses",
            "replayCapped"
        ])
    );
    for (name, property) in properties {
        let description = property["description"].as_str().unwrap_or_default();
        assert!(!description.is_empty(), "{name} has no description");
    }
}
