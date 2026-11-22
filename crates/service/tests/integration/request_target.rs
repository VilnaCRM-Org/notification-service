//! The bounds of the query string of `GET /api/graphql` on a served binary, over HTTP/1.1 (FR-4,
//! maintainer decision M6): the service reads a query string up to the smaller of
//! `HTTP_MAX_BODY_BYTES` and 65 521 bytes. Its own 414 problem, with the CORS and `Vary` headers of
//! every answer, refuses a query string over a lowered `HTTP_MAX_BODY_BYTES`; the HTTP server
//! (hyper) refuses a request-target over 65 534 bytes itself, with a bare 414, before the service
//! sees the request; and a document and an operation name at their bounds of decision M5 fit that
//! query string in their widest encoding. These tests pin all three, so an upgrade of the HTTP
//! server that moves its limit fails here.

use serde_json::{Value, json};

use crate::support::{Answer, Finished, Scratch, messages_of, serve};

/// The origin that the served binaries allow, and that every request of these tests comes from.
const ORIGIN: &str = "https://app.example.com";

/// The configuration of a node whose broker does not answer (the Kafka broker on port 1, where
/// nothing listens) and that allows [`ORIGIN`]: a query that touches no dependency is still
/// answered.
const NODE: [(&str, &str); 2] = [
    ("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:1"),
    ("CORS_ALLOWED_ORIGINS", ORIGIN),
];

/// A lowered `HTTP_MAX_BODY_BYTES`, its smallest valid value.
const LOWERED_BOUND: usize = 1_024;

/// The longest request-target that the HTTP server lets through, in bytes: `u16::MAX - 1`.
const SERVER_TARGET_LIMIT: usize = 65_534;

/// The longest query string that the HTTP server lets through: the request-target limit less the
/// 13 bytes of `/api/graphql?`.
const SERVER_QUERY_LIMIT: usize = 65_521;

/// The start of every query string of these tests: the query `{__typename}`, which touches no
/// dependency, then an unknown parameter, which the GET form ignores, whose value pads the query
/// string to its length.
const QUERY_START: &str = "query=%7B__typename%7D&pad=";

/// The request-target of a GET of the `GraphQL` path whose query string has exactly `length`
/// bytes.
fn target(length: usize) -> String {
    let padding = "a".repeat(length.saturating_sub(QUERY_START.len()));
    format!("/api/graphql?{QUERY_START}{padding}")
}

/// The headers of a GET that negotiates the event stream from [`ORIGIN`].
const STREAM_FROM_ORIGIN: [(&str, &str); 2] = [("accept", "text/event-stream"), ("origin", ORIGIN)];

/// The values of every header `name` of `answer`, each header split at its commas.
fn values(answer: &Answer, name: &str) -> Vec<String> {
    let prefix = format!("{name}:");
    answer
        .head
        .lines()
        .filter_map(|line| line.strip_prefix(prefix.as_str()))
        .flat_map(|value| value.split(','))
        .map(|value| value.trim().to_owned())
        .collect()
}

/// Asserts that the service read the query string: the event stream of the query, a result and
/// `complete`, readable by [`ORIGIN`], with the request id of the stack.
fn assert_read(answer: &Answer) {
    assert_eq!(answer.status, 200, "{}", answer.head);
    assert_eq!(values(answer, "content-type"), ["text/event-stream"]);
    assert!(
        answer.body.contains(r#"{"data":{"__typename":"Query"}}"#),
        "{}",
        answer.body
    );
    assert!(answer.body.contains("event: complete"), "{}", answer.body);
    assert_eq!(values(answer, "access-control-allow-origin"), [ORIGIN]);
    assert_eq!(values(answer, "x-request-id").len(), 1, "{}", answer.head);
}

/// The statuses of the `request completed` entries of a node, in order.
fn completed(finished: &Finished) -> Vec<Value> {
    messages_of(&finished.lines, "request completed")
        .into_iter()
        .map(|line| line["http.response.status_code"].clone())
        .collect()
}

#[test]
fn a_served_binary_answers_the_414_problem_above_a_lowered_query_string_bound_with_cors_and_vary() {
    let scratch = Scratch::new();
    let mut variables = NODE.to_vec();
    let bound = LOWERED_BOUND.to_string();
    variables.push(("HTTP_MAX_BODY_BYTES", &bound));
    let server = serve(&scratch, &variables);

    let read = server.request("GET", &target(LOWERED_BOUND), &STREAM_FROM_ORIGIN, "");
    let refused = server.request(
        "GET",
        &target(LOWERED_BOUND.saturating_add(1)),
        &STREAM_FROM_ORIGIN,
        "",
    );
    let finished = server.terminate();

    assert_read(&read);
    assert_eq!(refused.status, 414, "{}", refused.head);
    assert_eq!(
        values(&refused, "content-type"),
        ["application/problem+json"]
    );
    assert_eq!(
        serde_json::from_str::<Value>(&refused.body).unwrap(),
        json!({
            "type": "about:blank",
            "title": "URI Too Long",
            "status": 414,
            "detail": "The query string is too long.",
        })
    );
    assert_eq!(values(&refused, "access-control-allow-origin"), [ORIGIN]);
    let vary = values(&refused, "vary");
    assert_eq!(
        vary.iter().filter(|value| *value == "accept").count(),
        1,
        "{vary:?}"
    );
    assert!(vary.iter().any(|value| value == "origin"), "{vary:?}");
    assert!(
        !refused.head.contains("text/event-stream"),
        "{}",
        refused.head
    );
    assert!(!refused.body.contains("event:"), "{}", refused.body);
    assert_eq!(completed(&finished), [json!(200), json!(414)]);
    assert_eq!(finished.code, Some(0));
}

#[test]
fn a_served_binary_leaves_a_request_target_over_65534_bytes_to_the_bare_414_of_the_http_server() {
    let scratch = Scratch::new();
    let server = serve(&scratch, &NODE);
    let longest = target(SERVER_QUERY_LIMIT);
    let over = target(SERVER_QUERY_LIMIT.saturating_add(1));

    let read = server.request("GET", &longest, &STREAM_FROM_ORIGIN, "");
    let refused = server.request("GET", &over, &STREAM_FROM_ORIGIN, "");
    let finished = server.terminate();

    assert_eq!(longest.len(), SERVER_TARGET_LIMIT);
    assert_eq!(over.len(), SERVER_TARGET_LIMIT.saturating_add(1));
    assert_read(&read);
    assert_eq!(refused.status, 414, "{}", refused.head);
    assert_eq!(refused.body, "", "{}", refused.head);
    for name in [
        "content-type",
        "access-control-allow-origin",
        "vary",
        "x-request-id",
    ] {
        assert_eq!(
            values(&refused, name),
            Vec::<String>::new(),
            "{name}: {}",
            refused.head
        );
    }
    assert_eq!(
        completed(&finished),
        [json!(200)],
        "the service never saw the request"
    );
    assert_eq!(finished.code, Some(0));
}

/// The bound of the document, `query`, in Unicode code points (decision M5).
const DOCUMENT_BOUND: usize = 4_096;

/// The bound of the operation name, `operationName`, in Unicode code points (decision M5).
const OPERATION_NAME_BOUND: usize = 128;

/// The character U+1F600 in its widest form in a query string: four percent-encoded bytes,
/// twelve bytes per character.
const WIDE: &str = "%F0%9F%98%80";

/// The request-target of a GET of the `GraphQL` path whose `query` and `operationName` hold
/// `query` and `name` copies of [`WIDE`].
fn widest(query: usize, name: usize) -> String {
    format!(
        "/api/graphql?query={}&operationName={}&variables=%7B%7D&extensions=%7B%7D",
        WIDE.repeat(query),
        WIDE.repeat(name)
    )
}

/// With the default `HTTP_MAX_BODY_BYTES`, a document and an operation name at their bounds, every
/// character in its widest encoding, fit the query string that the HTTP server lets through
/// (FR-4, decision M6): the served binary reads that GET and answers it with an event stream, and
/// one character more is the 422 length problem, not the 414 problem or the bare 414 of the HTTP
/// server.
#[test]
fn a_served_binary_reads_a_get_at_the_document_bounds_in_the_widest_encoding() {
    let scratch = Scratch::new();
    let server = serve(&scratch, &NODE);
    let at_bounds = widest(DOCUMENT_BOUND, OPERATION_NAME_BOUND);
    let one_more = widest(DOCUMENT_BOUND.saturating_add(1), OPERATION_NAME_BOUND);

    let read = server.request("GET", &at_bounds, &STREAM_FROM_ORIGIN, "");
    let refused = server.request("GET", &one_more, &STREAM_FROM_ORIGIN, "");
    let finished = server.terminate();

    assert!(
        at_bounds.len() <= SERVER_TARGET_LIMIT,
        "{}",
        at_bounds.len()
    );
    assert_eq!(read.status, 200, "{}", read.head);
    assert_eq!(values(&read, "content-type"), ["text/event-stream"]);
    assert!(read.body.contains("event: next"), "{}", read.body);
    assert!(read.body.contains("event: complete"), "{}", read.body);
    assert_eq!(values(&read, "access-control-allow-origin"), [ORIGIN]);
    assert_eq!(refused.status, 422, "{}", refused.head);
    assert_eq!(
        values(&refused, "content-type"),
        ["application/problem+json"]
    );
    let problem = serde_json::from_str::<Value>(&refused.body).unwrap();
    assert_eq!(problem["violations"][0]["propertyPath"], "query");
    assert_eq!(problem["violations"][0]["code"], "length");
    assert_eq!(values(&refused, "access-control-allow-origin"), [ORIGIN]);
    assert_eq!(completed(&finished), [json!(200), json!(422)]);
    assert_eq!(finished.code, Some(0));
}
