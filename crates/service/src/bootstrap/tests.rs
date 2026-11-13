use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use health_check_domain::{Dependency, HealthProbe};
use health_check_infrastructure::CacheProbe;
use http_body_util::BodyExt as _;
use serde_json::Value;
use shared_domain::EventBus;
use shared_infrastructure::bus::{InMemoryCommandBus, InMemoryQueryBus};
use some_module_application::{CreateExampleApiResourceHandler, GetExampleApiResourceHandler};
use some_module_domain::ExampleApiResourceRepository;
use test_support::bus::RecordingEventBus;
use test_support::cache::StubCache;
use test_support::clock::FixedClock;
use test_support::id::SequenceUuidGenerator;
use test_support::log::LogRecorder;
use test_support::probe::StubProbe;
use test_support::repository::InMemoryExampleApiResourceRepository;
use tower::ServiceExt as _;

use super::{Adapters, BootstrapError, event_bus, register_commands, register_queries, router};
use crate::cli::ServeArgs;
use crate::config::Config;

const FIRST_ID: &str = "00000000-0000-7000-8000-000000000001";

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Answer {
    fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap()
    }
}

fn config(app_env: &str) -> Config {
    let mut args = ServeArgs {
        app_env: Some(app_env.to_owned()),
        aws_region: Some("us-east-1".to_owned()),
        ..ServeArgs::default()
    };
    args.database.url = Some("postgres://app:unused@127.0.0.1:1/app".to_owned());
    Config::from_args(args).unwrap()
}

fn healthy_probes() -> Vec<Arc<dyn HealthProbe>> {
    vec![
        Arc::new(StubProbe::healthy(Dependency::Database)),
        Arc::new(StubProbe::healthy(Dependency::Cache)),
        Arc::new(StubProbe::healthy(Dependency::MessageBroker)),
    ]
}

fn adapters() -> Adapters {
    Adapters {
        repository: Arc::new(InMemoryExampleApiResourceRepository::new()),
        probes: healthy_probes(),
        event_bus: event_bus(),
        clock: Arc::new(FixedClock::at(SystemTime::UNIX_EPOCH)),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(1)),
    }
}

fn app(adapters: Adapters) -> Router {
    router(adapters, &config("test")).unwrap()
}

async fn send(app: &Router, request: Request<Body>) -> Answer {
    let response = app.clone().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    Answer {
        status: parts.status,
        headers: parts.headers,
        body: String::from_utf8(bytes.to_vec()).unwrap(),
    }
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

fn post(path: &str, body: &str) -> Request<Body> {
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn create() -> Request<Body> {
    post(
        "/api/example_api_resources",
        r#"{"name":"Name","description":"Description"}"#,
    )
}

fn graphql(query: &str) -> Request<Body> {
    post(
        "/api/graphql",
        &serde_json::json!({ "query": query }).to_string(),
    )
}

#[tokio::test]
async fn every_handler_subscriber_and_probe_is_registered() {
    let app = app(adapters());

    let created = send(&app, create()).await;
    assert_eq!(created.status, StatusCode::CREATED);
    let id = created.json()["id"].as_str().unwrap().to_owned();
    let one = send(&app, get(&format!("/api/example_api_resources/{id}"))).await;
    assert_eq!(one.status, StatusCode::OK);
    assert_eq!(one.json()["name"], "Name");
    let list = send(&app, get("/api/example_api_resources")).await;
    assert_eq!(list.status, StatusCode::OK);
    assert_eq!(list.json().as_array().unwrap().len(), 1);

    let mutation = graphql(
        r#"mutation { createExampleApiResource(input: {name: "G", description: "D"}) { id name } }"#,
    );
    let query_one = graphql(&format!(
        r#"{{ exampleApiResource(id: "{id}") {{ id name }} }}"#
    ));
    let query_list = graphql("{ exampleApiResources { id name } }");
    for request in [mutation, query_one, query_list] {
        let answer = send(&app, request).await;
        assert_eq!(answer.status, StatusCode::OK);
        assert!(answer.json().get("errors").is_none(), "{}", answer.body);
    }

    let health = send(&app, get("/api/health")).await;
    assert_eq!(health.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_created_resource_is_audited_by_the_registered_subscriber() {
    let (recorder, _guard) = LogRecorder::install();
    let app = app(adapters());

    let created = send(&app, create()).await;

    assert_eq!(created.status, StatusCode::CREATED);
    let audit: Vec<_> = recorder
        .lines()
        .into_iter()
        .filter(|line| line.target == "audit")
        .collect();
    assert_eq!(audit.len(), 1);
    assert_eq!(audit[0].field("aggregate_id"), Some(FIRST_ID));
}

#[tokio::test]
async fn a_replaced_repository_serves_the_requests() {
    let mut adapters = adapters();
    adapters.repository = Arc::new(InMemoryExampleApiResourceRepository::failing());

    let answer = send(&app(adapters), create()).await;

    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn a_replaced_probe_decides_the_health_answer() {
    let adapters = adapters().with_probe(Arc::new(StubProbe::failing(Dependency::Database)));

    let answer = send(&app(adapters), get("/api/health")).await;

    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        answer.body.contains("Database is not available"),
        "{}",
        answer.body
    );
    assert!(
        !answer.body.contains("Cache is not working"),
        "{}",
        answer.body
    );
}

#[tokio::test]
async fn a_failing_cache_behind_the_cache_probe_is_reported() {
    let cache_probe = CacheProbe::new(Arc::new(StubCache::failing()));
    let adapters = adapters().with_probe(Arc::new(cache_probe));

    let answer = send(&app(adapters), get("/api/health")).await;

    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(
        answer.body.contains("Cache is not working"),
        "{}",
        answer.body
    );
    assert!(
        !answer.body.contains("Database is not available"),
        "{}",
        answer.body
    );
}

#[tokio::test]
async fn a_replaced_event_bus_receives_the_event() {
    let recording = Arc::new(RecordingEventBus::new());
    let mut adapters = adapters();
    adapters.event_bus = {
        let port: Arc<dyn EventBus> = Arc::<RecordingEventBus>::clone(&recording);
        port
    };

    let answer = send(&app(adapters), create()).await;

    assert_eq!(answer.status, StatusCode::CREATED);
    let published = recording.published();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].aggregate_id.to_string(), FIRST_ID);
}

#[tokio::test]
async fn a_failing_event_bus_does_not_fail_the_create_request() {
    let mut adapters = adapters();
    adapters.event_bus = Arc::new(RecordingEventBus::failing());

    let answer = send(&app(adapters), create()).await;

    assert_eq!(answer.status, StatusCode::CREATED);
    assert_eq!(answer.json()["id"], FIRST_ID);
}

#[tokio::test]
async fn a_replaced_clock_stamps_the_event() {
    let (recorder, _guard) = LogRecorder::install();
    let moment = SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(1_700_000_000))
        .unwrap();
    let mut adapters = adapters();
    adapters.clock = Arc::new(FixedClock::at(moment));

    let answer = send(&app(adapters), create()).await;

    assert_eq!(answer.status, StatusCode::CREATED);
    let lines = recorder.lines();
    let audit = lines.iter().find(|line| line.target == "audit").unwrap();
    assert_eq!(
        audit.field("occurred_at"),
        Some("2023-11-14T22:13:20.000000Z")
    );
}

#[tokio::test]
async fn a_replaced_uuid_generator_chooses_the_identifier() {
    let mut adapters = adapters();
    adapters.uuid_generator = Arc::new(SequenceUuidGenerator::starting_at(42));

    let answer = send(&app(adapters), create()).await;

    assert_eq!(answer.status, StatusCode::CREATED);
    assert_eq!(answer.json()["id"], "00000000-0000-7000-8000-00000000002a");
    assert_eq!(
        answer.headers[header::LOCATION],
        "/api/example_api_resources/00000000-0000-7000-8000-00000000002a"
    );
}

#[tokio::test]
async fn with_probe_keeps_the_order_and_the_other_probes() {
    let adapters = adapters().with_probe(Arc::new(StubProbe::failing(Dependency::Cache)));

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
    let outcomes: Vec<_> = {
        let mut outcomes = Vec::new();
        for probe in &adapters.probes {
            outcomes.push(probe.check().await.is_ok());
        }
        outcomes
    };
    assert_eq!(outcomes, [true, false, true]);
}

fn create_handler(
    repository: &Arc<dyn ExampleApiResourceRepository>,
) -> CreateExampleApiResourceHandler {
    CreateExampleApiResourceHandler::new(
        Arc::clone(repository),
        event_bus(),
        Arc::new(FixedClock::at(SystemTime::UNIX_EPOCH)),
        Arc::new(SequenceUuidGenerator::starting_at(1)),
    )
}

fn repository() -> Arc<dyn ExampleApiResourceRepository> {
    Arc::new(InMemoryExampleApiResourceRepository::new())
}

#[test]
fn a_second_command_handler_is_a_duplicate_that_names_the_message() {
    let repository = repository();
    let bus = register_commands(InMemoryCommandBus::new(), create_handler(&repository)).unwrap();

    let error = register_commands(bus, create_handler(&repository)).unwrap_err();

    assert!(
        matches!(error, BootstrapError::DuplicateHandler(_)),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("CreateExampleApiResource"),
        "{error}"
    );
}

#[test]
fn a_second_query_handler_is_a_duplicate_that_names_the_message() {
    let repository = repository();
    let bus = InMemoryQueryBus::new()
        .register(GetExampleApiResourceHandler::new(Arc::clone(&repository)))
        .unwrap();

    let error = register_queries(bus, &repository).unwrap_err();

    assert!(
        matches!(error, BootstrapError::DuplicateHandler(_)),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("GetExampleApiResource"),
        "{error}"
    );
}

#[test]
fn the_database_error_keeps_its_message_and_source() {
    let source = sqlx::Error::Configuration("bad url".into());

    let error = BootstrapError::from(source);

    assert_eq!(error.to_string(), "invalid database settings");
    assert!(std::error::Error::source(&error).is_some());
}
