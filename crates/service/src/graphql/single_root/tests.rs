use async_graphql::{Request, ServerError};
use rstest::rstest;
use serde_json::{Value, json};

use super::{breach, check};

/// The message of the one violation of a breach.
const RULE: &str = "a subscription operation must select exactly one root field";

/// The answer of `check` to `document`, run as the operation `operation_name` names.
fn checked(document: &str, operation_name: Option<&str>) -> Result<(), ServerError> {
    let mut request = Request::new(document);
    request.operation_name = operation_name.map(str::to_owned);
    check(&mut request)
}

/// The error of `check` for `document`, run as the operation `operation_name` names, as JSON.
fn refusal(document: &str, operation_name: Option<&str>) -> Value {
    let error = checked(document, operation_name).expect_err("the rule refuses the operation");
    serde_json::to_value(error).unwrap()
}

/// The error of a breach at `line` and `column`: no path, the `VALIDATION_FAILED` code and the
/// one violation of the rule.
fn breach_at(line: usize, column: usize) -> Value {
    json!({
        "message": "The request failed validation.",
        "locations": [{"line": line, "column": column}],
        "extensions": {
            "code": "VALIDATION_FAILED",
            "violations": [{"propertyPath": "query", "message": RULE, "code": "single_root_field"}],
        },
    })
}

/// The 1-based column of the first `marker` in the one-line `document`.
fn column_of(document: &str, marker: &str) -> usize {
    document
        .find(marker)
        .unwrap_or_else(|| panic!("{marker:?} in {document:?}"))
        .saturating_add(1)
}

// ---- operations that satisfy the rule ----

#[rstest]
#[case::one_field(r#"subscription { events(types: ["a"]) { id type data } }"#)]
#[case::one_aliased_field(r#"subscription { feed: events(types: ["a"]) { id } }"#)]
#[case::fragments_inside_the_field(
    r#"subscription { events(types: ["a"]) { ...F ... on Event { type } } } fragment F on Event { id }"#
)]
#[case::conditional_directives_inside_the_field(
    r#"subscription { events(types: ["a"]) { id @skip(if: true) type @include(if: false) } }"#
)]
#[case::another_directive_at_the_root(r#"subscription { events(types: ["a"]) @live { id } }"#)]
#[case::a_named_operation(r#"subscription Feed { events(types: ["a"]) { id } }"#)]
#[case::a_field_that_is_not_events(r#"subscription { unknownField(types: ["a"]) { id } }"#)]
#[case::a_field_whose_name_holds_two_underscores_later("subscription { e__vents { id } }")]
fn a_subscription_with_one_direct_root_field_passes(#[case] document: &str) {
    assert!(checked(document, None).is_ok(), "{document}");
}

#[rstest]
#[case::a_query_with_two_root_fields("{ a: eventTypes { name } b: eventTypes { name } }")]
#[case::a_query_with_a_root_fragment("query { ...F } fragment F on Query { eventTypes { name } }")]
#[case::a_query_of_the_typename("{ __typename }")]
#[case::a_mutation_with_two_root_fields(
    "mutation { a: eventTypes { name } b: eventTypes { name } }"
)]
#[case::a_document_that_does_not_parse(r#"subscription { a: events(types: ["a"]) b: events"#)]
#[case::an_empty_document("")]
fn a_query_a_mutation_or_a_document_that_does_not_parse_passes_through(#[case] document: &str) {
    assert!(checked(document, None).is_ok(), "{document}");
}

// ---- breaches, each at the selection that breaks the rule ----

#[rstest]
#[case::two_fields(
    r#"subscription { a: events(types: ["a"]) { id } b: events(types: ["b"]) { id } }"#,
    "b: events"
)]
#[case::the_same_response_name_twice(
    r#"subscription { events(types: ["a"]) { id } events(types: ["a"]) { type } }"#,
    "events(types: [\"a\"]) { type }"
)]
#[case::three_fields_at_the_second(
    r#"subscription { a: events(types: ["a"]) { id } b: events(types: ["a"]) { id } c: events(types: ["a"]) { id } }"#,
    "b: events"
)]
#[case::a_root_fragment_spread(
    r#"subscription { ...F } fragment F on Subscription { events(types: ["a"]) { id } }"#,
    "...F"
)]
#[case::a_root_inline_fragment(
    r#"subscription { ... on Subscription { events(types: ["a"]) { id } } }"#,
    "... on"
)]
#[case::a_root_inline_fragment_without_a_type(
    r#"subscription { ... { events(types: ["a"]) { id } } }"#,
    "... {"
)]
#[case::the_typename_alone("subscription { __typename }", "__typename")]
#[case::the_typename_aliased("subscription { name: __typename }", "name:")]
#[case::another_introspection_field("subscription { __schema { queryType { name } } }", "__schema")]
#[case::the_typename_after_events(
    r#"subscription { events(types: ["a"]) { id } __typename }"#,
    "__typename"
)]
#[case::the_typename_before_events(
    r#"subscription { __typename events(types: ["a"]) { id } }"#,
    "events"
)]
#[case::a_skip_directive(
    r#"subscription { events(types: ["a"]) @skip(if: false) { id } }"#,
    "events"
)]
#[case::an_include_directive(
    r#"subscription { events(types: ["a"]) @include(if: true) { id } }"#,
    "events"
)]
#[case::an_include_directive_on_an_aliased_field(
    r#"subscription { feed: events(types: ["a"]) @include(if: true) { id } }"#,
    "feed:"
)]
#[case::a_skip_directive_after_another_directive(
    r#"subscription { events(types: ["a"]) @live @skip(if: false) { id } }"#,
    "events"
)]
#[case::a_breach_that_also_fails_validation(
    r#"subscription { events(types: ["a"]) { nope } unknownField }"#,
    "unknownField"
)]
fn each_breach_is_the_validation_error_at_the_selection_that_breaks_the_rule(
    #[case] document: &str,
    #[case] marker: &str,
) {
    assert_eq!(
        refusal(document, None),
        breach_at(1, column_of(document, marker))
    );
}

#[test]
fn the_location_counts_lines_and_columns_from_one() {
    let document =
        "subscription {\n  events(types: [\"a\"]) { id }\n  events(types: [\"a\"]) { type }\n}";

    assert_eq!(refusal(document, None), breach_at(3, 3));
}

#[test]
fn a_breach_carries_exactly_its_message_location_and_extensions() {
    let error = checked("subscription { __typename }", None).unwrap_err();

    assert_eq!(error.message, "The request failed validation.");
    assert_eq!(error.locations.len(), 1);
    assert_eq!(
        (error.locations[0].line, error.locations[0].column),
        (1, 16)
    );
    assert_eq!(error.path, Vec::new());
    assert!(error.source.is_none());
}

// ---- the operation that runs (the choice of async-graphql) ----

/// A subscription that satisfies the rule.
const VALID: &str = r#"subscription Valid { events(types: ["a"]) { id } }"#;

/// A subscription with two root fields.
const TWO_FIELDS: &str =
    r#"subscription TwoFields { a: events(types: ["a"]) { id } b: events(types: ["b"]) { id } }"#;

/// A subscription with a root fragment.
const FRAGMENT: &str = "subscription Fragment { ...F }";

/// The fragment of [`FRAGMENT`].
const FRAGMENT_F: &str = r#"fragment F on Subscription { events(types: ["a"]) { id } }"#;

/// A query with two root fields.
const QUERY: &str = "query Catalog { a: eventTypes { name } b: eventTypes { name } }";

#[test]
fn the_only_named_operation_runs_without_an_operation_name_and_is_checked() {
    assert_eq!(
        refusal(TWO_FIELDS, None),
        breach_at(1, column_of(TWO_FIELDS, "b: events"))
    );
}

#[test]
fn the_operation_that_operation_name_selects_is_the_one_checked() {
    let document = format!("{VALID} {TWO_FIELDS} {QUERY} {FRAGMENT} {FRAGMENT_F}");

    assert!(checked(&document, Some("Valid")).is_ok());
    assert!(checked(&document, Some("Catalog")).is_ok());
    assert_eq!(
        refusal(&document, Some("TwoFields")),
        breach_at(1, column_of(&document, "b: events"))
    );
    assert_eq!(
        refusal(&document, Some("Fragment")),
        breach_at(1, column_of(&document, "...F"))
    );
}

#[test]
fn a_named_operation_selected_by_its_own_name_is_checked() {
    assert_eq!(
        refusal(TWO_FIELDS, Some("TwoFields")),
        breach_at(1, column_of(TWO_FIELDS, "b: events"))
    );
}

#[rstest]
#[case::an_unknown_operation_name(&format!("{TWO_FIELDS} {FRAGMENT} {FRAGMENT_F}"), Some("Other"))]
#[case::the_only_named_operation_under_another_name(TWO_FIELDS, Some("Other"))]
#[case::an_anonymous_operation_with_an_operation_name(
    r#"subscription { a: events(types: ["a"]) { id } b: events(types: ["b"]) { id } }"#,
    Some("Anonymous")
)]
#[case::several_operations_that_all_break_without_an_operation_name(
    &format!("{TWO_FIELDS} {FRAGMENT} {FRAGMENT_F}"),
    None
)]
fn no_runnable_operation_passes_through(#[case] document: &str, #[case] name: Option<&str>) {
    assert!(checked(document, name).is_ok(), "{document} {name:?}");
}

#[test]
fn an_empty_root_selection_set_names_no_breach() {
    assert_eq!(breach(&[]), None);
}
