pub(crate) mod wiring;

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, Response, StatusCode, header};
use event_stream_application::{EventCatalog, ListEventTypesHandler};
use event_stream_domain::{Batch, Cursor, Offset, TopicError, TopicRecord, Watermarks};
use health_check_domain::Dependency;
use health_check_infrastructure::CacheProbe;
use http_body_util::BodyExt as _;
use serde_json::Value;
use shared_domain::EventBus;
use shared_infrastructure::bus::{InMemoryCommandBus, InMemoryQueryBus};
use some_module_application::{CreateExampleApiResourceHandler, GetExampleApiResourceHandler};
use some_module_domain::ExampleApiResourceRepository;
use test_support::bus::RecordingEventBus;
use test_support::cache::StubCache;
use test_support::clock::{FixedClock, SettableClock};
use test_support::id::SequenceUuidGenerator;
use test_support::log::{LogLine, LogRecorder};
use test_support::probe::StubProbe;
use test_support::repository::InMemoryExampleApiResourceRepository;
use test_support::static_topic::StaticTopic;
use test_support::topic::{
    ProbeCall, ScriptedTopicProbe, ScriptedTopicReplay, ScriptedTopicTail, TailCall,
};
use tokio::task::JoinHandle;
use tower::ServiceExt as _;

use self::wiring::{adapters_on, paced_on, record};
use super::{
    Adapters, BootstrapError, application, event_bus, register_commands, register_queries, router,
};
use crate::background::Running;
use crate::cli::ServeArgs;
use crate::config::Config;

const FIRST_ID: &str = "00000000-0000-7000-8000-000000000001";

/// The bound of every awaited call of a test that is not a single request.
const LIMIT: Duration = Duration::from_secs(10);

/// The event type the subscriptions of the tests ask for.
const TYPE: &str = "user.created";

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

/// The adapters of a node on an empty `StaticTopic`; its tail reads the bare topic, so these
/// adapters serve a router whose loop never runs.
fn adapters() -> Adapters {
    let topic = StaticTopic::empty();
    adapters_on(&topic, Box::new(topic.clone()))
}

/// The adapters of a node on an empty `StaticTopic` whose tail awaits the fetch wait of the test
/// configuration after each empty batch, so its running loop does not spin.
fn paced() -> Adapters {
    let topic = StaticTopic::empty();
    paced_on(&topic, config("test").kafka.fetch_wait).0
}

fn app(adapters: Adapters) -> Router {
    router(adapters, &config("test")).unwrap()
}

async fn send(app: &Router, request: Request<Body>) -> Answer {
    tokio::time::timeout(Duration::from_secs(5), async {
        let response = app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Answer {
            status: parts.status,
            headers: parts.headers,
            body: String::from_utf8(bytes.to_vec()).unwrap(),
        }
    })
    .await
    .expect("the request answers in time")
}

/// The router of the application of `adapters` with its background work running, once the status
/// route reports `ready` (bounded, 10 s). The polls are spaced by a timer, so paused time advances
/// between them and the bound ends a node that never becomes ready.
async fn ready(adapters: Adapters) -> (Router, Running) {
    ready_with(adapters, &config("test")).await
}

/// [`ready`] with `config`.
async fn ready_with(adapters: Adapters, config: &Config) -> (Router, Running) {
    let application = application(adapters, config).unwrap();
    let running = application.background.spawn();
    let router = application.router;
    tokio::time::timeout(LIMIT, async {
        while !send(&router, get("/api/events/status"))
            .await
            .body
            .contains(r#""state":"ready""#)
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the node becomes ready");
    (router, running)
}

/// Ends the background work within [`LIMIT`].
async fn shut_down(running: Running) {
    tokio::time::timeout(LIMIT, running.shutdown())
        .await
        .expect("the shutdown ends");
}

/// The answer to `request` with its body unread: the body of a subscription does not end.
async fn open(app: &Router, request: Request<Body>) -> Response<Body> {
    tokio::time::timeout(LIMIT, app.clone().oneshot(request))
        .await
        .expect("the request answers in time")
        .unwrap()
}

/// The status and the headers of the answer to `request`; its body is dropped unread.
async fn head(app: &Router, request: Request<Body>) -> (StatusCode, HeaderMap) {
    let response = open(app, request).await;
    (response.status(), response.headers().clone())
}

/// The subscription request of [`TYPE`] followed by `rest` in its query.
fn stream(rest: &str) -> Request<Body> {
    get(&format!("/api/events/stream?types={TYPE}{rest}"))
}

/// The SSE body of `response`, read by a task of its own into the returned text until the task
/// is aborted or the body ends.
fn read_events(response: Response<Body>) -> (Arc<Mutex<String>>, JoinHandle<()>) {
    let read = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&read);
    let mut body = response.into_body();
    let reader = tokio::spawn(async move {
        while let Some(Ok(frame)) = body.frame().await {
            if let Ok(data) = frame.into_data() {
                sink.lock()
                    .unwrap()
                    .push_str(std::str::from_utf8(&data).unwrap());
            }
        }
    });
    (read, reader)
}

/// Lets every task run until all of them wait; paused time advances to each timer on the way.
async fn settle() {
    tokio::time::sleep(Duration::from_secs(60)).await;
}

/// Waits until `condition` holds, within [`LIMIT`]; paused time advances while every task waits.
async fn wait_until(condition: impl Fn() -> bool + Sync) {
    tokio::time::timeout(LIMIT, async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the condition holds in time");
}

/// The time the settable clocks of the tests start at.
fn start_time() -> SystemTime {
    SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(1_700_000_000))
        .unwrap()
}

const fn marks(earliest: u64, end: u64) -> Watermarks {
    Watermarks {
        earliest: Cursor::new(earliest),
        end: Cursor::new(end),
    }
}

fn unavailable() -> TopicError {
    TopicError::Unavailable("the broker refused the connection".into())
}

/// A batch of the events with ids 1 to 3 of [`TYPE`] at offsets 0 to 2.
fn three_events() -> Batch {
    Batch {
        records: (0..3)
            .zip(1..=3)
            .map(|(offset, id)| TopicRecord {
                offset: Offset::new(offset),
                value: Some(record(id, TYPE, 0)),
            })
            .collect(),
        next: Cursor::new(3),
    }
}

/// The adapters of a node on the scripted topic readers, with the settable `clock`.
fn scripted(
    probe: &ScriptedTopicProbe,
    tail: &ScriptedTopicTail,
    clock: &SettableClock,
) -> Adapters {
    let mut adapters = adapters();
    adapters.topic_probe = Arc::new(probe.clone());
    adapters.topic_tail = Box::new(tail.clone());
    adapters.clock = Arc::new(clock.clone());
    adapters
}

/// The `health probe failed` entries of the recorder.
fn probe_failures(recorder: &LogRecorder) -> Vec<LogLine> {
    recorder
        .lines()
        .into_iter()
        .filter(|line| line.message == "health probe failed")
        .collect()
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

#[tokio::test(start_paused = true)]
async fn every_handler_subscriber_and_probe_is_registered() {
    let (app, running) = ready(paced()).await;

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
    tokio::time::timeout(Duration::from_secs(5), running.shutdown())
        .await
        .expect("the shutdown ends");
}

#[tokio::test]
async fn the_probe_of_the_event_stream_follows_the_probes_of_the_adapters() {
    let (recorder, _guard) = LogRecorder::install();
    let mut adapters = adapters();
    adapters.probes = vec![
        Arc::new(StubProbe::failing(Dependency::Database)),
        Arc::new(StubProbe::failing(Dependency::Cache)),
        Arc::new(StubProbe::failing(Dependency::MessageBroker)),
    ];

    let answer = send(&app(adapters), get("/api/health")).await;

    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        answer.json()["failures"],
        serde_json::json!([
            {"dependency": "database", "message": "Database is not available"},
            {"dependency": "cache", "message": "Cache is not working"},
            {"dependency": "message_broker", "message": "Message broker is not available"},
            {"dependency": "event_stream", "message": "Event stream is not ready"}
        ])
    );
    let failed = probe_failures(&recorder);
    let dependencies: Vec<_> = failed.iter().map(|line| line.field("dependency")).collect();
    assert_eq!(
        dependencies,
        [
            Some("database"),
            Some("cache"),
            Some("message_broker"),
            Some("event_stream")
        ]
    );
    assert_eq!(
        failed[3].field("cause"),
        Some("probe failed: the node is starting")
    );
}

#[tokio::test]
async fn the_configured_retry_delay_reaches_the_background() {
    let mut args = ServeArgs {
        aws_region: Some("us-east-1".to_owned()),
        kafka: crate::cli::KafkaArgs {
            retry_delay: Some("2500ms".to_owned()),
            ..crate::cli::KafkaArgs::default()
        },
        ..ServeArgs::default()
    };
    args.database.url = Some("postgres://app:unused@127.0.0.1:1/app".to_owned());

    let application = application(adapters(), &Config::from_args(args).unwrap()).unwrap();

    assert_eq!(
        application.background.retry_delay,
        Duration::from_millis(2_500)
    );
}

#[tokio::test(start_paused = true)]
async fn a_ready_application_accepts_a_subscription_that_its_router_alone_refuses() {
    let (app, running) = ready(paced()).await;
    let starting = router(adapters(), &config("test")).unwrap();

    let accepted = head(&app, stream("")).await;
    let refused = send(&starting, stream("")).await;
    let status = send(&app, get("/api/events/status")).await;
    shut_down(running).await;
    let closed = send(&app, stream("")).await;

    assert_eq!(accepted.0, StatusCode::OK);
    assert_eq!(accepted.1[header::CONTENT_TYPE], "text/event-stream");
    assert_eq!(refused.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(refused.json()["detail"], "the node is starting");
    assert_eq!(status.json()["state"], "ready");
    assert_eq!(closed.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(closed.json()["detail"], "the node is shutting down");
}

#[tokio::test(start_paused = true)]
async fn a_started_node_whose_watermark_reads_fail_past_the_bound_is_unready_and_still_subscribable()
 {
    let (recorder, _guard) = LogRecorder::install();
    let clock = SettableClock::at(start_time());
    let probe = ScriptedTopicProbe::new();
    probe.push_partition_count(Ok(Some(1)));
    probe.push_watermarks(Ok(marks(0, 0)));
    probe.push_watermarks(Err(unavailable()));
    let tail = ScriptedTopicTail::new();
    tail.push_seek(Ok(()));
    tail.push_fetch(Ok(Batch {
        records: Vec::new(),
        next: Cursor::START,
    }));
    let stale_after = config("test").streams.stale_after;
    let (app, running) = ready(scripted(&probe, &tail, &clock)).await;
    wait_until(|| tail.calls().len() == 3).await;

    let observed = send(&app, get("/api/health")).await;
    clock.set(start_time().checked_add(stale_after).unwrap());
    let at_the_bound = send(&app, get("/api/health")).await;
    let past = stale_after.checked_add(Duration::from_millis(1)).unwrap();
    clock.set(start_time().checked_add(past).unwrap());
    let unreachable = send(&app, get("/api/health")).await;
    let subscription = head(&app, stream("")).await;
    let status = send(&app, get("/api/events/status")).await;
    shut_down(running).await;

    assert_eq!(observed.status, StatusCode::NO_CONTENT, "{}", observed.body);
    assert_eq!(at_the_bound.status, StatusCode::NO_CONTENT);
    assert_eq!(unreachable.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        unreachable.json()["failures"],
        serde_json::json!([{"dependency": "event_stream", "message": "Event stream is not ready"}])
    );
    let failed = probe_failures(&recorder);
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].field("dependency"), Some("event_stream"));
    assert_eq!(
        failed[0].field("cause"),
        Some("probe failed: the topic is unreachable")
    );
    assert_eq!(subscription.0, StatusCode::OK);
    assert_eq!(subscription.1[header::CONTENT_TYPE], "text/event-stream");
    assert_eq!(status.json()["state"], "ready");
    assert_eq!(status.json()["brokerReachable"], false);
}

#[tokio::test(start_paused = true)]
async fn a_started_node_whose_tail_stays_behind_past_the_bound_is_unready_and_still_subscribable() {
    let (recorder, _guard) = LogRecorder::install();
    let clock = SettableClock::at(start_time());
    let probe = ScriptedTopicProbe::new();
    probe.push_partition_count(Ok(Some(1)));
    probe.push_watermarks(Ok(marks(0, 0)));
    probe.push_watermarks(Ok(marks(0, 5)));
    probe.push_watermarks(Ok(marks(0, 5)));
    let tail = ScriptedTopicTail::new();
    tail.push_seek(Ok(()));
    tail.push_fetch(Err(unavailable()));
    tail.push_seek(Ok(()));
    tail.push_fetch(Ok(Batch {
        records: Vec::new(),
        next: Cursor::START,
    }));
    tail.push_seek(Ok(()));
    let stale_after = config("test").streams.stale_after;
    let (app, running) = ready(scripted(&probe, &tail, &clock)).await;

    // The tail caught up at the start time; the round after the retry pause observes the topic at
    // the moved time while its position stays below the end.
    let past = stale_after.checked_add(Duration::from_millis(1)).unwrap();
    clock.set(start_time().checked_add(past).unwrap());
    wait_until(|| tail.calls().len() == 6).await;
    let behind = send(&app, get("/api/health")).await;
    let subscription = head(&app, stream("")).await;
    let status = send(&app, get("/api/events/status")).await;
    shut_down(running).await;

    assert_eq!(
        tail.calls(),
        [
            TailCall::Seek(Cursor::START),
            TailCall::Fetch,
            TailCall::Seek(Cursor::START),
            TailCall::Fetch,
            TailCall::Seek(Cursor::START),
            TailCall::Fetch
        ]
    );
    assert_eq!(behind.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        behind.json()["failures"],
        serde_json::json!([{"dependency": "event_stream", "message": "Event stream is not ready"}])
    );
    let failed = probe_failures(&recorder);
    assert_eq!(failed.len(), 1);
    assert_eq!(
        failed[0].field("cause"),
        Some("probe failed: the event tail is behind")
    );
    assert_eq!(subscription.0, StatusCode::OK);
    assert_eq!(subscription.1[header::CONTENT_TYPE], "text/event-stream");
    assert_eq!(status.json()["state"], "ready");
    assert_eq!(status.json()["position"], "0");
    assert_eq!(status.json()["endOffset"], "5");
}

#[tokio::test(start_paused = true)]
async fn a_cursor_below_the_window_replays_through_the_replay_port_under_the_replay_permits() {
    let probe = ScriptedTopicProbe::new();
    probe.push_partition_count(Ok(Some(1)));
    probe.push_watermarks(Ok(marks(0, 3)));
    let tail = ScriptedTopicTail::new();
    tail.push_seek(Ok(()));
    tail.push_fetch(Ok(three_events()));
    let replay = ScriptedTopicReplay::new();
    let mut adapters = scripted(&probe, &tail, &SettableClock::at(start_time()));
    adapters.topic_replay = Arc::new(replay.clone());
    let mut config = config("test");
    config.streams.window_max_events = 1;
    config.streams.replay_permits = 1;
    let (app, running) = ready_with(adapters, &config).await;

    let first = open(&app, stream("&lastEventId=0")).await;
    let (first_read, first_reader) = read_events(first);
    settle().await;
    let holding = replay.calls();
    let second = open(&app, stream("&lastEventId=0")).await;
    let (second_read, second_reader) = read_events(second);
    settle().await;
    let waiting = replay.calls();
    replay.push_page(Ok(three_events()));
    first_reader.abort();
    settle().await;
    let read = second_read.lock().unwrap().clone();
    second_reader.abort();
    shut_down(running).await;

    assert_eq!(holding, [Cursor::START]);
    assert_eq!(
        waiting,
        [Cursor::START],
        "the second replay waits for the only permit"
    );
    assert_eq!(replay.calls(), [Cursor::START, Cursor::START]);
    assert!(!first_read.lock().unwrap().contains("id: "));
    let at = |id: &str| read.find(&format!("id: {id}\nevent: {TYPE}\n"));
    assert!(at("1").is_some(), "{read}");
    assert!(at("1") < at("2") && at("2") < at("3"), "{read}");
}

/// The cost of the window of an event of [`TYPE`] whose record is `value`: its JSON object (the
/// record itself), its type and 128 bytes (FR-13).
fn window_cost(value: &[u8]) -> usize {
    value
        .len()
        .checked_add(TYPE.len())
        .and_then(|bytes| bytes.checked_add(128))
        .unwrap()
}

#[tokio::test(start_paused = true)]
async fn the_window_keeps_the_newest_events_that_fit_its_byte_bound() {
    let padded = |n: u64| record(n, TYPE, 1_000);
    let probe = ScriptedTopicProbe::new();
    probe.push_partition_count(Ok(Some(1)));
    probe.push_watermarks(Ok(marks(0, 3)));
    let tail = ScriptedTopicTail::new();
    tail.push_seek(Ok(()));
    tail.push_fetch(Ok(Batch {
        records: (0..3)
            .zip(1..=3)
            .map(|(offset, n)| TopicRecord {
                offset: Offset::new(offset),
                value: Some(padded(n)),
            })
            .collect(),
        next: Cursor::new(3),
    }));
    let replay = ScriptedTopicReplay::new();
    let mut adapters = scripted(&probe, &tail, &SettableClock::at(start_time()));
    adapters.topic_replay = Arc::new(replay.clone());
    let mut config = config("test");
    config.streams.window_max_events = 100;
    config.streams.window_max_bytes = window_cost(&padded(1)).checked_mul(2).unwrap();
    let (app, running) = ready_with(adapters, &config).await;

    let held = open(&app, stream("&lastEventId=1")).await;
    let (held_read, held_reader) = read_events(held);
    wait_until(|| held_read.lock().unwrap().contains("id: 3\n")).await;
    let after_held = replay.calls();
    let evicted = open(&app, stream("&lastEventId=0")).await;
    let (evicted_read, evicted_reader) = read_events(evicted);
    settle().await;
    let after_evicted = replay.calls();
    let read = held_read.lock().unwrap().clone();
    held_reader.abort();
    evicted_reader.abort();
    shut_down(running).await;

    assert_eq!(after_held, Vec::<Cursor>::new(), "the window held id 2");
    assert_eq!(after_evicted, [Cursor::START], "the window evicted id 1");
    let at = |id: &str| read.find(&format!("id: {id}\nevent: {TYPE}\n"));
    assert!(at("2").is_some() && at("2") < at("3"), "{read}");
    assert!(!read.contains("id: 1\n"), "{read}");
    assert!(!evicted_read.lock().unwrap().contains("id: "));
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

    let error = register_queries(bus, &repository, &EventCatalog::default()).unwrap_err();

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
fn a_second_event_type_handler_is_a_duplicate_that_names_the_message() {
    let bus = InMemoryQueryBus::new()
        .register(ListEventTypesHandler::new(
            Arc::new(EventCatalog::default()),
        ))
        .unwrap();

    let error = register_queries(bus, &repository(), &EventCatalog::default()).unwrap_err();

    assert!(
        matches!(error, BootstrapError::DuplicateHandler(_)),
        "{error:?}"
    );
    assert!(error.to_string().contains("ListEventTypes"), "{error}");
}

#[tokio::test]
async fn the_event_catalog_of_the_configuration_is_served() {
    let mut args = ServeArgs {
        aws_region: Some("us-east-1".to_owned()),
        event_types: Some("user.created,order.paid,order.paid".to_owned()),
        ..ServeArgs::default()
    };
    args.database.url = Some("postgres://app:unused@127.0.0.1:1/app".to_owned());
    let app = router(adapters(), &Config::from_args(args).unwrap()).unwrap();

    let answer = send(&app, get("/api/event-types")).await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.json(),
        serde_json::json!([{"name": "order.paid"}, {"name": "user.created"}])
    );
}

#[tokio::test]
async fn an_unset_event_catalog_answers_an_empty_array() {
    let answer = send(&app(adapters()), get("/api/event-types")).await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, "[]");
}

#[tokio::test]
async fn the_status_of_a_node_without_a_tail_is_starting_and_catching_up() {
    let app = app(adapters());

    let answer = send(&app, get("/api/events/status")).await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.json(),
        serde_json::json!({
            "state": "starting",
            "reason": "catching-up",
            "brokerReachable": false,
            "position": null,
            "endOffset": null,
            "lag": null,
            "windowFirst": null,
            "windowLast": null,
            "subscribers": 0,
            "rejectedRecords": "0",
            "retentionLosses": "0",
            "replayCapped": "0"
        })
    );
}

#[tokio::test]
async fn the_graphql_schema_carries_the_event_stream_state() {
    let mut args = ServeArgs {
        aws_region: Some("us-east-1".to_owned()),
        event_types: Some("user.created,order.paid,user.deleted".to_owned()),
        ..ServeArgs::default()
    };
    args.database.url = Some("postgres://app:unused@127.0.0.1:1/app".to_owned());
    let app = router(adapters(), &Config::from_args(args).unwrap()).unwrap();

    let answer = send(
        &app,
        graphql(
            "{ eventTypes(page: 2, itemsPerPage: 1) { name } \
             eventStreamStatus { state reason subscribers } }",
        ),
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.json(),
        serde_json::json!({
            "data": {
                "eventTypes": [{"name": "user.created"}],
                "eventStreamStatus": {"state": "starting", "reason": "catching-up", "subscribers": 0}
            }
        })
    );
}

#[test]
fn the_database_error_keeps_its_message_and_source() {
    let source = sqlx::Error::Configuration("bad url".into());

    let error = BootstrapError::from(source);

    assert_eq!(error.to_string(), "invalid database settings");
    assert!(std::error::Error::source(&error).is_some());
}

#[tokio::test]
async fn the_event_stream_of_a_router_answers_the_503_problem_of_a_starting_node() {
    let (recorder, _guard) = LogRecorder::install();

    let answer = send(
        &app(adapters()),
        get("/api/events/stream?types=user.created"),
    )
    .await;

    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert_eq!(
        answer.json(),
        serde_json::json!({
            "type": "about:blank",
            "title": "Service Unavailable",
            "status": 503,
            "detail": "the node is starting"
        })
    );
    let rejected: Vec<_> = recorder
        .lines()
        .into_iter()
        .filter(|line| line.message == "subscription rejected")
        .collect();
    assert_eq!(rejected.len(), 1);
    assert_eq!(rejected[0].field("detail"), Some("the node is starting"));
}

#[tokio::test]
async fn the_event_stream_of_a_router_validates_before_it_answers_the_503() {
    let app = app(adapters());

    let missing = send(&app, get("/api/events/stream")).await;
    let invalid = send(&app, get("/api/events/stream?types=User.Created")).await;
    let cursor = send(&app, get("/api/events/stream?types=a&lastEventId=x")).await;

    assert_eq!(missing.status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid.json()["violations"][0]["code"], "pattern");
    assert_eq!(cursor.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_router_serves_with_scripted_topic_readers_and_reads_none_of_them() {
    let probe = ScriptedTopicProbe::new();
    let tail = ScriptedTopicTail::new();
    let replay = ScriptedTopicReplay::new();
    let mut adapters = adapters();
    adapters.topic_probe = Arc::new(probe.clone());
    adapters.topic_tail = Box::new(tail.clone());
    adapters.topic_replay = Arc::new(replay.clone());
    let app = app(adapters);

    let status = send(&app, get("/api/events/status")).await;
    let stream = send(&app, get("/api/events/stream?types=a&lastEventId=0")).await;

    assert_eq!(status.status, StatusCode::OK);
    assert_eq!(stream.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(probe.calls(), Vec::<ProbeCall>::new());
    assert_eq!(tail.calls(), Vec::<TailCall>::new());
    assert_eq!(replay.calls(), Vec::<Cursor>::new());
}
