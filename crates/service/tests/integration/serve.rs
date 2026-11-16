//! `service serve` as a process: start-up, the request log, shutdown, failures and outages.

use std::io::{Read as _, Write as _};
use std::time::Duration;

use serde_json::Value;
use tokio::time::Instant;

use crate::support::{READINESS, Scratch, Server, VALID_ENVIRONMENT, messages_of, serve, stack};

const REQUEST_ID: &str = "serve-test-1";

/// The configuration of a service that starts but whose dependencies do not answer: SQS and the
/// Kafka broker on port 1, where nothing listens.
const DEAD_DEPENDENCIES: [(&str, &str); 3] = [
    ("AWS_REGION", "us-east-1"),
    ("AWS_ENDPOINT_URL_SQS", "http://127.0.0.1:1"),
    ("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:1"),
];

/// Messages written outside a request.
const OUTSIDE_REQUESTS: [&str; 4] = [
    "listening",
    "event tail ready",
    "shutdown signal received",
    "stopped",
];

/// A service whose dependencies do not answer, the Kafka broker included (port 1, whatever runs on
/// the default `localhost:9092` of the test machine); returns once its tail logged the unreachable
/// topic, so that every later entry comes after that one.
fn dead_server(scratch: &Scratch, extra: &[(&str, &str)]) -> Server {
    let mut variables = DEAD_DEPENDENCIES.to_vec();
    variables.extend_from_slice(extra);
    let mut server = serve(scratch, &variables);
    server.read_until("event topic unreachable");
    server
}

/// Whether the entry belongs to a request that waited for readiness.
fn waits_for_readiness(line: &Value) -> bool {
    line["span"]["request_id"] == READINESS
}

fn as_pairs<'a>(owned: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    owned
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect()
}

#[tokio::test]
async fn a_request_sequence_is_logged_as_json_with_the_request_id() {
    let scratch = Scratch::new();
    let owned = stack();
    let server = serve(&scratch, &as_pairs(&owned));
    server.wait_until_ready().await;
    let headers = [
        ("x-request-id", REQUEST_ID),
        ("content-type", "application/json"),
    ];
    let body = r#"{"query":"{ eventTypes { name } }"}"#;

    let ping = server.request("GET", "/ping", &headers, "");
    let health = server.request("GET", "/api/health", &headers, "");
    let graphql = server.request("POST", "/api/graphql", &headers, body);
    let unknown = server.request("GET", "/no-such-route", &headers, "");
    let finished = server.terminate();

    assert_eq!((ping.status, ping.body.as_str()), (200, "pong"));
    assert_eq!(health.status, 204);
    assert!(
        ping.head.contains("x-request-id: serve-test-1"),
        "{}",
        ping.head
    );
    assert_eq!(graphql.status, 200, "{}", graphql.body);
    assert!(
        graphql.body.contains(r#""eventTypes":[]"#),
        "{}",
        graphql.body
    );
    assert_eq!(unknown.status, 404);
    assert_eq!(finished.code, Some(0));
    assert_eq!(finished.stderr, "");
    let listening = &finished.lines[0];
    assert_eq!(listening["message"], "listening");
    assert_eq!(listening["level"], "INFO");
    assert!(
        listening["addr"]
            .as_str()
            .unwrap()
            .starts_with("127.0.0.1:")
    );
    assert_eq!(listening["env"], "prod");
    assert_eq!(listening["version"], env!("CARGO_PKG_VERSION"));
    for line in &finished.lines {
        for key in ["timestamp", "level", "message"] {
            assert!(line[key].is_string(), "{key} missing in {line}");
        }
    }
    let inside: Vec<&Value> = finished
        .lines
        .iter()
        .filter(|line| !OUTSIDE_REQUESTS.contains(&line["message"].as_str().unwrap()))
        .filter(|line| !waits_for_readiness(line))
        .collect();
    assert!(
        inside
            .iter()
            .all(|line| line["span"]["request_id"] == REQUEST_ID),
        "{inside:?}"
    );
    let completed = messages_of(&finished.lines, "request completed")
        .into_iter()
        .filter(|line| !waits_for_readiness(line))
        .count();
    assert_eq!(completed, 4);
}

#[test]
fn the_start_up_entry_names_the_mode() {
    for mode in ["dev", "test", "prod"] {
        let scratch = Scratch::new();
        let server = dead_server(&scratch, &[("APP_ENV", mode)]);

        let finished = server.terminate();

        assert_eq!(finished.lines[0]["env"], mode);
        assert_eq!(finished.code, Some(0));
    }
}

/// Returns when the service has accepted the connection opened by `begin` and handed it to its
/// task. A service accepts connections in the order they were made, so an answered request on a
/// later connection proves that the earlier one was accepted. It does not prove that the task of
/// the earlier connection has already been polled.
fn wait_until_accepted(server: &Server) {
    assert_eq!(server.get("/ping").status, 200);
}

/// How long an idle service with the broker down fails its rounds before the signal: about ten
/// rounds with `KAFKA_RETRY_DELAY` 100 ms.
const FAILED_ROUNDS: Duration = Duration::from_secs(1);

#[test]
fn an_idle_service_with_the_broker_down_stops_on_sigterm_within_two_seconds() {
    let scratch = Scratch::new();
    let server = dead_server(&scratch, &[("KAFKA_RETRY_DELAY", "100ms")]);
    // A process has no paused time, and that the fault is logged once and nothing comes from
    // krafka is only shown over rounds that fail; the wait is bounded by its fixed length.
    std::thread::sleep(FAILED_ROUNDS);
    let started = Instant::now();

    let finished = server.terminate();

    assert_eq!(finished.code, Some(0));
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(
        finished.messages(),
        [
            "listening",
            "event topic unreachable",
            "shutdown signal received",
            "stopped"
        ]
    );
    assert_eq!(finished.lines[1]["level"], "WARN");
    assert!(
        finished.lines[1]["error"]
            .as_str()
            .is_some_and(|error| error.starts_with("the topic is unavailable")),
        "{}",
        finished.lines[1]
    );
    assert_eq!(finished.lines[2]["signal"], "SIGTERM");
    assert_eq!(finished.lines[2]["level"], "INFO");
    assert_eq!(finished.lines[3]["level"], "INFO");
    assert!(
        finished.lines.iter().all(|line| !line["target"]
            .as_str()
            .unwrap_or_default()
            .starts_with("krafka")),
        "{:?}",
        finished.lines
    );
    assert_eq!(finished.stderr, "");
}

#[test]
fn sigint_stops_the_service_like_sigterm() {
    let scratch = Scratch::new();
    let server = dead_server(&scratch, &[]);

    server.signal("INT");
    let finished = server.finish();

    assert_eq!(finished.code, Some(0));
    assert_eq!(finished.lines[2]["signal"], "SIGINT");
    assert_eq!(finished.lines[3]["message"], "stopped");
    assert_eq!(finished.lines[2]["level"], "INFO");
    assert_eq!(finished.lines[3]["level"], "INFO");
}

#[test]
fn a_request_in_flight_completes_and_new_connections_are_refused() {
    let scratch = Scratch::new();
    let mut server = dead_server(&scratch, &[]);
    let mut stream = server.begin("/ping");
    wait_until_accepted(&server);

    server.signal("TERM");
    server.read_until("shutdown signal received");
    server.wait_until_refusing(Duration::from_secs(10));
    stream.write_all(b"\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    let finished = server.finish();

    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.ends_with("pong"), "{response}");
    assert_eq!(finished.code, Some(0));
    assert_eq!(finished.lines.last().unwrap()["message"], "stopped");
}

#[test]
fn a_connection_open_at_the_end_of_the_grace_period_forces_exit_status_1() {
    let scratch = Scratch::new();
    let mut server = dead_server(&scratch, &[("SHUTDOWN_GRACE_PERIOD", "1s")]);
    let mut stream = server.begin("/ping");
    wait_until_accepted(&server);
    let started = Instant::now();

    server.signal("TERM");
    server.read_until("shutdown signal received");
    let finished = server.finish();
    let elapsed = started.elapsed();
    let mut rest = String::new();
    let read = stream.read_to_string(&mut rest);

    assert_eq!(finished.code, Some(1));
    assert!(elapsed >= Duration::from_secs(1), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(3), "{elapsed:?}");
    let last = finished.lines.last().unwrap();
    assert_eq!(last["message"], "grace period elapsed, forcing shutdown");
    assert_eq!(last["level"], "WARN");
    assert_eq!(last["grace_period"], "1s");
    assert!(!finished.messages().contains(&"stopped"));
    assert!(
        read.is_err() || rest.is_empty(),
        "the connection was answered: {rest}"
    );
    assert_eq!(finished.stderr, "");
}

#[test]
fn a_port_in_use_is_one_serve_failed_entry_and_exit_status_1() {
    let scratch = Scratch::new();
    let first = dead_server(&scratch, &[]);
    let port = first.port.to_string();
    let mut variables = DEAD_DEPENDENCIES.to_vec();
    variables.extend([("APP_PORT", port.as_str()), ("APP_HOST", "127.0.0.1")]);

    let second = scratch.run(&["serve"], &variables);
    let first = first.terminate();

    assert_eq!(second.status.code(), Some(1));
    assert_eq!(crate::support::stderr(&second), "");
    let stdout = crate::support::stdout(&second);
    let lines: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "{stdout}");
    assert_eq!(lines[0]["message"], "serve failed");
    assert_eq!(lines[0]["level"], "ERROR");
    assert!(
        lines[0]["error"]
            .as_str()
            .is_some_and(|error| !error.is_empty())
    );
    assert_eq!(first.code, Some(0));
}

#[test]
fn an_invalid_variable_is_one_json_entry_per_problem_and_exit_status_2() {
    let scratch = Scratch::new();
    let variables = [
        ("APP_PORT", "eighty"),
        ("AWS_ENDPOINT_URL_SQS", "ftp://root:hunter2@sqs/queue"),
    ];

    let output = scratch.run(&["serve"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(crate::support::stderr(&output), "");
    let stdout = crate::support::stdout(&output);
    let lines: Vec<Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let named: Vec<&str> = lines
        .iter()
        .map(|line| line["variable"].as_str().unwrap())
        .collect();
    assert_eq!(named, ["APP_PORT", "AWS_REGION", "AWS_ENDPOINT_URL_SQS"]);
    for line in &lines {
        assert_eq!(line["message"], "invalid configuration");
        assert_eq!(line["level"], "ERROR");
        assert!(
            line["reason"]
                .as_str()
                .is_some_and(|reason| !reason.is_empty())
        );
    }
    assert!(!stdout.contains("hunter2"));
}

#[test]
fn unreachable_dependencies_do_not_stop_ping_and_health_names_them() {
    let scratch = Scratch::new();
    let started = Instant::now();
    let server = dead_server(&scratch, &[]);

    let ping = server.get("/ping");
    let ping_after = started.elapsed();
    let probed = Instant::now();
    let health = server.get("/api/health");
    let probe_took = probed.elapsed();
    let finished = server.terminate();

    assert_eq!(ping.status, 200);
    assert!(ping_after < Duration::from_secs(5), "{ping_after:?}");
    assert_eq!(health.status, 500);
    assert!(probe_took < Duration::from_secs(4), "{probe_took:?}");
    assert!(health.body.contains("Message broker"), "{}", health.body);
    assert!(
        health.body.contains("Event stream is not ready"),
        "{}",
        health.body
    );
    assert!(!health.body.contains("Cache"), "{}", health.body);
    assert_eq!(finished.code, Some(0));
}

#[test]
fn the_credentials_are_never_logged() {
    let scratch = Scratch::new();
    let mut variables = VALID_ENVIRONMENT.to_vec();
    variables.extend([
        ("AWS_ENDPOINT_URL_SQS", "http://127.0.0.1:1"),
        ("AWS_ACCESS_KEY_ID", "reader"),
        ("AWS_SECRET_ACCESS_KEY", "s3cret-pass"),
        ("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:1"),
        ("KAFKA_SECURITY_PROTOCOL", "sasl_plaintext"),
        ("KAFKA_SASL_MECHANISM", "PLAIN"),
        ("KAFKA_SASL_USERNAME", "reader"),
        ("KAFKA_SASL_PASSWORD", "s3cret-kafka"),
    ]);
    let mut server = serve(&scratch, &variables);
    server.read_until("event topic unreachable");
    let _answer = server.get("/api/health");

    let finished = server.terminate();

    let text = serde_json::to_string(&finished.lines).unwrap();
    assert!(!text.contains("s3cret-pass"), "{text}");
    assert!(!text.contains("s3cret-kafka"), "{text}");
}

#[tokio::test]
async fn entries_inside_nested_spans_carry_the_request_id() {
    let scratch = Scratch::new();
    let mut owned = stack();
    owned.push(("LOG_LEVEL", "debug".to_owned()));
    let server = serve(&scratch, &as_pairs(&owned));
    server.wait_until_ready().await;

    let health = server.request("GET", "/api/health", &[("x-request-id", REQUEST_ID)], "");
    let finished = server.terminate();

    assert_eq!(health.status, 204, "{}", health.body);
    assert_eq!(finished.code, Some(0));
    // The one request is the only work between the start-up marker and the signal; entries of
    // start-up (the AWS configuration is read in spans of its own) come before the marker.
    let listening = finished
        .lines
        .iter()
        .position(|line| line["message"] == "listening");
    let signal = finished
        .lines
        .iter()
        .position(|line| line["message"] == "shutdown signal received");
    let inside: Vec<&Value> = finished.lines[listening.unwrap().saturating_add(1)..signal.unwrap()]
        .iter()
        .filter(|line| {
            !line["message"]
                .as_str()
                .is_some_and(|message| OUTSIDE_REQUESTS.contains(&message))
        })
        .filter(|line| !waits_for_readiness(line))
        .collect();
    assert!(
        inside
            .iter()
            .any(|line| line["span"]["name"] != "http.request"),
        "no entry was written inside a nested span: {inside:?}"
    );
    for line in &inside {
        assert_eq!(line["spans"][0]["name"], "http.request", "{line}");
        assert_eq!(line["spans"][0]["request_id"], REQUEST_ID, "{line}");
    }
}
