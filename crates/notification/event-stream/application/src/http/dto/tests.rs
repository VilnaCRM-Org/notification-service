use rstest::rstest;
use serde_json::{Value, json};
use shared_domain::PageRequest;
use utoipa::{PartialSchema as _, ToSchema as _};
use validator::Validate as _;

use super::{EventTypeResponse, ListEventTypesParams};
use crate::query::EventTypeView;

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
