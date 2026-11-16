//! The world of a scenario: injected faults, the configured event types and variables, the last
//! response, and the event stream of a scenario: a topic of its own on the broker of the stack with
//! a producer, a node that reads it, and the SSE and WebSocket clients of that node.
//!
//! Without a node of its own, a new application instance is built for every request; its tail
//! reads the topic `notifications` of the broker of the stack, and the request is sent once the
//! node is ready. Once the scenario started a node, every request goes to that node.

use std::ffi::OsStr;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use clap::{CommandFactory as _, Parser as _};
use futures_util::{SinkExt as _, StreamExt as _};
use health_check_domain::Dependency;
use http_body_util::BodyExt as _;
use krafka::Kafka;
use krafka::admin::{CreateTopicsOptions, DeleteTopicsOptions, NewTopic};
use krafka::producer::{Acks, Producer, Record};
use serde_json::Value;
use service::background::Running;
use service::bootstrap::{Adapters, application};
use service::cli::{Cli, Command};
use service::config::Config;
use test_support::probe::StubProbe;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};
use tower::ServiceExt as _;

/// Counter that makes the topic names of the scenarios of one process unique.
static TOPICS: AtomicUsize = AtomicUsize::new(0);

/// The topic of the broker of the stack.
const STACK_TOPIC: &str = "notifications";

/// How long a request waits for its node to become ready, and for its node to shut down.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// The request time-out of every node of the suite. The production default is a few seconds; a
/// scenario must succeed however long the machine needs to open the first connection.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// How long a request waits for the whole answer of a node: the request time-out of the node,
/// which bounds the head of every answer, and a margin for the body. A body that never ends, such
/// as an SSE stream kept open by its keep-alives, fails the step at this limit.
const ANSWER_TIMEOUT: Duration = REQUEST_TIMEOUT.saturating_add(Duration::from_secs(10));

/// The environment variables whose value [`config_with`], [`AppWorld::config`] or
/// [`AppWorld::start_node`] replaces after parsing; a scenario cannot set them.
const SUITE_VARIABLES: [&str; 6] = [
    "SQS_CONNECT_TIMEOUT",
    "SQS_OPERATION_TIMEOUT",
    "HEALTH_PROBE_TIMEOUT",
    "HTTP_REQUEST_TIMEOUT",
    "KAFKA_BOOTSTRAP_SERVERS",
    "KAFKA_TOPIC",
];

/// How long a step waits for the broker, for a frame of a client and for the status of its node
/// to show an expected value.
const STREAM_TIMEOUT: Duration = Duration::from_secs(30);

/// The request time-out of the client of the broker of the stack.
const BROKER_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// How often a step reads the status of its node while it waits for a value.
const STATUS_POLL: Duration = Duration::from_millis(50);

/// The payload of the ping a WebSocket command step sends after its command.
const BARRIER: &str = "barrier";

/// The value of a record that is not an event: it is not JSON.
const INVALID_RECORD: &str = "this record is not an event";

/// The bootstrap servers of the broker of the stack.
#[expect(
    clippy::disallowed_methods,
    reason = "the end-to-end suite runs against the broker that the environment of the stack names"
)]
fn stack_broker() -> String {
    std::env::var("KAFKA_BOOTSTRAP_SERVERS")
        .expect("KAFKA_BOOTSTRAP_SERVERS is not set; run the suite through `make e2e-tests`")
}

/// What the last request answered.
#[derive(Debug)]
pub(crate) struct Reply {
    pub(crate) status: u16,
    pub(crate) headers: HeaderMap,
    pub(crate) body: String,
}

impl Reply {
    /// The body as JSON, `Null` when it is not JSON.
    pub(crate) fn json(&self) -> Value {
        serde_json::from_str(&self.body).unwrap_or(Value::Null)
    }
}

/// A valid event the scenario wrote to its topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Produced {
    /// Its id on every node: the offset of its record plus 1.
    pub(crate) cursor: u64,
    /// Its type.
    pub(crate) event_type: String,
    /// The value of its record, which is also the event object the node writes: the members
    /// `id`, `type`, `occurredAt` and `data` in this order, without white space.
    pub(crate) object: String,
}

/// The fields of one SSE frame as a client reads them.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct SseEvent {
    /// The `id` field.
    pub(crate) id: Option<String>,
    /// The `event` field.
    pub(crate) event: Option<String>,
    /// The `data` fields, joined by line breaks.
    pub(crate) data: Option<String>,
}

impl SseEvent {
    /// The fields of `block`, the lines of one frame up to the empty line that ends it; `None` for
    /// a frame of comments only, such as the keep-alive.
    fn parse(block: &str) -> Option<Self> {
        let mut event = Self::default();
        let mut any = false;
        let fields = block
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with(':'));
        for line in fields {
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value).to_owned();
            match field {
                "id" => event.id = Some(value),
                "event" => event.event = Some(value),
                "data" => {
                    event.data = Some(match event.data.take() {
                        Some(data) => format!("{data}\n{value}"),
                        None => value,
                    });
                }
                other => panic!("unexpected SSE field {other:?} in {block:?}"),
            }
            any = true;
        }
        any.then_some(event)
    }
}

/// An open SSE response of the node, and the bytes read from it that do not end a frame yet.
///
/// The body is `Send` but not `Sync`; behind the mutex the world stays `Sync`, so the futures of
/// the steps that borrow it stay `Send`. The body is only reached through `Mutex::get_mut`, so no
/// lock is ever taken.
#[derive(Debug)]
struct SseStream {
    body: Mutex<Body>,
    unread: Vec<u8>,
}

impl SseStream {
    /// The next event of the stream, within [`STREAM_TIMEOUT`] for the whole wait: keep-alives are
    /// passed over and do not extend it.
    async fn next_event(&mut self) -> SseEvent {
        bounded(async {
            loop {
                if let Some(event) = self.take_event() {
                    return event;
                }
                let body = self.body.get_mut().expect("the body is never locked");
                let frame = body
                    .frame()
                    .await
                    .expect("the SSE stream stays open")
                    .expect("the SSE stream is readable");
                if let Ok(data) = frame.into_data() {
                    self.unread.extend_from_slice(&data);
                }
            }
        })
        .await
    }

    /// The first whole event of the bytes read, when there is one.
    fn take_event(&mut self) -> Option<SseEvent> {
        while let Some(end) = self.unread.windows(2).position(|pair| pair == b"\n\n") {
            let block: Vec<u8> = self.unread.drain(..end.saturating_add(2)).collect();
            let text = std::str::from_utf8(&block).expect("SSE frames are UTF-8");
            if let Some(event) = SseEvent::parse(text) {
                return Some(event);
            }
        }
        None
    }
}

/// A WebSocket client of the node.
type WebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// The connections of a scenario to the broker of the stack: the client that created the topic of
/// the scenario, and the producer that writes to it.
#[derive(Debug)]
struct Broker {
    kafka: Kafka,
    producer: Producer,
}

impl Broker {
    /// Creates `topic` with one partition, and a producer that waits for every replica.
    async fn create(topic: &str) -> Self {
        let kafka = bounded(
            Kafka::builder(stack_broker())
                .client_id("e2e-tests")
                .request_timeout(BROKER_REQUEST_TIMEOUT)
                .connect(),
        )
        .await
        .expect("the broker of the stack is reachable");
        let created = bounded(kafka.admin().create_topics(
            [NewTopic::new(topic, 1, 1).expect("a valid topic")],
            CreateTopicsOptions::default(),
        ))
        .await
        .expect("create the topic of the scenario");
        created[topic]
            .clone()
            .expect("the topic of the scenario is created");
        let producer = bounded(
            kafka
                .producer()
                .acks(Acks::All)
                .linger(Duration::ZERO)
                .build(),
        )
        .await
        .expect("a producer");
        Self { kafka, producer }
    }

    /// Writes `value` to partition 0 of `topic` and returns the offset of the record.
    async fn produce(&self, topic: &str, value: String) -> u64 {
        let record = Record::new(topic, value.into_bytes()).partition(0);
        let delivered = bounded(self.producer.send(record))
            .await
            .expect("the broker takes the record");
        assert_eq!(delivered.partition, 0);
        u64::try_from(delivered.offset).expect("the broker names the offset")
    }

    /// Closes the producer and deletes `topic`.
    async fn delete(self, topic: &str) {
        bounded(self.producer.close())
            .await
            .expect("close the producer");
        let deleted = bounded(
            self.kafka
                .admin()
                .delete_topics([topic], DeleteTopicsOptions::default()),
        )
        .await
        .expect("delete the topic of the scenario");
        deleted[topic]
            .clone()
            .expect("the topic of the scenario is deleted");
    }
}

/// The node of a scenario: an application whose tail reads the topic of the scenario. Its router
/// takes the requests of the scenario and is also served on a loopback port, where a WebSocket can
/// upgrade.
#[derive(Debug)]
struct Node {
    router: Router,
    address: SocketAddr,
    running: Running,
    server: JoinHandle<std::io::Result<()>>,
}

impl Node {
    /// Assembles the application of `adapters`, runs its tail and serves its router.
    async fn start(adapters: Adapters, config: &Config) -> Self {
        let application = application(adapters, config).expect("assembled application");
        let running = application.background.spawn();
        let listener = listen().await;
        let address = listener.local_addr().expect("a bound address");
        let server = tokio::spawn(axum::serve(listener, application.router.clone()).into_future());
        Self {
            router: application.router,
            address,
            running,
            server,
        }
    }

    /// Shuts the background work down (every stream ends), then stops the server.
    async fn stop(self) {
        shut_down(self.running).await;
        self.server.abort();
    }
}

/// Shuts the background work of a node down (every stream ends), within [`READY_TIMEOUT`].
async fn shut_down(running: Running) {
    tokio::time::timeout(READY_TIMEOUT, running.shutdown())
        .await
        .expect("the node shuts down");
}

#[expect(
    clippy::disallowed_methods,
    reason = "the end-to-end suite serves the router of its node on a loopback port, where a WebSocket can upgrade"
)]
async fn listen() -> TcpListener {
    TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a loopback port")
}

/// Awaits `future` within [`STREAM_TIMEOUT`].
async fn bounded<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(STREAM_TIMEOUT, future)
        .await
        .expect("the call ends within the time limit")
}

/// One instance per scenario; cucumber builds it with `Default`.
#[derive(Debug, Default, cucumber::World)]
pub(crate) struct AppWorld {
    failing: Vec<Dependency>,
    event_types: Option<String>,
    variables: Vec<(String, String)>,
    reply: Option<Reply>,
    /// The topic of the node of the scenario.
    topic: Option<String>,
    /// Set once the topic of the scenario was created.
    broker: Option<Broker>,
    node: Option<Node>,
    /// The valid events the scenario wrote, in offset order.
    produced: Vec<Produced>,
    /// How many of [`Self::produced`] a step already compared with what a client received.
    checked: usize,
    /// The id of the last record the scenario wrote, valid or not.
    written: u64,
    sse_path: Option<String>,
    sse: Option<SseStream>,
    /// The id of the last SSE event received, kept across a disconnect.
    last_id: Option<String>,
    websocket: Option<WebSocket>,
}

/// The configuration as the binary reads it from the environment and the extra flags.
fn config_with(flags: &[String]) -> Config {
    let arguments = ["service", "serve"]
        .map(str::to_owned)
        .into_iter()
        .chain(flags.iter().cloned());
    // A flag no argument takes fails the scenario that set it, not the whole run.
    let Command::Serve(args) = Cli::try_parse_from(arguments)
        .expect("the serve command takes the flags of the scenario")
        .command
    else {
        panic!("`serve` arguments expected");
    };
    let mut config = Config::from_args(args).expect("valid configuration");
    // The production defaults are a few seconds. A scenario must succeed however long the machine
    // needs to open the first connection, so the budgets are long and keep the order the
    // configuration requires (request > probe >= operation). Every value replaced here is named
    // in `SUITE_VARIABLES`.
    config.sqs.connect_timeout = Duration::from_secs(10);
    config.sqs.operation_timeout = Duration::from_secs(30);
    config.health_probe_timeout = Duration::from_secs(40);
    config.request_timeout = REQUEST_TIMEOUT;
    config.kafka.bootstrap_servers = stack_broker();
    config.kafka.topic = STACK_TOPIC.to_owned();
    config
}

/// The long flag of the argument of the serve command that reads the environment variable `name`;
/// a value given on the command line wins over the environment.
fn serve_flag(name: &str) -> String {
    let command = Cli::command();
    let serve = command
        .find_subcommand("serve")
        .expect("the binary has a serve command");
    let long = serve
        .get_arguments()
        .find(|argument| argument.get_env() == Some(OsStr::new(name)))
        .and_then(clap::Arg::get_long)
        .unwrap_or_else(|| {
            panic!("no argument of the serve command reads the environment variable {name}")
        });
    format!("--{long}")
}

/// The record value of the valid event `number` of `event_type`, which is also its event object.
fn envelope(number: usize, event_type: &str) -> String {
    format!(
        r#"{{"id":"00000000-0000-4000-8000-{number:012x}","type":"{event_type}","occurredAt":"2026-11-14T10:00:00Z","data":{{"n":{number}}}}}"#
    )
}

/// The id in the string member `field` of a status, `None` when it is null.
fn id_field(status: &Value, field: &str) -> Option<u64> {
    status.get(field)?.as_str()?.parse().ok()
}

/// Sends `request` to `router` and reads the whole answer, within [`ANSWER_TIMEOUT`] for the head
/// and the body together.
async fn dispatch(router: &Router, request: Request<Body>) -> Reply {
    tokio::time::timeout(ANSWER_TIMEOUT, async {
        let response = router.clone().oneshot(request).await.expect("infallible");
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        Reply {
            status,
            headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    })
    .await
    .expect("the node sends its whole answer within the time limit")
}

/// The answer of `GET /api/events/status` of the node of `router`.
async fn status_of(router: &Router) -> Value {
    let request = Request::get("/api/events/status")
        .body(Body::empty())
        .expect("valid request");
    dispatch(router, request).await.json()
}

/// Reads the status of the node of `router` every [`STATUS_POLL`] until `holds` accepts it, at
/// most `limit`; `what` names the awaited value in the failure.
async fn await_status<P>(router: &Router, limit: Duration, what: &str, holds: P)
where
    P: Fn(&Value) -> bool + Send + Sync,
{
    let mut last = Value::Null;
    let reached = tokio::time::timeout(limit, async {
        loop {
            last = status_of(router).await;
            if holds(&last) {
                return;
            }
            tokio::time::sleep(STATUS_POLL).await;
        }
    })
    .await;
    assert!(reached.is_ok(), "the status never showed {what}: {last}");
}

/// Waits until the node of `router` reports `ready`, at most [`READY_TIMEOUT`].
async fn wait_until_ready(router: &Router) {
    await_status(router, READY_TIMEOUT, "the state ready", |status| {
        status["state"] == "ready"
    })
    .await;
}

impl AppWorld {
    /// Marks a dependency as failing: its probe is replaced before the next request.
    pub(crate) fn fail(&mut self, dependency: Dependency) {
        self.failing.push(dependency);
    }

    /// Sets `EVENT_TYPES` of the next requests and nodes.
    pub(crate) fn configure_event_types(&mut self, value: &str) {
        self.event_types = Some(value.to_owned());
    }

    /// Gives the serve command of the next requests and nodes `value` for the environment variable
    /// `name`: the value goes to the argument that reads that variable. A variable the suite sets
    /// itself ([`SUITE_VARIABLES`]) is refused, since its value would be discarded.
    pub(crate) fn set_variable(&mut self, name: &str, value: &str) {
        assert!(
            !SUITE_VARIABLES.contains(&name),
            "the suite sets {name} itself and would discard the value of the scenario"
        );
        self.variables.push((serve_flag(name), value.to_owned()));
    }

    /// The last response.
    pub(crate) fn reply(&self) -> &Reply {
        self.reply.as_ref().expect("a request was sent")
    }

    /// Closes the clients, stops the node and deletes the topic of the scenario, as far as the
    /// scenario made them. The runner calls it after every scenario.
    pub(crate) async fn end_event_stream(&mut self) {
        self.sse = None;
        self.websocket = None;
        if let Some(node) = self.node.take() {
            node.stop().await;
        }
        if let (Some(broker), Some(topic)) = (self.broker.take(), self.topic.as_deref()) {
            broker.delete(topic).await;
        }
    }

    /// The flags of the configured event types and variables.
    fn flags(&self) -> Vec<String> {
        let event_types = self
            .event_types
            .iter()
            .flat_map(|value| ["--event-types".to_owned(), value.clone()]);
        let variables = self
            .variables
            .iter()
            .flat_map(|(flag, value)| [flag.clone(), value.clone()]);
        event_types.chain(variables).collect()
    }

    /// The configuration of a node of the scenario: the configured flags.
    fn config(&self) -> Config {
        config_with(&self.flags())
    }

    /// The production wiring with the failing probes of the scenario.
    async fn adapters(&self, config: &Config) -> Adapters {
        let mut adapters = Adapters::connect(config).await;
        for dependency in &self.failing {
            adapters = adapters.with_probe(Arc::new(StubProbe::failing(*dependency)));
        }
        adapters
    }

    /// Sends one request: to the node of the scenario when there is one, else to a new application
    /// instance.
    pub(crate) async fn send(
        &mut self,
        method: &str,
        path: &str,
        content_type: Option<&str>,
        body: &str,
    ) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(content_type) = content_type {
            request = request.header(header::CONTENT_TYPE, content_type);
        }
        let request = request
            .body(Body::from(body.to_owned()))
            .expect("valid request");
        let reply = if let Some(node) = &self.node {
            dispatch(&node.router, request).await
        } else {
            self.send_to_new_instance(request).await
        };
        self.reply = Some(reply);
    }

    /// Builds a new application instance, runs its tail until the node is ready, sends `request`
    /// to it and shuts its background work down.
    async fn send_to_new_instance(&self, request: Request<Body>) -> Reply {
        let config = self.config();
        let adapters = self.adapters(&config).await;
        let application = application(adapters, &config).expect("assembled application");
        let running = application.background.spawn();
        wait_until_ready(&application.router).await;
        let reply = dispatch(&application.router, request).await;
        shut_down(running).await;
        reply
    }

    /// Creates a topic of one partition for the scenario, starts the node of the scenario on it
    /// and waits until the node is ready.
    pub(crate) async fn start_ready_node(&mut self) {
        let topic = self.name_topic();
        self.broker = Some(Broker::create(&topic).await);
        self.start_node(topic).await;
        wait_until_ready(&self.node().router).await;
    }

    /// Starts the node of the scenario on a topic that does not exist; the node stays starting.
    pub(crate) async fn start_node_without_topic(&mut self) {
        let topic = self.name_topic();
        self.start_node(topic).await;
    }

    /// A topic name no other scenario and no earlier run of this process uses.
    fn name_topic(&mut self) -> String {
        let topic = format!(
            "e2e.{}.{}",
            std::process::id(),
            TOPICS.fetch_add(1, Ordering::SeqCst)
        );
        self.topic = Some(topic.clone());
        topic
    }

    /// Starts the node of the scenario on `topic`.
    async fn start_node(&mut self, topic: String) {
        assert!(self.node.is_none(), "the scenario already has a node");
        let mut config = self.config();
        config.kafka.topic = topic;
        let adapters = self.adapters(&config).await;
        self.node = Some(Node::start(adapters, &config).await);
    }

    /// The node of the scenario.
    fn node(&self) -> &Node {
        self.node.as_ref().expect("the scenario started a node")
    }

    /// Writes `value` to the topic of the scenario and returns the id of its record.
    async fn write(&mut self, value: String) -> u64 {
        let topic = self.topic.as_deref().expect("the scenario has a topic");
        let offset = self
            .broker
            .as_ref()
            .expect("the topic of the scenario exists")
            .produce(topic, value)
            .await;
        self.written = offset.checked_add(1).expect("an id below u64::MAX");
        self.written
    }

    /// Writes a valid event of `event_type`.
    pub(crate) async fn produce_event(&mut self, event_type: &str) {
        let number = self.produced.len().saturating_add(1);
        let object = envelope(number, event_type);
        let cursor = self.write(object.clone()).await;
        self.produced.push(Produced {
            cursor,
            event_type: event_type.to_owned(),
            object,
        });
    }

    /// Writes a record whose value is not an event.
    pub(crate) async fn produce_invalid(&mut self) {
        self.write(INVALID_RECORD.to_owned()).await;
    }

    /// The valid events the scenario wrote, in offset order.
    pub(crate) fn produced(&self) -> &[Produced] {
        &self.produced
    }

    /// The events of `event_type` written since the last call, in offset order; at least one.
    pub(crate) fn take_unchecked(&mut self, event_type: &str) -> Vec<Produced> {
        let unchecked: Vec<Produced> = self.produced[self.checked..]
            .iter()
            .filter(|event| event.event_type == event_type)
            .cloned()
            .collect();
        self.checked = self.produced.len();
        assert!(
            !unchecked.is_empty(),
            "no {event_type} event was written since the last comparison"
        );
        unchecked
    }

    /// Reads the status of the node of the scenario until `holds` accepts it, at most
    /// [`STREAM_TIMEOUT`]; `what` names the awaited value in the failure.
    pub(crate) async fn wait_for_status<P>(&self, what: &str, holds: P)
    where
        P: Fn(&Value) -> bool + Send + Sync,
    {
        await_status(&self.node().router, STREAM_TIMEOUT, what, holds).await;
    }

    /// Waits until the position of the node passed every record the scenario wrote.
    pub(crate) async fn wait_until_read(&self) {
        let written = self.written;
        self.wait_for_status(&format!("a position of at least {written}"), |status| {
            id_field(status, "position").is_some_and(|position| position >= written)
        })
        .await;
    }

    /// The floor of the memory window of the node: it holds the events with a higher id.
    pub(crate) async fn window_floor(&self) -> u64 {
        let status = status_of(&self.node().router).await;
        id_field(&status, "windowFirst").unwrap_or_else(|| panic!("no window floor in {status}"))
    }

    /// Opens an SSE stream on `path` of the node, with `last_event_id` in the `Last-Event-ID`
    /// header when given; the answer must be 200.
    pub(crate) async fn open_sse(&mut self, path: &str, last_event_id: Option<&str>) {
        let mut request = Request::get(path);
        if let Some(id) = last_event_id {
            request = request.header("last-event-id", id);
        }
        let request = request.body(Body::empty()).expect("valid request");
        let response = bounded(self.node().router.clone().oneshot(request))
            .await
            .expect("infallible");
        let status = response.status();
        let body = response.into_body();
        if status != StatusCode::OK {
            let bytes = bounded(body.collect()).await.expect("body").to_bytes();
            panic!(
                "the SSE request answered {status}: {}",
                String::from_utf8_lossy(&bytes)
            );
        }
        self.sse_path = Some(path.to_owned());
        self.sse = Some(SseStream {
            body: Mutex::new(body),
            unread: Vec::new(),
        });
    }

    /// The next event of the SSE stream, whose id becomes the last id received.
    pub(crate) async fn next_sse_event(&mut self) -> SseEvent {
        let event = self
            .sse
            .as_mut()
            .expect("an SSE stream is open")
            .next_event()
            .await;
        if event.id.is_some() {
            self.last_id.clone_from(&event.id);
        }
        event
    }

    /// Closes the SSE stream; the last id received is kept.
    pub(crate) fn disconnect_sse(&mut self) {
        assert!(self.sse.take().is_some(), "no SSE stream is open");
    }

    /// Opens the SSE stream on its path again, with the last id received in `Last-Event-ID`.
    pub(crate) async fn reconnect_sse(&mut self) {
        let path = self.sse_path.clone().expect("an SSE stream was open");
        let last_id = self
            .last_id
            .clone()
            .expect("the SSE stream received an event");
        self.open_sse(&path, Some(&last_id)).await;
    }

    /// Opens a WebSocket on `path` of the served node; the upgrade must answer 101.
    pub(crate) async fn open_websocket(&mut self, path: &str) {
        let url = format!("ws://{}{path}", self.node().address);
        let (socket, response) = bounded(connect_async(url))
            .await
            .expect("the WebSocket opens");
        assert_eq!(response.status().as_u16(), 101);
        self.websocket = Some(socket);
    }

    /// The WebSocket of the scenario.
    fn websocket(&mut self) -> &mut WebSocket {
        self.websocket.as_mut().expect("a WebSocket is open")
    }

    /// Sends `command` as a text frame, then a ping, and waits for its pong, within
    /// [`STREAM_TIMEOUT`] for the whole step: pings of the node do not extend it. The node reads the
    /// messages of a connection in order and answers a ping after it handled what came before, so
    /// the command has taken effect once the pong arrives; the node answers a valid command with
    /// nothing.
    pub(crate) async fn send_command(&mut self, command: String) {
        let socket = self.websocket();
        bounded(async {
            socket
                .send(Message::text(command))
                .await
                .expect("the command is sent");
            socket
                .send(Message::Ping(BARRIER.into()))
                .await
                .expect("the ping is sent");
            loop {
                match socket.next().await {
                    Some(Ok(Message::Pong(payload))) if payload.as_ref() == BARRIER.as_bytes() => {
                        return;
                    }
                    Some(Ok(Message::Ping(_payload))) => {}
                    other => panic!("the node sent {other:?} before the pong of the command"),
                }
            }
        })
        .await;
    }

    /// The next text frame of the WebSocket, within [`STREAM_TIMEOUT`] for the whole wait: pings
    /// and pongs are passed over and do not extend it.
    pub(crate) async fn next_text(&mut self) -> String {
        let socket = self.websocket();
        bounded(async {
            loop {
                match socket.next().await {
                    Some(Ok(Message::Text(text))) => return text.as_str().to_owned(),
                    Some(Ok(Message::Ping(_payload) | Message::Pong(_payload))) => {}
                    other => panic!("not a text frame: {other:?}"),
                }
            }
        })
        .await
    }
}
