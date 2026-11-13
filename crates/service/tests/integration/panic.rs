//! The production telemetry on a captured writer with an application whose probe panics.
//!
//! The test installs the global subscriber and the panic hook, so it relies on nextest giving
//! every test its own process.

use std::io;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use clap::Parser as _;
use health_check_domain::Dependency;
use http_body_util::BodyExt as _;
use serde_json::Value;
use service::bootstrap::{Adapters, event_bus, router};
use service::cli::{Cli, Command};
use service::config::Config;
use service::telemetry;
use test_support::clock::FixedClock;
use test_support::id::SequenceUuidGenerator;
use test_support::probe::StubProbe;
use test_support::repository::InMemoryExampleApiResourceRepository;
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

fn config() -> Config {
    let Command::Serve(mut args) = Cli::parse_from(["service", "serve"]).command else {
        panic!("`serve` arguments expected");
    };
    args.aws_region = Some("us-east-1".to_owned());
    args.database.url = Some("postgres://app:unused@127.0.0.1:1/app".to_owned());
    Config::from_args(args).unwrap()
}

#[tokio::test]
async fn a_panic_is_one_json_line_with_the_request_id_and_a_500_problem() {
    let output = Captured::default();
    let sink = output.clone();
    telemetry::init("info", move || sink.clone()).unwrap();
    let adapters = Adapters {
        repository: Arc::new(InMemoryExampleApiResourceRepository::new()),
        probes: vec![Arc::new(StubProbe::panicking(Dependency::Database))],
        event_bus: event_bus(),
        clock: Arc::new(FixedClock::at(SystemTime::UNIX_EPOCH)),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(1)),
    };
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
