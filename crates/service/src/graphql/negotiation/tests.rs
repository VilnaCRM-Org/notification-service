use axum::http::header::ACCEPT;
use axum::http::{HeaderMap, HeaderValue};
use rstest::rstest;

use super::negotiates_event_stream;

/// The request headers with one `Accept` field per entry of `values`, in order.
fn accept(values: &[&str]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for value in values {
        headers.append(ACCEPT, HeaderValue::from_str(value).unwrap());
    }
    headers
}

/// The urql header of a query: the JSON types and the event stream at the same weight.
const URQL_QUERY: &str = "application/graphql-response+json, application/graphql+json, application/json, text/event-stream, multipart/mixed";

/// The header of a browser navigation.
const BROWSER_NAVIGATION: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

#[test]
fn a_request_without_accept_keeps_the_json_transport() {
    assert!(!negotiates_event_stream(&HeaderMap::new()));
}

/// The POST column of the table of FR-2.
#[rstest]
#[case::graphql_sse_and_event_source("text/event-stream", true)]
#[case::urql_subscription("text/event-stream, multipart/mixed", true)]
#[case::urql_query(URQL_QUERY, false)]
#[case::any_type("*/*", false)]
#[case::any_text_type("text/*", false)]
#[case::browser_navigation(BROWSER_NAVIGATION, false)]
#[case::json_clients("application/json", false)]
#[case::json_ranked_below("application/json;q=0.5, text/event-stream", true)]
#[case::event_stream_refused("text/event-stream;q=0", false)]
#[case::letter_case_and_a_parameter("TEXT/Event-Stream; charset=utf-8", true)]
#[case::an_invalid_qvalue("text/event-stream;q=2", false)]
fn the_post_rule_answers_the_table_of_fr_2(#[case] value: &str, #[case] expected: bool) {
    assert_eq!(
        negotiates_event_stream(&accept(&[value])),
        expected,
        "{value}"
    );
}

#[rstest]
#[case::upper_case("TEXT/EVENT-STREAM")]
#[case::mixed_case("TEXT/Event-Stream")]
#[case::spaces_around_the_range("  text/event-stream  ")]
#[case::spaces_around_the_parameter("text/event-stream ; q = 0.5 ")]
#[case::other_parameters_ignored("text/event-stream;charset=utf-8;level=1")]
#[case::a_parameter_after_the_weight("text/event-stream;q=0.5;level=1")]
#[case::an_empty_range_between_commas(",,text/event-stream,")]
fn the_event_stream_range_is_read_whatever_its_case_and_spacing(#[case] value: &str) {
    assert!(negotiates_event_stream(&accept(&[value])), "{value}");
}

/// The weights in thousandths: a valid qvalue has at most three decimals and lies in 0 to 1.
#[rstest]
#[case::zero("0", false)]
#[case::zero_with_a_point("0.", false)]
#[case::zero_with_three_zeros("0.000", false)]
#[case::the_smallest_weight("0.001", true)]
#[case::one_decimal("0.5", true)]
#[case::two_decimals("0.25", true)]
#[case::the_largest_fraction("0.999", true)]
#[case::one("1", true)]
#[case::one_with_a_point("1.", true)]
#[case::one_with_three_zeros("1.000", true)]
#[case::four_decimals_of_one("1.0001", false)]
#[case::four_decimals_of_zero("0.0001", false)]
#[case::four_decimals("0.1234", false)]
#[case::one_and_a_half("1.5", false)]
#[case::one_and_a_thousandth("1.001", false)]
#[case::two("2", false)]
#[case::a_leading_zero("00.5", false)]
#[case::no_whole_part(".5", false)]
#[case::a_sign_in_the_fraction("0.+5", false)]
#[case::a_sign_before("+0.5", false)]
#[case::a_letter("0.5a", false)]
#[case::empty("", false)]
fn a_weight_counts_only_when_it_is_a_valid_qvalue_above_zero(
    #[case] weight: &str,
    #[case] expected: bool,
) {
    let value = format!("text/event-stream;q={weight}");

    assert_eq!(
        negotiates_event_stream(&accept(&[&value])),
        expected,
        "{value}"
    );
}

#[rstest]
#[case::lower_case("text/event-stream;q=0")]
#[case::upper_case("text/event-stream;Q=0")]
#[case::a_weight_without_a_value("text/event-stream;q")]
#[case::an_upper_case_weight_without_a_value("text/event-stream;Q")]
fn the_weight_name_is_read_whatever_its_case(#[case] value: &str) {
    assert!(!negotiates_event_stream(&accept(&[value])), "{value}");
}

/// `s > j`, with the highest weight of each side.
#[rstest]
#[case::a_tie("text/event-stream;q=0.5, application/json;q=0.5", false)]
#[case::a_tie_with_the_graphql_type(
    "text/event-stream;q=0.5, application/graphql-response+json;q=0.5",
    false
)]
#[case::one_thousandth_above("text/event-stream;q=0.501, application/json;q=0.5", true)]
#[case::one_thousandth_below("text/event-stream;q=0.5, application/json;q=0.501", false)]
#[case::the_graphql_type_above(
    "text/event-stream;q=0.5, application/graphql-response+json;q=0.501",
    false
)]
#[case::the_highest_event_stream_weight(
    "text/event-stream;q=0.2, text/event-stream;q=0.9, application/json;q=0.5",
    true
)]
#[case::the_highest_json_weight(
    "application/json;q=0.9, application/json;q=0.2, text/event-stream;q=0.5",
    false
)]
#[case::the_highest_weight_of_both_json_types(
    "application/json;q=0.2, application/graphql-response+json;q=0.9, text/event-stream;q=0.5",
    false
)]
#[case::an_invalid_json_weight_ignores_its_range(
    "application/json;q=2, text/event-stream;q=0.5",
    true
)]
#[case::an_invalid_event_stream_weight_ignores_its_range(
    "text/event-stream;q=1.5, text/event-stream;q=0.1",
    true
)]
#[case::a_wildcard_does_not_count_as_json("text/event-stream;q=0.1, */*", true)]
#[case::a_text_wildcard_does_not_count_as_the_event_stream("text/*, application/json;q=0.1", false)]
#[case::the_graphql_json_type_alone("application/graphql+json, text/event-stream;q=0.5", true)]
#[case::other_types_count_for_neither("multipart/mixed, text/html", false)]
fn the_event_stream_wins_only_above_every_json_type(#[case] value: &str, #[case] expected: bool) {
    assert_eq!(
        negotiates_event_stream(&accept(&[value])),
        expected,
        "{value}"
    );
}

#[rstest]
#[case::json_then_event_stream(&["application/json;q=0.5", "text/event-stream"], true)]
#[case::event_stream_then_json(&["text/event-stream;q=0.5", "application/json"], false)]
#[case::two_event_stream_fields(&["text/event-stream;q=0", "text/event-stream;q=0.3"], true)]
fn every_accept_field_is_read_in_order(#[case] values: &[&str], #[case] expected: bool) {
    assert_eq!(
        negotiates_event_stream(&accept(values)),
        expected,
        "{values:?}"
    );
}

#[test]
fn a_field_that_is_not_visible_ascii_counts_as_absent() {
    let mut headers = HeaderMap::new();
    headers.append(
        ACCEPT,
        HeaderValue::from_bytes(b"text/event-stream, \xe9").unwrap(),
    );
    assert!(!negotiates_event_stream(&headers));

    headers.append(ACCEPT, HeaderValue::from_static("text/event-stream;q=0.2"));
    assert!(negotiates_event_stream(&headers));

    let mut json_hidden = HeaderMap::new();
    json_hidden.append(
        ACCEPT,
        HeaderValue::from_bytes(b"application/json, \xe9").unwrap(),
    );
    json_hidden.append(ACCEPT, HeaderValue::from_static("text/event-stream;q=0.2"));
    assert!(negotiates_event_stream(&json_hidden));
}
