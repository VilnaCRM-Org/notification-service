//! The production telemetry on a captured writer with an application whose probe panics, or writes
//! an entry inside a span of its own.
//!
//! Each test installs the global subscriber and the panic hook, so it relies on nextest giving
//! every test its own process.

use std::io;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use clap::Parser as _;
use health_check_domain::{Dependency, HealthProbe};
use http_body_util::BodyExt as _;
use serde_json::Value;
use service::bootstrap::{Adapters, event_bus, router};
use service::cli::{Cli, Command};
use service::config::Config;
use service::telemetry;
use shared_domain::BoxError;
use test_support::clock::FixedClock;
use test_support::id::SequenceUuidGenerator;
use test_support::probe::StubProbe;
use test_support::static_topic::StaticTopic;
use tower::ServiceExt as _;

const REQUEST_ID: &str = "panic-test-1";

/// An in-memory log sink that the test reads back.
#[derive(Debug, Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_poisoned| io::ErrorKind::Other)?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The adapters of an application without a topic whose only probe is `probe`.
fn adapters(probe: Arc<dyn HealthProbe>) -> Adapters {
    Adapters {
        probes: vec![probe],
        event_bus: event_bus(),
        clock: Arc::new(FixedClock::at(SystemTime::UNIX_EPOCH)),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(1)),
        topic_probe: Arc::new(StaticTopic::empty()),
        topic_tail: Box::new(StaticTopic::empty()),
        topic_replay: Arc::new(StaticTopic::empty()),
    }
}

/// The captured entries, one JSON value per line.
fn entries(output: &Captured) -> Vec<Value> {
    let captured = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    captured
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn config() -> Config {
    let Command::Serve(mut args) = Cli::parse_from(["service", "serve"]).command else {
        panic!("`serve` arguments expected");
    };
    args.kafka
        .bootstrap_servers
        .get_or_insert_with(|| "localhost:9092".to_owned());
    Config::from_args(args).unwrap()
}

#[tokio::test]
async fn a_panic_is_one_json_line_with_the_request_id_and_a_500_problem() {
    let output = Captured::default();
    let sink = output.clone();
    telemetry::init("info", move || sink.clone()).unwrap();
    let adapters = adapters(Arc::new(StubProbe::panicking(Dependency::Cache)));
    let request = Request::get("/api/health")
        .header("x-request-id", REQUEST_ID)
        .body(Body::empty())
        .unwrap();

    let response = router(adapters, &config())
        .unwrap()
        .oneshot(request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response.headers()["content-type"],
        "application/problem+json"
    );
    assert_eq!(response.headers()["x-request-id"], REQUEST_ID);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let problem: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(problem["status"], 500);
    let captured = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    let lines: Vec<Value> = captured
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let messages: Vec<&str> = lines
        .iter()
        .map(|line| line["message"].as_str().unwrap())
        .collect();
    assert_eq!(
        messages,
        ["panic", "request failed", "request completed"],
        "{captured}"
    );
    assert!(
        lines
            .iter()
            .all(|line| line["span"]["request_id"] == REQUEST_ID),
        "{captured}"
    );
    assert_eq!(lines[0]["level"], "ERROR");
    assert!(
        lines[0]["panic"]
            .as_str()
            .unwrap()
            .contains("stub probe panic"),
        "{captured}"
    );
}

const NESTED_REQUEST_ID: &str = "nested-span-test-1";

/// A healthy probe of the cache that writes one entry inside a span of its own, as a client
/// library of a dependency does.
#[derive(Debug)]
struct SpanningProbe;

#[async_trait::async_trait]
impl HealthProbe for SpanningProbe {
    fn dependency(&self) -> Dependency {
        Dependency::Cache
    }

    async fn check(&self) -> Result<(), BoxError> {
        tracing::info_span!("probe.call", probe = "spanning").in_scope(|| {
            tracing::info!("probe called");
        });
        Ok(())
    }
}

#[tokio::test]
async fn an_entry_of_a_nested_span_inside_a_request_carries_the_request_span() {
    let output = Captured::default();
    let sink = output.clone();
    telemetry::init("info", move || sink.clone()).unwrap();
    let request = Request::get("/api/health")
        .header("x-request-id", NESTED_REQUEST_ID)
        .body(Body::empty())
        .unwrap();

    let response = router(adapters(Arc::new(SpanningProbe)), &config())
        .unwrap()
        .oneshot(request)
        .await
        .unwrap();

    assert_eq!(response.headers()["x-request-id"], NESTED_REQUEST_ID);
    let lines = entries(&output);
    let called: Vec<&Value> = lines
        .iter()
        .filter(|line| line["message"] == "probe called")
        .collect();
    assert_eq!(called.len(), 1, "{lines:?}");
    let entry = called[0];
    assert_eq!(entry["span"]["name"], "probe.call", "{entry}");
    assert_eq!(entry["span"]["probe"], "spanning", "{entry}");
    assert_eq!(entry["spans"][0]["name"], "http.request", "{entry}");
    assert_eq!(
        entry["spans"][0]["request_id"], NESTED_REQUEST_ID,
        "{entry}"
    );
    assert_eq!(entry["spans"][1]["name"], "probe.call", "{entry}");
}
