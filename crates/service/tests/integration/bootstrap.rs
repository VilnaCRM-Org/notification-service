//! The production wiring in process: `Adapters::connect` and `bootstrap::router` against the
//! compose services, dispatched without a socket.

use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use clap::Parser as _;
use health_check_domain::Dependency;
use http_body_util::BodyExt as _;
use serde_json::Value;
use service::bootstrap::{Adapters, router};
use service::cli::{Cli, Command};
use service::config::Config;
use tower::ServiceExt as _;

use crate::support::{database_url, silent_listener};

/// The configuration as the binary reads it from the environment, with long budgets for what must
/// succeed: the tests prove the wiring, not how fast a loaded machine opens the first connection.
/// The order the configuration requires holds (request > probe >= acquire and operation).
fn config() -> Config {
    let Command::Serve(args) = Cli::parse_from(["service", "serve"]).command else {
        panic!("`serve` arguments expected");
    };
    let mut config = Config::from_args(args).unwrap();
    config.database.acquire_timeout = Duration::from_secs(30);
    config.sqs.connect_timeout = Duration::from_secs(10);
    config.sqs.operation_timeout = Duration::from_secs(30);
    config.health_probe_timeout = Duration::from_secs(40);
    config.request_timeout = Duration::from_secs(60);
    config
}

/// Builds a new application instance from the production wiring and sends one request to it.
async fn send(config: &Config, request: Request<Body>) -> (StatusCode, Value) {
    let adapters = Adapters::connect(config).await.unwrap();
    let response = router(adapters, config)
        .unwrap()
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]
async fn health_is_204_through_the_real_probes(pool: sqlx::PgPool) {
    let mut config = config();
    config.database.url = database_url(&pool).await;

    let health = Request::get("/api/health").body(Body::empty()).unwrap();

    assert_eq!(send(&config, health).await.0, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]
async fn a_resource_created_through_one_instance_is_read_through_another(pool: sqlx::PgPool) {
    let mut config = config();
    config.database.url = database_url(&pool).await;
    let create = Request::post("/api/example_api_resources")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"name":"Stateless","description":"Read by a second instance"}"#,
        ))
        .unwrap();

    let (status, created) = send(&config, create).await;

    assert_eq!(status, StatusCode::CREATED);
    let path = format!(
        "/api/example_api_resources/{}",
        created["id"].as_str().unwrap()
    );
    let (status, read) = send(&config, Request::get(path).body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read, created);
}

#[tokio::test]
async fn connect_makes_no_network_access_and_orders_the_probes() {
    let listener = silent_listener();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut config = config();
    config.database.url = format!("postgres://app:unused@127.0.0.1:{port}/app");
    config.sqs.endpoint_url = Some(format!("http://127.0.0.1:{port}"));

    let adapters = Adapters::connect(&config).await.unwrap();

    let dependencies: Vec<_> = adapters
        .probes
        .iter()
        .map(|probe| probe.dependency())
        .collect();
    assert_eq!(
        dependencies,
        [
            Dependency::Database,
            Dependency::Cache,
            Dependency::MessageBroker
        ]
    );
    let accepted = listener.accept();
    assert_eq!(accepted.unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}
