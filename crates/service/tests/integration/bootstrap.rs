//! The production wiring in process: `Adapters::connect`, `bootstrap::router` and
//! `bootstrap::application` against the compose services, dispatched without a socket.

use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use clap::Parser as _;
use health_check_domain::Dependency;
use http_body_util::BodyExt as _;
use serde_json::Value;
use service::bootstrap::{Adapters, application, router};
use service::cli::{Cli, Command};
use service::config::Config;
use tower::ServiceExt as _;

use crate::support::silent_listener;

/// The configuration as the binary reads it from the environment, with long budgets for what must
/// succeed: the tests prove the wiring, not how fast a loaded machine opens the first connection.
/// The order the configuration requires holds (request > probe >= operation).
fn config() -> Config {
    let Command::Serve(args) = Cli::parse_from(["service", "serve"]).command else {
        panic!("`serve` arguments expected");
    };
    let mut config = Config::from_args(args).unwrap();
    config.sqs.connect_timeout = Duration::from_secs(10);
    config.sqs.operation_timeout = Duration::from_secs(30);
    config.health_probe_timeout = Duration::from_secs(40);
    config.request_timeout = Duration::from_secs(60);
    config
}

/// How long a test waits for the node to become ready.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// Sends one request to `router` and reads the whole answer.
async fn dispatch(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

/// Builds a new application instance from the production wiring and sends one request to it.
async fn send(config: &Config, request: Request<Body>) -> (StatusCode, Value) {
    let adapters = Adapters::connect(config).await;
    dispatch(&router(adapters, config).unwrap(), request).await
}

/// Polls `GET /api/events/status` until the node reports `ready`, for at most 10 s.
async fn wait_until_ready(router: &Router) {
    tokio::time::timeout(READY_TIMEOUT, async {
        loop {
            let status = Request::get("/api/events/status")
                .body(Body::empty())
                .unwrap();
            if dispatch(router, status).await.1["state"] == "ready" {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the node becomes ready");
}

/// Builds a new application instance from the production wiring, runs its tail on the broker of
/// the stack until the node is ready, sends one request and shuts the background work down.
async fn send_when_ready(config: &Config, request: Request<Body>) -> (StatusCode, Value) {
    let adapters = Adapters::connect(config).await;
    let application = application(adapters, config).unwrap();
    let running = application.background.spawn();
    wait_until_ready(&application.router).await;
    let answer = dispatch(&application.router, request).await;
    tokio::time::timeout(READY_TIMEOUT, running.shutdown())
        .await
        .expect("the shutdown ends");
    answer
}

#[tokio::test]
async fn health_is_204_through_the_real_probes() {
    let config = config();

    let health = Request::get("/api/health").body(Body::empty()).unwrap();

    assert_eq!(
        send_when_ready(&config, health).await.0,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn health_names_the_event_stream_of_a_router_without_its_tail() {
    let config = config();

    let health = Request::get("/api/health").body(Body::empty()).unwrap();
    let (status, problem) = send(&config, health).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        problem["failures"],
        serde_json::json!([{"dependency": "event_stream", "message": "Event stream is not ready"}])
    );
}

#[tokio::test]
async fn connect_makes_no_network_access_and_orders_the_probes() {
    let listener = silent_listener();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut config = config();
    config.sqs.endpoint_url = Some(format!("http://127.0.0.1:{port}"));
    config.kafka.bootstrap_servers = format!("127.0.0.1:{port}");

    let adapters = Adapters::connect(&config).await;

    let servers = format!("bootstrap_servers: \"127.0.0.1:{port}\"");
    let readers = [
        format!("{:?}", adapters.topic_probe),
        format!("{:?}", adapters.topic_tail),
        format!("{:?}", adapters.topic_replay),
    ];
    for (reader, name) in readers
        .iter()
        .zip(["KafkaProbe", "KafkaTail", "KafkaReplay"])
    {
        assert!(reader.starts_with(name), "{reader}");
        assert!(reader.contains(&servers), "{reader}");
    }

    let dependencies: Vec<_> = adapters
        .probes
        .iter()
        .map(|probe| probe.dependency())
        .collect();
    assert_eq!(dependencies, [Dependency::Cache, Dependency::MessageBroker]);
    let accepted = listener.accept();
    assert_eq!(accepted.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}
