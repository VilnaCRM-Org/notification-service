use std::collections::BTreeSet;

use axum::http::header::HeaderName;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use rstest::rstest;
use serde_json::{Value, json};
use shared_application::http::{Problem, Violation};
use shared_domain::PageRequest;
use utoipa::{PartialSchema as _, ToSchema as _};
use validator::Validate as _;

use event_stream_domain::{Cursor, EventType, InvalidEvent, Sequence};

use super::{
    Command, CursorFault, EventStreamStatusResponse, EventTypeResponse, ListEventTypesParams,
    StreamParams, TypesError, WsParams, parse_cursor, parse_names, parse_types, read_cursor,
    types_problem,
};
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
            "reason",
            "brokerReachable",
            "position",
            "endOffset",
            "lag",
            "windowFirst",
            "windowLast",
            "subscribers",
            "rejectedRecords",
            "retentionLosses",
            "replayCapped"
        ])
    );
    for name in [
        "reason",
        "position",
        "endOffset",
        "lag",
        "windowFirst",
        "windowLast",
    ] {
        assert_eq!(
            properties[name]["type"],
            json!(["string", "null"]),
            "{name} stays nullable"
        );
    }
    for (name, property) in properties {
        let description = property["description"].as_str().unwrap_or_default();
        assert!(!description.is_empty(), "{name} has no description");
    }
}

// ---- streaming parameters ----

fn event_types(names: &[&str]) -> BTreeSet<EventType> {
    names
        .iter()
        .map(|name| EventType::new((*name).to_owned()).unwrap())
        .collect()
}

/// `count` distinct valid names: `t0`, `t1`, ...
fn distinct(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("t{index}")).collect()
}

#[test]
fn the_stream_parameters_read_the_camel_case_names_and_need_no_validation() {
    let parsed: StreamParams =
        serde_json::from_value(json!({"types": "a,b", "lastEventId": "7"})).unwrap();

    assert_eq!(parsed.types, "a,b");
    assert_eq!(parsed.last_event_id.as_deref(), Some("7"));
    assert!(parsed.validate().is_ok());
}

#[test]
fn the_stream_parameters_require_types_and_leave_the_cursor_optional() {
    let without_cursor: StreamParams = serde_json::from_value(json!({"types": ""})).unwrap();
    let missing = serde_json::from_value::<StreamParams>(json!({"lastEventId": "7"})).unwrap_err();

    assert_eq!(without_cursor.types, "");
    assert_eq!(without_cursor.last_event_id, None);
    assert_eq!(missing.to_string(), "missing field `types`");
}

#[test]
fn the_websocket_parameters_read_both_values_in_camel_case() {
    let parsed: WsParams =
        serde_json::from_value(json!({"types": "a,b", "lastEventId": "7"})).unwrap();

    assert_eq!(parsed.types.as_deref(), Some("a,b"));
    assert_eq!(parsed.last_event_id.as_deref(), Some("7"));
    assert!(parsed.validate().is_ok());
}

#[test]
fn the_websocket_parameters_leave_both_values_optional() {
    let empty: WsParams = serde_json::from_value(json!({})).unwrap();
    let blank: WsParams = serde_json::from_value(json!({"types": ""})).unwrap();

    assert_eq!(empty.types, None);
    assert_eq!(empty.last_event_id, None);
    assert_eq!(blank.types.as_deref(), Some(""));
}

fn command(text: &str) -> Result<Command, String> {
    serde_json::from_str::<Command>(text).map_err(|error| error.to_string())
}

#[test]
fn a_subscribe_command_reads_its_types() {
    assert_eq!(
        command(r#"{"action":"subscribe","types":["user.created","order.paid"]}"#),
        Ok(Command::Subscribe {
            types: vec!["user.created".to_owned(), "order.paid".to_owned()]
        })
    );
}

#[test]
fn an_unsubscribe_command_reads_its_types_in_any_member_order() {
    assert_eq!(
        command(r#"{"types":["order.paid"],"action":"unsubscribe"}"#),
        Ok(Command::Unsubscribe {
            types: vec!["order.paid".to_owned()]
        })
    );
}

#[test]
fn an_empty_list_of_types_is_a_command_whose_names_the_session_checks() {
    assert_eq!(
        command(r#"{"action":"subscribe","types":[]}"#),
        Ok(Command::Subscribe { types: Vec::new() })
    );
}

#[rstest]
#[case::not_json("subscribe")]
#[case::not_an_object(r#"["subscribe"]"#)]
#[case::unknown_action(r#"{"action":"replay","types":["a"]}"#)]
#[case::action_in_another_case(r#"{"action":"Subscribe","types":["a"]}"#)]
#[case::missing_action(r#"{"types":["a"]}"#)]
#[case::unknown_member(r#"{"action":"subscribe","types":["a"],"since":"1"}"#)]
#[case::unknown_member_of_unsubscribe(r#"{"action":"unsubscribe","types":["a"],"x":1}"#)]
#[case::missing_types(r#"{"action":"subscribe"}"#)]
#[case::types_not_a_list(r#"{"action":"subscribe","types":"a"}"#)]
#[case::name_not_a_string(r#"{"action":"subscribe","types":[1]}"#)]
#[case::types_twice(r#"{"action":"subscribe","types":["a"],"types":["b"]}"#)]
fn a_text_that_is_not_a_command_does_not_parse(#[case] text: &str) {
    assert!(command(text).is_err(), "{text}");
}

#[rstest]
#[case::empty(TypesError::Empty, "types must hold 1 to 100 event type names")]
#[case::too_many(TypesError::TooMany, "types must hold 1 to 100 event type names")]
#[case::blank(TypesError::Blank, "invalid event type name")]
#[case::length(TypesError::Name(InvalidEvent::TypeLength), "invalid event type name")]
#[case::pattern(TypesError::Name(InvalidEvent::TypePattern), "invalid event type name")]
fn each_fault_has_the_reason_of_its_error_frame(#[case] error: TypesError, #[case] reason: &str) {
    assert_eq!(error.reason(), reason);
}

/// Parses `names` through one instantiation of the generic `parse_names`: the coverage report
/// scores a generic function by its best instantiation, so every case goes through the same one.
fn names_of<S: AsRef<str>>(names: &[S]) -> Result<BTreeSet<EventType>, TypesError> {
    let owned: Vec<String> = names.iter().map(|name| name.as_ref().to_owned()).collect();
    parse_names(owned.iter().map(String::as_str))
}

#[test]
fn no_name_at_all_is_empty() {
    assert_eq!(names_of::<&str>(&[]), Err(TypesError::Empty));
}

#[rstest]
#[case::one(&["user.created"], &["user.created"])]
#[case::repeated_once(&["b", "a", "b", "a"], &["a", "b"])]
fn valid_names_are_one_set(#[case] given: &[&str], #[case] expected: &[&str]) {
    assert_eq!(names_of(given), Ok(event_types(expected)));
}

#[rstest]
#[case::blank(&[""], TypesError::Blank)]
#[case::blank_between(&["a", "", "b"], TypesError::Blank)]
#[case::blank_before_a_bad_name(&["a", "", "B"], TypesError::Blank)]
#[case::bad_name_before_a_blank(&["B", ""], TypesError::Name(InvalidEvent::TypePattern))]
#[case::pattern(&["User.Created"], TypesError::Name(InvalidEvent::TypePattern))]
#[case::blank_space(&[" "], TypesError::Name(InvalidEvent::TypePattern))]
fn the_first_faulty_name_decides(#[case] given: &[&str], #[case] expected: TypesError) {
    assert_eq!(names_of(given), Err(expected));
}

#[test]
fn a_name_of_128_characters_is_valid_and_one_of_129_is_a_length_fault() {
    let longest = "a".repeat(128);
    let too_long = "a".repeat(129);

    assert_eq!(names_of(&[&longest]), Ok(event_types(&[&longest])));
    assert_eq!(
        names_of(&[&too_long]),
        Err(TypesError::Name(InvalidEvent::TypeLength))
    );
}

#[test]
fn a_hundred_distinct_names_with_repeats_are_accepted() {
    let mut names = distinct(100);
    names.extend(distinct(100));

    let parsed = names_of(&names).unwrap();

    assert_eq!(parsed.len(), 100);
}

#[test]
fn the_101st_distinct_name_is_too_many_before_any_later_fault() {
    let mut names = distinct(101);
    names.push(String::new());
    names.push("B".to_owned());

    assert_eq!(names_of(&names), Err(TypesError::TooMany));
}

#[test]
fn a_faulty_101st_name_is_a_name_fault() {
    let mut names = distinct(100);
    names.push("B".to_owned());

    assert_eq!(
        names_of(&names),
        Err(TypesError::Name(InvalidEvent::TypePattern))
    );
}

fn violation(problem: &Problem) -> Value {
    assert_eq!(problem.status, 422);
    assert_eq!(problem.r#type, "/problems/validation-failed");
    assert_eq!(
        problem.detail.as_deref(),
        Some("The request failed validation.")
    );
    assert_eq!(problem.violations.len(), 1);
    serde_json::to_value(&problem.violations[0]).unwrap()
}

#[rstest]
#[case::empty(
    TypesError::Empty,
    "required",
    "types must hold 1 to 100 event type names"
)]
#[case::blank(
    TypesError::Blank,
    "required",
    "types must not hold an empty event type name"
)]
#[case::length(
    TypesError::Name(InvalidEvent::TypeLength),
    "length",
    "An event type must contain between 1 and 128 characters."
)]
#[case::pattern(
    TypesError::Name(InvalidEvent::TypePattern),
    "pattern",
    "An event type must match ^[a-z][a-z0-9]*([._-][a-z0-9]+)*$."
)]
#[case::too_many(
    TypesError::TooMany,
    "max_items",
    "types must hold 1 to 100 event type names"
)]
fn each_fault_is_one_violation_on_types(
    #[case] error: TypesError,
    #[case] code: &str,
    #[case] message: &str,
) {
    let problem = types_problem(error);

    assert_eq!(
        violation(&problem),
        json!({"propertyPath": "types", "message": message, "code": code})
    );
}

#[rstest]
#[case::empty_parameter("", "required")]
#[case::empty_entry("a,,b", "required")]
#[case::trailing_comma("a,", "required")]
#[case::pattern("User.Created", "pattern")]
#[case::too_long(&"a".repeat(129), "length")]
#[case::too_many(&distinct(101).join(","), "max_items")]
fn the_types_parameter_splits_at_commas_and_answers_422(#[case] raw: &str, #[case] code: &str) {
    let problem = parse_types(raw).unwrap_err();

    assert_eq!(violation(&problem)["code"], code);
}

#[test]
fn the_types_parameter_gives_the_distinct_names() {
    assert_eq!(
        parse_types("user.created,order.paid,user.created"),
        Ok(event_types(&["order.paid", "user.created"]))
    );
    assert_eq!(parse_types(&distinct(100).join(",")).unwrap().len(), 100);
}

#[rstest]
#[case::pattern("User.Created")]
#[case::too_long(&"x".repeat(129))]
#[case::blank_among("first.ok,,Secret.Name")]
fn a_problem_on_types_never_holds_a_name_of_the_request(#[case] raw: &str) {
    let problem = parse_types(raw).unwrap_err();

    let text = serde_json::to_string(&problem).unwrap();
    for name in raw.split(',').filter(|name| !name.is_empty()) {
        assert!(!text.contains(name), "{text} holds {name}");
    }
}

const LAST_EVENT_ID: HeaderName = HeaderName::from_static("last-event-id");

fn header(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(LAST_EVENT_ID, HeaderValue::from_str(value).unwrap());
    headers
}

fn bad_request(problem: &Problem) -> &str {
    assert_eq!(problem.status, 400);
    assert_eq!(problem.r#type, "about:blank");
    assert_eq!(problem.title, "Bad Request");
    assert_eq!(problem.violations, Vec::<Violation>::new());
    problem.detail.as_deref().unwrap()
}

#[test]
fn without_a_carrier_there_is_no_cursor() {
    assert_eq!(parse_cursor(&HeaderMap::new(), None), Ok(None));
}

#[rstest]
#[case::zero("0", 0)]
#[case::number("950", 950)]
#[case::leading_zeros("007", 7)]
#[case::largest("18446744073709551615", u64::MAX)]
fn each_carrier_reads_a_decimal_u64(#[case] text: &str, #[case] expected: u64) {
    let cursor = Some(Cursor::new(expected));

    assert_eq!(parse_cursor(&header(text), None), Ok(cursor));
    assert_eq!(parse_cursor(&HeaderMap::new(), Some(text)), Ok(cursor));
}

#[rstest]
#[case::both_valid(Some("5"))]
#[case::parameter_invalid(Some("abc"))]
#[case::parameter_absent(None)]
fn the_header_wins_over_the_parameter(#[case] parameter: Option<&str>) {
    assert_eq!(
        parse_cursor(&header("8"), parameter),
        Ok(Some(Cursor::new(8)))
    );
}

#[rstest]
#[case::text("abc")]
#[case::negative("-1")]
#[case::plus("+1")]
#[case::blank_inside("1 2")]
#[case::fraction("1.5")]
#[case::empty("")]
#[case::overflow("18446744073709551616")]
fn an_invalid_header_is_400_naming_the_header(#[case] text: &str) {
    let problem = parse_cursor(&header(text), Some("5")).unwrap_err();

    assert_eq!(
        bad_request(&problem),
        "Last-Event-ID must be a decimal unsigned 64-bit integer"
    );
}

#[test]
fn a_header_that_is_not_text_is_400_naming_the_header() {
    let mut headers = HeaderMap::new();
    headers.insert(LAST_EVENT_ID, HeaderValue::from_bytes(b"\xff1").unwrap());

    let problem = parse_cursor(&headers, None).unwrap_err();

    assert_eq!(
        bad_request(&problem),
        "Last-Event-ID must be a decimal unsigned 64-bit integer"
    );
}

#[rstest]
#[case::text("abc")]
#[case::negative("-1")]
#[case::empty("")]
#[case::overflow("18446744073709551616")]
fn an_invalid_parameter_is_400_naming_the_parameter(#[case] text: &str) {
    let problem = parse_cursor(&HeaderMap::new(), Some(text)).unwrap_err();

    assert_eq!(
        bad_request(&problem),
        "lastEventId must be a decimal unsigned 64-bit integer"
    );
    assert_eq!(
        StatusCode::from_u16(problem.status).unwrap(),
        StatusCode::BAD_REQUEST
    );
}

/// The fault of the `Last-Event-ID` header.
const HEADER_FAULT: CursorFault = CursorFault {
    carrier: "Last-Event-ID",
};

/// The fault of the `lastEventId` parameter.
const PARAMETER_FAULT: CursorFault = CursorFault {
    carrier: "lastEventId",
};

const HEADER_MESSAGE: &str = "Last-Event-ID must be a decimal unsigned 64-bit integer";
const PARAMETER_MESSAGE: &str = "lastEventId must be a decimal unsigned 64-bit integer";

/// The parameters a faulty header is read with: none, a valid one and a malformed one.
const ANY_PARAMETER: [Option<&str>; 3] = [None, Some("5"), Some("abc")];

fn header_value(text: &str) -> HeaderValue {
    HeaderValue::from_str(text).unwrap()
}

#[test]
fn read_cursor_answers_none_without_a_carrier() {
    assert_eq!(read_cursor(None, None), Ok(None));
}

#[rstest]
#[case::zero("0", 0)]
#[case::one("1", 1)]
#[case::largest("18446744073709551615", u64::MAX)]
fn read_cursor_answers_the_cursor_of_the_carrier_given(#[case] text: &str, #[case] expected: u64) {
    let cursor = Ok(Some(Cursor::new(expected)));

    assert_eq!(read_cursor(Some(&header_value(text)), None), cursor);
    assert_eq!(read_cursor(None, Some(text)), cursor);
}

#[rstest]
#[case::parameter_valid(Some("5"))]
#[case::parameter_malformed(Some("abc"))]
#[case::parameter_absent(None)]
fn read_cursor_answers_the_header_and_never_reads_the_parameter(#[case] parameter: Option<&str>) {
    assert_eq!(
        read_cursor(Some(&header_value("8")), parameter),
        Ok(Some(Cursor::new(8)))
    );
}

#[rstest]
#[case::text("abc")]
#[case::negative("-1")]
#[case::plus("+1")]
#[case::leading_blank(" 1")]
#[case::empty("")]
#[case::overflow("18446744073709551616")]
fn a_faulty_header_is_the_header_fault_whatever_the_parameter_holds(#[case] text: &str) {
    let header = header_value(text);

    for parameter in ANY_PARAMETER {
        assert_eq!(read_cursor(Some(&header), parameter), Err(HEADER_FAULT));
    }
}

#[test]
fn a_header_that_is_not_visible_ascii_text_is_the_header_fault() {
    let header = HeaderValue::from_bytes(b"\xff1").unwrap();

    for parameter in ANY_PARAMETER {
        assert_eq!(read_cursor(Some(&header), parameter), Err(HEADER_FAULT));
    }
}

#[rstest]
#[case::text("abc")]
#[case::negative("-1")]
#[case::plus("+1")]
#[case::leading_blank(" 1")]
#[case::empty("")]
#[case::overflow("18446744073709551616")]
#[case::not_ascii("\u{ff}1")]
fn a_faulty_parameter_without_a_header_is_the_parameter_fault(#[case] text: &str) {
    assert_eq!(read_cursor(None, Some(text)), Err(PARAMETER_FAULT));
}

#[rstest]
#[case::header(HEADER_FAULT, HEADER_MESSAGE)]
#[case::parameter(PARAMETER_FAULT, PARAMETER_MESSAGE)]
fn each_cursor_fault_message_names_its_carrier(#[case] fault: CursorFault, #[case] expected: &str) {
    assert_eq!(fault.message(), expected);
}

#[rstest]
#[case::text("abc")]
#[case::negative("-1")]
#[case::overflow("18446744073709551616")]
fn a_cursor_fault_message_never_holds_the_value(#[case] text: &str) {
    let header = read_cursor(Some(&header_value(text)), None).unwrap_err();
    let parameter = read_cursor(None, Some(text)).unwrap_err();

    assert_eq!(header.message(), HEADER_MESSAGE);
    assert_eq!(parameter.message(), PARAMETER_MESSAGE);
    assert!(!header.message().contains(text));
    assert!(!parameter.message().contains(text));
}

#[rstest]
#[case::header(HEADER_FAULT, HEADER_MESSAGE)]
#[case::parameter(PARAMETER_FAULT, PARAMETER_MESSAGE)]
fn each_cursor_fault_is_the_400_problem_naming_its_carrier(
    #[case] fault: CursorFault,
    #[case] detail: &str,
) {
    assert_eq!(bad_request(&fault.bad_request()), detail);
}
