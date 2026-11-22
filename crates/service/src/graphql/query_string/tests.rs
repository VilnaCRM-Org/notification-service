use axum::http::StatusCode;
use rstest::rstest;
use serde_json::{Map, Value, json};
use shared_application::http::{Problem, TYPE_BLANK, TYPE_VALIDATION_FAILED, Violation};
use test_support::log::{LogLine, LogRecorder};
use tracing::Level;

use super::{QueryStringLimit, limit, read};
use crate::graphql::schema::builder;

/// The bound of the tests that are not about the bound.
const MAX: usize = 4096;

/// The text of a query string that cannot be decoded.
const UNDECODABLE: &str = "Failed to deserialize query string: invalid percent-encoding or UTF-8";

/// The map `read` answers for `raw`, which must be readable.
fn map_of(raw: &str) -> Map<String, Value> {
    read(Some(raw), MAX).unwrap_or_else(|problem| panic!("{raw}: {problem:?}"))
}

/// The problem `read` answers for `raw`, which must be refused.
fn problem_of(raw: &str) -> Problem {
    read(Some(raw), MAX).expect_err(raw)
}

/// The JSON object of `value`.
fn object(value: &Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

/// The 400 problem with `detail`.
fn bad_request(detail: &str) -> Problem {
    Problem {
        r#type: TYPE_BLANK,
        title: "Bad Request",
        status: 400,
        detail: Some(detail.to_owned()),
        violations: Vec::new(),
        failures: Vec::new(),
    }
}

/// The 422 problem whose one violation names `name`.
fn not_an_object(name: &str) -> Problem {
    Problem {
        r#type: TYPE_VALIDATION_FAILED,
        title: "Unprocessable Entity",
        status: 422,
        detail: Some("The request failed validation.".to_owned()),
        violations: vec![Violation {
            property_path: name.to_owned(),
            message: format!("{name} must be a JSON-encoded object"),
            code: "format".to_owned(),
        }],
        failures: Vec::new(),
    }
}

/// The 414 problem.
fn too_long() -> Problem {
    Problem {
        r#type: TYPE_BLANK,
        title: "URI Too Long",
        status: 414,
        detail: Some("The query string is too long.".to_owned()),
        violations: Vec::new(),
        failures: Vec::new(),
    }
}

#[test]
fn the_limit_is_read_from_the_schema_data() {
    let schema = builder().data(QueryStringLimit { max_bytes: 7 }).finish();

    assert_eq!(limit(&schema), Ok(QueryStringLimit { max_bytes: 7 }));
}

#[test]
fn a_schema_without_the_limit_is_the_500_problem_logged_once() {
    let schema = builder().finish();
    let (recorder, guard) = LogRecorder::install();

    let answer = limit(&schema);
    drop(guard);

    assert_eq!(answer, Err(Problem::new(StatusCode::INTERNAL_SERVER_ERROR)));
    assert_eq!(
        recorder.lines(),
        vec![LogLine {
            level: Level::ERROR,
            target: "shared_application::http::problem".to_owned(),
            message: "request failed".to_owned(),
            fields: vec![(
                "error".to_owned(),
                "the GraphQL schema has no QueryStringLimit data for the GET form".to_owned()
            )],
        }]
    );
}

#[test]
fn every_parameter_becomes_the_member_the_post_body_would_carry() {
    let raw = "query=subscription+%7B+events(types%3A+%5B%22a%22%5D)+%7B+id+%7D+%7D\
        &operationName=Op&variables=%7B%22a%22%3A1%2C%22b%22%3A%5Btrue%5D%7D\
        &extensions=%7B%22e%22%3A%7B%7D%7D&unknown=1";

    let map = map_of(raw);

    assert_eq!(
        Value::Object(map),
        json!({
            "query": r#"subscription { events(types: ["a"]) { id } }"#,
            "operationName": "Op",
            "variables": { "a": 1, "b": [true] },
            "extensions": { "e": {} },
        })
    );
}

#[test]
fn a_query_alone_is_the_one_member() {
    assert_eq!(
        map_of("query=%7B__typename%7D"),
        object(&json!({ "query": "{__typename}" }))
    );
}

/// The members do not depend on the order of the parameters. Their order in the map is not
/// compared: `to_request` reads the members by name.
#[test]
fn each_parameter_reaches_its_own_member_whatever_the_order_of_the_parameters() {
    let map =
        map_of("extensions=%7B%22e%22%3A2%7D&variables=%7B%22v%22%3A1%7D&operationName=A&query=q");

    assert_eq!(
        Value::Object(map),
        json!({
            "query": "q",
            "operationName": "A",
            "variables": { "v": 1 },
            "extensions": { "e": 2 },
        })
    );
}

#[rstest]
#[case::a_plus_is_a_space("query=a+b", "a b")]
#[case::an_encoded_plus_is_a_plus("query=a%2Bb", "a+b")]
#[case::an_encoded_space("query=a%20b", "a b")]
#[case::upper_case_hex("query=%C3%A9", "\u{e9}")]
#[case::lower_case_hex("query=%c3%a9", "\u{e9}")]
#[case::mixed_case_hex("query=%c3%A9", "\u{e9}")]
#[case::every_hex_digit("query=%01%23%45%67%7f%3a%3B%3c%3D%3e%3F", "\u{1}#Eg\u{7f}:;<=>?")]
#[case::a_supplementary_character("query=%F0%9F%98%80", "\u{1f600}")]
#[case::an_encoded_ampersand_and_equals_sign("query=a%26b%3Dc", "a&b=c")]
#[case::an_equals_sign_in_the_value("query=a=b", "a=b")]
#[case::no_equals_sign("query", "")]
#[case::an_empty_value("query=", "")]
#[case::an_encoded_name("%71uery=x", "x")]
#[case::a_name_with_a_plus_is_another_name("query=x&query+=y", "x")]
#[case::empty_pieces_are_skipped("&&query=x&&", "x")]
#[case::unencoded_text("query={ __typename }", "{ __typename }")]
fn each_name_and_value_is_decoded(#[case] raw: &str, #[case] query: &str) {
    assert_eq!(map_of(raw)["query"], query, "{raw}");
}

#[test]
fn the_operation_name_is_text_as_received() {
    let map = map_of("query=q&operationName=");

    assert_eq!(map["operationName"], "");
    assert_eq!(map_of("query=q&operationName=a+b")["operationName"], "a b");
}

#[rstest]
#[case::no_query_string(None)]
#[case::an_empty_query_string(Some(""))]
#[case::only_empty_pieces(Some("&&"))]
#[case::other_parameters(Some("operationName=A&variables=%7B%7D"))]
#[case::a_name_in_another_case(Some("Query=x"))]
#[case::a_name_with_a_space(Some("query+=x"))]
fn a_query_string_without_query_is_the_400_problem(#[case] raw: Option<&str>) {
    assert_eq!(
        read(raw, MAX),
        Err(bad_request(
            "Failed to deserialize query string: missing field `query`"
        )),
        "{raw:?}"
    );
}

#[rstest]
#[case::query("query=a&query=b", "query")]
#[case::query_given_empty_twice("query&query=", "query")]
#[case::operation_name("query=a&operationName=x&operationName=y", "operationName")]
#[case::variables("variables=%7B%7D&query=a&variables=%7B%7D", "variables")]
#[case::extensions("query=a&extensions=1&extensions=2", "extensions")]
#[case::an_encoded_name("query=a&%71uery=b", "query")]
fn a_known_parameter_given_twice_is_the_400_problem(#[case] raw: &str, #[case] name: &str) {
    assert_eq!(
        problem_of(raw),
        bad_request(&format!(
            "Failed to deserialize query string: duplicate field `{name}`"
        )),
        "{raw}"
    );
}

#[test]
fn an_unknown_parameter_is_ignored_also_when_repeated_and_never_echoed() {
    let map = map_of("secret=1&query=a&secret=2&secret");

    assert_eq!(map, object(&json!({ "query": "a" })));
    let refused = problem_of("secret=1&secret=2");
    assert!(!refused.detail.unwrap().contains("secret"));
}

#[rstest]
#[case::a_bad_digit("%zz")]
#[case::a_bad_second_digit("%4g")]
#[case::a_bad_first_digit("%g4")]
#[case::a_percent_at_the_end("%")]
#[case::one_digit_at_the_end("%4")]
#[case::a_plus_as_first_digit("%+1")]
#[case::a_plus_as_second_digit("%1+")]
#[case::a_sign_before_the_digits("%-1")]
#[case::invalid_utf_8("%ff")]
#[case::a_truncated_sequence("%C3")]
#[case::an_encoded_surrogate("%ED%A0%80")]
fn an_undecodable_value_of_a_known_parameter_is_the_400_problem_that_names_it(#[case] value: &str) {
    for name in ["query", "operationName", "variables", "extensions"] {
        let raw = format!("{name}={value}");

        assert_eq!(
            problem_of(&raw),
            bad_request(&format!("{UNDECODABLE} in `{name}`")),
            "{raw}"
        );
    }
}

#[rstest]
#[case::a_bad_name("%zz")]
#[case::a_bad_name_with_a_value("%zz=1&query=a")]
#[case::a_name_with_a_percent_at_the_end("query%=a")]
#[case::a_name_of_invalid_utf_8("%ff=a&query=a")]
#[case::a_bad_value_of_an_unknown_parameter("query=a&secret=%zz")]
#[case::a_bad_value_of_a_repeated_unknown_parameter("secret=1&secret=%ff&query=a")]
fn an_undecodable_name_or_unknown_value_is_the_400_problem_without_a_name(#[case] raw: &str) {
    assert_eq!(problem_of(raw), bad_request(UNDECODABLE), "{raw}");
}

#[rstest]
#[case::an_array("[]")]
#[case::a_number("1")]
#[case::a_string("%22x%22")]
#[case::null("null")]
#[case::a_boolean("true")]
#[case::text_that_is_not_json("x")]
#[case::an_unterminated_object("%7B")]
#[case::an_object_and_more("%7B%7D%7B%7D")]
#[case::empty("")]
fn variables_or_extensions_that_are_not_a_json_object_are_the_422_problem(#[case] value: &str) {
    for name in ["variables", "extensions"] {
        let raw = format!("query=a&{name}={value}");

        assert_eq!(problem_of(&raw), not_an_object(name), "{raw}");
    }
}

#[test]
fn variables_are_checked_before_extensions() {
    assert_eq!(
        problem_of("query=a&extensions=1&variables=1"),
        not_an_object("variables")
    );
    assert_eq!(
        problem_of("query=a&extensions=1&variables=%7B%7D"),
        not_an_object("extensions")
    );
}

/// A query string of exactly `length` bytes that is readable.
fn readable_of(length: usize) -> String {
    let padding = length.checked_sub("query=".len()).unwrap();
    format!("query={}", "a".repeat(padding))
}

#[rstest]
#[case::one_byte(7)]
#[case::the_default_body_limit(1_048_576)]
fn a_query_string_of_the_bound_is_read_and_one_byte_more_is_the_414_problem(
    #[case] max_bytes: usize,
) {
    let at_bound = readable_of(max_bytes);
    let above = readable_of(max_bytes.checked_add(1).unwrap());

    let read_at_bound = read(Some(&at_bound), max_bytes).unwrap();

    assert_eq!(
        read_at_bound["query"].as_str().map(str::len),
        max_bytes.checked_sub(6)
    );
    assert_eq!(read(Some(&above), max_bytes), Err(too_long()));
}

#[test]
fn the_bound_counts_the_bytes_as_received() {
    // Nine bytes as received, three once decoded.
    let raw = "query=%41%42%43";

    assert_eq!(
        read(Some(raw), raw.len().checked_sub(1).unwrap()),
        Err(too_long())
    );
    assert_eq!(read(Some(raw), raw.len()).unwrap()["query"], "ABC");
}

#[test]
fn the_414_answers_before_every_other_rule() {
    for raw in [
        "%zz",
        "query=a&query=b",
        "variables=1",
        "query=a&variables=1",
    ] {
        assert_eq!(
            read(Some(raw), raw.len().checked_sub(1).unwrap()),
            Err(too_long()),
            "{raw}"
        );
    }
}

#[rstest]
#[case::an_undecodable_value_before_an_invalid_object(
    "query=a&variables=1&extensions=%zz",
    "invalid percent-encoding or UTF-8 in `extensions`"
)]
#[case::a_duplicate_before_an_invalid_object(
    "variables=1&query=a&query=b",
    "duplicate field `query`"
)]
#[case::a_missing_query_before_an_invalid_object(
    "variables=1&extensions=1",
    "missing field `query`"
)]
#[case::the_first_failing_piece_wins(
    "query=%zz&query=a",
    "invalid percent-encoding or UTF-8 in `query`"
)]
#[case::a_duplicate_before_a_later_undecodable_piece(
    "query=a&query=b&secret=%zz",
    "duplicate field `query`"
)]
#[case::an_undecodable_repetition(
    "query=a&query=%zz",
    "invalid percent-encoding or UTF-8 in `query`"
)]
fn the_400_answers_before_the_422_and_in_the_order_of_the_pieces(
    #[case] raw: &str,
    #[case] text: &str,
) {
    assert_eq!(
        problem_of(raw),
        bad_request(&format!("Failed to deserialize query string: {text}")),
        "{raw}"
    );
}
