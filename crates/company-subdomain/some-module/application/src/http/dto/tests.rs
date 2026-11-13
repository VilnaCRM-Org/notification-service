use proptest::prelude::*;
use rstest::rstest;
use serde_json::{Value, json};
use shared_domain::{PageRequest, Uuid};
use some_module_domain::{ExampleApiResourceDescription, ExampleApiResourceName};
use utoipa::ToSchema as _;
use validator::Validate as _;

use super::{
    CreateExampleApiResourceRequest, ExampleApiResourceResponse, ListExampleApiResourcesParams,
};
use crate::query::ExampleApiResourceView;

fn request(name: String, description: String) -> CreateExampleApiResourceRequest {
    CreateExampleApiResourceRequest { name, description }
}

fn params(page: i32, items_per_page: i32) -> ListExampleApiResourcesParams {
    ListExampleApiResourcesParams {
        page,
        items_per_page,
    }
}

fn schema_of<T: utoipa::PartialSchema>() -> Value {
    serde_json::to_value(T::schema()).unwrap()
}

/// Strings around both limits, with and without U+0000, and arbitrary text.
fn text(limit: usize) -> impl Strategy<Value = String> {
    let letters = prop::collection::vec(
        prop_oneof![
            9 => any::<char>(),
            1 => Just('\0'),
            1 => Just('\u{1F600}'),
        ],
        0..=limit.saturating_add(8),
    )
    .prop_map(|characters| characters.into_iter().collect::<String>());
    let around_the_limit = (
        limit.saturating_sub(1)..=limit.saturating_add(1),
        prop::option::of(0..=limit),
    )
        .prop_map(move |(length, nul_at)| {
            let mut characters = vec!['a'; length];
            if let Some(position) = nul_at.filter(|position| *position < length) {
                characters[position] = '\0';
            }
            characters.into_iter().collect::<String>()
        });
    prop_oneof![letters, around_the_limit]
}

proptest! {
    #[test]
    fn the_dto_and_the_name_value_object_accept_the_same_strings(value in text(255)) {
        let dto = request(value.clone(), "description".to_owned()).validate().is_ok();
        let domain = ExampleApiResourceName::new(value).is_ok();

        prop_assert_eq!(dto, domain);
    }

    #[test]
    fn the_dto_and_the_description_value_object_accept_the_same_strings(value in text(1024)) {
        let dto = request("name".to_owned(), value.clone()).validate().is_ok();
        let domain = ExampleApiResourceDescription::new(value).is_ok();

        prop_assert_eq!(dto, domain);
    }
}

#[rstest]
#[case::empty_name("", "d", "name", "length")]
#[case::long_name(&"a".repeat(256), "d", "name", "length")]
#[case::nul_in_name("a\0b", "d", "name", "does_not_contain")]
#[case::empty_description("n", "", "description", "length")]
#[case::long_description("n", &"a".repeat(1025), "description", "length")]
#[case::nul_in_description("n", "a\0b", "description", "does_not_contain")]
fn the_dto_reports_the_code_and_the_message_of_the_domain_error(
    #[case] name: &str,
    #[case] description: &str,
    #[case] property: &str,
    #[case] code: &str,
) {
    let errors = request(name.to_owned(), description.to_owned())
        .validate()
        .unwrap_err();

    let failed = &errors.field_errors()[property];
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].code, code);
    let domain_message = match property {
        "name" => ExampleApiResourceName::new(name.to_owned())
            .unwrap_err()
            .to_string(),
        _ => ExampleApiResourceDescription::new(description.to_owned())
            .unwrap_err()
            .to_string(),
    };
    assert_eq!(failed[0].message.as_deref(), Some(domain_message.as_str()));
}

#[test]
fn the_dto_accepts_whitespace_and_counts_scalar_values() {
    let value = "\u{1F600}".repeat(255);

    assert!(request(value, "   ".to_owned()).validate().is_ok());
}

#[test]
fn the_request_schema_carries_the_numbers_of_the_validator_attributes() {
    let schema = schema_of::<CreateExampleApiResourceRequest>();

    assert_eq!(schema["additionalProperties"], json!(false));
    assert_eq!(schema["required"], json!(["name", "description"]));
    assert_eq!(
        schema["properties"]["name"],
        json!({
            "type": "string",
            "description": "Display name of the resource.",
            "minLength": 1,
            "maxLength": 255,
            "pattern": "^[^\\u0000]*$",
            "example": "Example name"
        })
    );
    assert_eq!(
        schema["properties"]["description"],
        json!({
            "type": "string",
            "description": "Free-text description of the resource.",
            "minLength": 1,
            "maxLength": 1024,
            "pattern": "^[^\\u0000]*$",
            "example": "Example description"
        })
    );
}

#[test]
fn the_response_schema_is_named_example_api_resource() {
    let schema = schema_of::<ExampleApiResourceResponse>();

    assert_eq!(ExampleApiResourceResponse::name(), "ExampleApiResource");
    assert_eq!(schema["required"], json!(["id", "name", "description"]));
    assert_eq!(
        schema["properties"]["id"],
        json!({"type": "string", "format": "uuid", "description": "Identifier of the resource."})
    );
}

#[rstest]
#[case("name", 255)]
#[case("description", 1024)]
fn the_response_schema_carries_the_constraints_of_the_domain(
    #[case] property: &str,
    #[case] max_length: u64,
) {
    let schema = schema_of::<ExampleApiResourceResponse>();

    let property = &schema["properties"][property];
    assert_eq!(property["minLength"], 1);
    assert_eq!(property["maxLength"], max_length);
    assert_eq!(property["pattern"], "^[^\\u0000]*$");
}

#[test]
fn the_request_rejects_unknown_fields_and_missing_fields() {
    let unknown = serde_json::from_value::<CreateExampleApiResourceRequest>(
        json!({"name": "n", "description": "d", "extra": 1}),
    );
    let missing = serde_json::from_value::<CreateExampleApiResourceRequest>(json!({"name": "n"}));

    assert!(unknown.is_err());
    assert!(missing.is_err());
}

#[test]
fn the_response_is_built_from_the_view_and_serializes_to_the_three_members() {
    let id = Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90").unwrap();
    let view = ExampleApiResourceView {
        id,
        name: "Name".to_owned(),
        description: "Description".to_owned(),
    };

    let response = ExampleApiResourceResponse::from(view);

    assert_eq!(response.id, id.as_uuid());
    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        json!({
            "id": "0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90",
            "name": "Name",
            "description": "Description"
        })
    );
}

#[test]
fn the_parameters_default_to_the_first_page_of_30() {
    let parsed: ListExampleApiResourcesParams = serde_json::from_value(json!({})).unwrap();

    assert_eq!((parsed.page, parsed.items_per_page), (1, 30));
}

#[test]
fn the_parameters_read_the_camel_case_names() {
    let parsed: ListExampleApiResourcesParams =
        serde_json::from_value(json!({"page": 4, "itemsPerPage": 9})).unwrap();

    assert_eq!((parsed.page, parsed.items_per_page), (4, 9));
}

#[rstest]
#[case(1, 1)]
#[case(2, 30)]
#[case(i32::MAX, 100)]
fn the_parameters_accept_the_values_inside_their_bounds(#[case] page: i32, #[case] items: i32) {
    assert!(params(page, items).validate().is_ok());
}

#[rstest]
#[case::page_zero(0, 30, "page", "This value must be 1 or greater.")]
#[case::page_negative(-1, 30, "page", "This value must be 1 or greater.")]
#[case::page_minimum(i32::MIN, 30, "page", "This value must be 1 or greater.")]
#[case::no_items(1, 0, "items_per_page", "This value must be between 1 and 100.")]
#[case::too_many_items(1, 101, "items_per_page", "This value must be between 1 and 100.")]
#[case::negative_items(1, -5, "items_per_page", "This value must be between 1 and 100.")]
fn the_parameters_reject_the_values_outside_their_bounds(
    #[case] page: i32,
    #[case] items: i32,
    #[case] property: &str,
    #[case] message: &str,
) {
    let errors = params(page, items).validate().unwrap_err();

    let failed = &errors.field_errors()[property];
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].code, "range");
    assert_eq!(failed[0].message.as_deref(), Some(message));
}

#[rstest]
#[case(1, 30)]
#[case(7, 100)]
#[case(i32::MAX, 1)]
fn the_query_carries_both_values(#[case] page: i32, #[case] items: i32) {
    let query = params(page, items).into_query();

    assert_eq!(i64::from(query.page), i64::from(page));
    assert_eq!(i64::from(query.items_per_page), i64::from(items));
}

#[test]
fn the_default_parameters_are_the_page_request_defaults() {
    let query = ListExampleApiResourcesParams::default().into_query();

    assert_eq!(query.page, PageRequest::DEFAULT_PAGE);
    assert_eq!(query.items_per_page, PageRequest::DEFAULT_ITEMS_PER_PAGE);
}

#[test]
fn the_items_per_page_bound_is_the_page_request_maximum() {
    let largest = i32::try_from(PageRequest::MAX_ITEMS_PER_PAGE).unwrap();

    assert!(params(1, largest).validate().is_ok());
    assert!(params(1, largest + 1).validate().is_err());
    assert_eq!(
        params(1, largest).into_query().items_per_page,
        PageRequest::MAX_ITEMS_PER_PAGE
    );
}
