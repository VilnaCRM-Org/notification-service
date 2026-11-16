//! Typed configuration of the service, parsed and validated from the command-line arguments.
//!
//! [`Config::from_args`] collects every problem instead of stopping at the first one. A problem
//! names the environment variable and a reason that never contains a value, so a secret that is
//! malformed is not echoed.

use std::collections::BTreeSet;
use std::fmt::Display;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::ops::RangeInclusive;
use std::str::FromStr;
use std::time::Duration;

use axum::http::uri::Authority;
use axum::http::{HeaderValue, Uri};
use event_stream_application::EventCatalog;
use event_stream_domain::EventType;
use event_stream_infrastructure::{KafkaSecurity, KafkaSettings, SaslCredentials, SaslMechanism};
use shared_infrastructure::messaging::SqsSettings;
use tracing_subscriber::EnvFilter;

use crate::cli::{KafkaArgs, ServeArgs, StreamArgs};

/// Deployment environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    /// Local development: the GraphQL explorer is served.
    Dev,
    /// Automated tests.
    Test,
    /// Production.
    Prod,
}

/// One invalid or missing variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProblem {
    /// Name of the environment variable.
    pub variable: &'static str,
    /// What is wrong with it; never contains the value.
    pub reason: String,
}

/// Every problem of a configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the configuration is invalid ({} problem(s))", .0.len())]
pub struct ConfigErrors(Vec<ConfigProblem>);

impl ConfigErrors {
    /// The problems in the order of the variables of the configuration table; the rules that
    /// relate two variables come last.
    #[must_use]
    pub fn problems(&self) -> &[ConfigProblem] {
        &self.0
    }
}

/// The validated configuration of `serve` and `check-config`.
#[derive(Debug, Clone)]
pub struct Config {
    /// Deployment environment.
    pub app_env: AppEnv,
    /// Address to listen on.
    pub host: IpAddr,
    /// Port to listen on; 0 selects any free port.
    pub port: u16,
    /// `tracing` filter directives.
    pub log_level: String,
    /// SQS client settings.
    pub sqs: SqsSettings,
    /// Timeout of one health probe.
    pub health_probe_timeout: Duration,
    /// Budget of one HTTP request.
    pub request_timeout: Duration,
    /// Largest accepted request body in bytes.
    pub max_body_bytes: usize,
    /// Time the in-flight requests get to finish after a termination signal.
    pub shutdown_grace_period: Duration,
    /// Settings of the Kafka connection.
    pub kafka: KafkaSettings,
    /// Settings of the event stream.
    pub streams: StreamConfig,
    /// The browser origins whose pages may use the service (`CORS_ALLOWED_ORIGINS`).
    pub allowed_origins: AllowedOrigins,
}

/// Settings of the event stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamConfig {
    /// The subscribable event types (`EVENT_TYPES`).
    pub catalog: EventCatalog,
    /// Largest record value in bytes (`EVENT_MAX_DATA_BYTES`).
    pub max_data_bytes: usize,
    /// Events in the memory window (`EVENT_WINDOW_MAX_EVENTS`).
    pub window_max_events: usize,
    /// Bytes in the memory window (`EVENT_WINDOW_MAX_BYTES`).
    pub window_max_bytes: usize,
    /// Offsets below the end of the topic read at start (`EVENT_WINDOW_START_EVENTS`).
    pub start_events: u64,
    /// Offsets below the tail a replay may reach (`EVENT_REPLAY_MAX_EVENTS`).
    pub replay_max_events: u64,
    /// Concurrent replay reads of the node (`EVENT_REPLAY_PERMITS`).
    pub replay_permits: usize,
    /// Pause after a failed round of the tail (`KAFKA_RETRY_DELAY`).
    pub retry_delay: Duration,
    /// Age at which the readiness probe of the tail fails (`EVENT_TAIL_STALE_AFTER`).
    pub stale_after: Duration,
    /// Queued events per subscriber (`SUBSCRIBER_BUFFER`).
    pub buffer: usize,
    /// Subscribers of the node (`MAX_SUBSCRIBERS`).
    pub max_subscribers: usize,
    /// Keep-alive, ping, send bound and write deadline of the streams (`STREAM_KEEP_ALIVE`).
    pub keep_alive: Duration,
}

/// The browser origins whose pages may use the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AllowedOrigins {
    /// No other origin: no CORS layer, and a WebSocket handshake with an `Origin` is refused.
    NoOrigin,
    /// Every origin (`*`).
    Any,
    /// These origins, compared byte for byte with the `Origin` header.
    List(Vec<HeaderValue>),
}

impl AllowedOrigins {
    /// Whether a request of `origin` is allowed.
    #[must_use]
    pub fn allows(&self, origin: &HeaderValue) -> bool {
        match self {
            Self::NoOrigin => false,
            Self::Any => true,
            Self::List(origins) => origins.contains(origin),
        }
    }
}

impl Config {
    /// Parses and validates every setting, including the rules between settings.
    ///
    /// # Errors
    ///
    /// Returns every problem found, each naming its environment variable.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "the arguments are handed over to the configuration (AD-07)"
    )]
    pub fn from_args(args: ServeArgs) -> Result<Self, ConfigErrors> {
        let mut collector = Collector::default();
        let server = server(&mut collector, &args);
        let sqs = sqs(&mut collector, &args);
        let limits = limits(&mut collector, &args);
        let kafka = kafka_fields(&mut collector, &args.kafka);
        let streams = stream_settings(&mut collector, &args);
        let allowed_origins = allowed_origins(&mut collector, &args);
        collector.cross_field_rules(
            sqs.as_ref().map(|settings| settings.operation_timeout),
            limits.as_ref().map(|limits| limits.health_probe_timeout),
            limits.as_ref().map(|limits| limits.request_timeout),
        );
        let parts = Parts {
            kafka: kafka_settings(&mut collector, &args.kafka, kafka),
            server,
            sqs,
            limits,
            streams,
            allowed_origins,
        };
        match parts.complete() {
            Some(config) if collector.problems.is_empty() => Ok(config),
            _ => Err(ConfigErrors(collector.problems)),
        }
    }
}

/// The groups of a configuration, each `None` when one of its variables is invalid.
struct Parts {
    server: Option<Server>,
    sqs: Option<SqsSettings>,
    limits: Option<Limits>,
    kafka: Option<KafkaSettings>,
    streams: Option<StreamConfig>,
    allowed_origins: Option<AllowedOrigins>,
}

impl Parts {
    /// The configuration when every group is valid.
    fn complete(self) -> Option<Config> {
        let (server, limits) = (self.server?, self.limits?);
        Some(Config {
            app_env: server.app_env,
            host: server.host,
            port: server.port,
            log_level: server.log_level,
            sqs: self.sqs?,
            health_probe_timeout: limits.health_probe_timeout,
            request_timeout: limits.request_timeout,
            max_body_bytes: limits.max_body_bytes,
            shutdown_grace_period: limits.shutdown_grace_period,
            kafka: self.kafka?,
            streams: self.streams?,
            allowed_origins: self.allowed_origins?,
        })
    }
}

struct Server {
    app_env: AppEnv,
    host: IpAddr,
    port: u16,
    log_level: String,
}

struct Limits {
    health_probe_timeout: Duration,
    request_timeout: Duration,
    max_body_bytes: usize,
    shutdown_grace_period: Duration,
}

type Parsed<T> = Result<T, String>;

/// Gathers the problems of all variables.
#[derive(Default)]
struct Collector {
    problems: Vec<ConfigProblem>,
}

impl Collector {
    fn collect<T>(&mut self, variable: &'static str, outcome: Parsed<T>) -> Option<T> {
        outcome
            .map_err(|reason| self.problems.push(ConfigProblem { variable, reason }))
            .ok()
    }

    fn credentials(&mut self, key_id: Option<&str>, secret: Option<&str>) {
        let problem = match (key_id.is_some(), secret.is_some()) {
            (true, false) => Some(("AWS_SECRET_ACCESS_KEY", "AWS_ACCESS_KEY_ID")),
            (false, true) => Some(("AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY")),
            (true, true) | (false, false) => None,
        };
        if let Some((missing, present)) = problem {
            self.problems.push(ConfigProblem {
                variable: missing,
                reason: format!("is required when {present} is set"),
            });
        }
    }

    fn cross_field_rules(
        &mut self,
        operation: Option<Duration>,
        probe: Option<Duration>,
        request: Option<Duration>,
    ) {
        self.at_least(
            "HEALTH_PROBE_TIMEOUT",
            probe,
            "SQS_OPERATION_TIMEOUT",
            operation,
        );
        self.longer_than(
            "HTTP_REQUEST_TIMEOUT",
            request,
            "HEALTH_PROBE_TIMEOUT",
            probe,
        );
    }

    /// `KAFKA_CONNECT_TIMEOUT` must not be longer than `KAFKA_REQUEST_TIMEOUT`; skipped when either
    /// is invalid.
    fn connect_within_request(&mut self, connect: Option<Duration>, request: Option<Duration>) {
        if let (Some(connect), Some(request)) = (connect, request)
            && connect > request
        {
            self.problems.push(ConfigProblem {
                variable: "KAFKA_CONNECT_TIMEOUT",
                reason: "must not be longer than KAFKA_REQUEST_TIMEOUT".to_owned(),
            });
        }
    }

    /// The three SASL variables are each required with a SASL protocol and must be unset with any
    /// other; skipped when the protocol is invalid. A set but empty variable counts as set.
    fn sasl_rule(&mut self, protocol: Option<SecurityProtocol>, args: &KafkaArgs) {
        let Some(protocol) = protocol else {
            return;
        };
        let sasl = protocol.is_sasl();
        let variables = [
            ("KAFKA_SASL_MECHANISM", args.sasl_mechanism.is_some()),
            ("KAFKA_SASL_USERNAME", args.sasl_username.is_some()),
            ("KAFKA_SASL_PASSWORD", args.sasl_password.is_some()),
        ];
        for (variable, set) in variables {
            let reason = match (sasl, set) {
                (true, false) => SASL_REQUIRED,
                (false, true) => SASL_FORBIDDEN,
                (true, true) | (false, false) => continue,
            };
            self.problems.push(ConfigProblem {
                variable,
                reason: reason.to_owned(),
            });
        }
    }

    fn at_least(
        &mut self,
        variable: &'static str,
        value: Option<Duration>,
        other_name: &str,
        other: Option<Duration>,
    ) {
        if let (Some(value), Some(other)) = (value, other)
            && value < other
        {
            self.problems.push(ConfigProblem {
                variable,
                reason: format!("must not be shorter than {other_name}"),
            });
        }
    }

    fn longer_than(
        &mut self,
        variable: &'static str,
        value: Option<Duration>,
        other_name: &str,
        other: Option<Duration>,
    ) {
        if let (Some(value), Some(other)) = (value, other)
            && value <= other
        {
            self.problems.push(ConfigProblem {
                variable,
                reason: format!("must be longer than {other_name}"),
            });
        }
    }
}

fn server(collector: &mut Collector, args: &ServeArgs) -> Option<Server> {
    let app_env = collector.collect("APP_ENV", app_env(args.app_env.as_deref()));
    let host = collector.collect("APP_HOST", host(args.app_host.as_deref()));
    let port = collector.collect("APP_PORT", port(args.app_port.as_deref()));
    let log_level = collector.collect("LOG_LEVEL", log_level(args.log_level.as_deref()));
    Some(Server {
        app_env: app_env?,
        host: host?,
        port: port?,
        log_level: log_level?,
    })
}

fn sqs(collector: &mut Collector, args: &ServeArgs) -> Option<SqsSettings> {
    let region = collector.collect("AWS_REGION", required(args.aws_region.as_deref()));
    let endpoint = collector.collect(
        "AWS_ENDPOINT_URL_SQS",
        endpoint(args.aws_endpoint_url_sqs.as_deref()),
    );
    collector.credentials(
        args.aws_access_key_id.as_deref(),
        args.aws_secret_access_key.as_deref(),
    );
    let connect = collector.collect(
        "SQS_CONNECT_TIMEOUT",
        duration(
            args.sqs_connect_timeout.as_deref(),
            Duration::from_millis(500),
        ),
    );
    let operation = collector.collect(
        "SQS_OPERATION_TIMEOUT",
        duration(
            args.sqs_operation_timeout.as_deref(),
            Duration::from_secs(2),
        ),
    );
    let attempts = collector.collect(
        "SQS_MAX_ATTEMPTS",
        bounded(args.sqs_max_attempts.as_deref(), 1, 1..=5),
    );
    Some(SqsSettings {
        region: region?,
        endpoint_url: endpoint?,
        connect_timeout: connect?,
        operation_timeout: operation?,
        max_attempts: attempts?,
    })
}

fn limits(collector: &mut Collector, args: &ServeArgs) -> Option<Limits> {
    let probe = collector.collect(
        "HEALTH_PROBE_TIMEOUT",
        duration(args.health_probe_timeout.as_deref(), Duration::from_secs(3)),
    );
    let request = collector.collect(
        "HTTP_REQUEST_TIMEOUT",
        duration(
            args.http_request_timeout.as_deref(),
            Duration::from_secs(10),
        ),
    );
    let body = collector.collect(
        "HTTP_MAX_BODY_BYTES",
        bounded(
            args.http_max_body_bytes.as_deref(),
            65_536,
            1_024..=10_485_760,
        ),
    );
    let grace = collector.collect(
        "SHUTDOWN_GRACE_PERIOD",
        duration(
            args.shutdown_grace_period.as_deref(),
            Duration::from_secs(10),
        ),
    );
    Some(Limits {
        health_probe_timeout: probe?,
        request_timeout: request?,
        max_body_bytes: body?,
        shutdown_grace_period: grace?,
    })
}

const SASL_REQUIRED: &str =
    "is required when KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl";
const SASL_FORBIDDEN: &str =
    "must not be set unless KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl";

/// The value of `KAFKA_SECURITY_PROTOCOL`, before the credentials are attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecurityProtocol {
    Plaintext,
    Ssl,
    SaslPlaintext,
    SaslSsl,
}

impl SecurityProtocol {
    const fn is_sasl(self) -> bool {
        matches!(self, Self::SaslPlaintext | Self::SaslSsl)
    }
}

/// The Kafka variables, each parsed on its own; [`kafka_settings`] applies the rules between them.
struct KafkaFields {
    bootstrap_servers: Option<String>,
    topic: Option<String>,
    protocol: Option<SecurityProtocol>,
    mechanism: Option<SaslMechanism>,
    username: Option<String>,
    password: Option<String>,
    client_id: Option<String>,
    request_timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    fetch_wait: Option<Duration>,
}

/// Parses each Kafka variable on its own and records its problem.
///
/// The design names one group function `kafka_settings`; it is split in two because the rules
/// between variables must be reported after every per-variable problem, including those of
/// `CORS_ALLOWED_ORIGINS`: `from_args` calls this function first, then `allowed_origins`, and only
/// then [`kafka_settings`], which applies those rules.
fn kafka_fields(collector: &mut Collector, args: &KafkaArgs) -> KafkaFields {
    KafkaFields {
        bootstrap_servers: collector.collect(
            "KAFKA_BOOTSTRAP_SERVERS",
            bootstrap_servers(args.bootstrap_servers.as_deref()),
        ),
        topic: collector.collect("KAFKA_TOPIC", topic(args.topic.as_deref())),
        protocol: collector.collect(
            "KAFKA_SECURITY_PROTOCOL",
            security_protocol(args.security_protocol.as_deref()),
        ),
        mechanism: collector
            .collect(
                "KAFKA_SASL_MECHANISM",
                sasl_mechanism(args.sasl_mechanism.as_deref()),
            )
            .flatten(),
        username: collector
            .collect(
                "KAFKA_SASL_USERNAME",
                not_empty(args.sasl_username.as_deref()),
            )
            .flatten(),
        password: collector
            .collect(
                "KAFKA_SASL_PASSWORD",
                not_empty(args.sasl_password.as_deref()),
            )
            .flatten(),
        client_id: collector.collect("KAFKA_CLIENT_ID", client_id(args.client_id.as_deref())),
        request_timeout: collector.collect(
            "KAFKA_REQUEST_TIMEOUT",
            ranged_duration(args.request_timeout.as_deref(), &KAFKA_REQUEST_TIMEOUT),
        ),
        connect_timeout: collector.collect(
            "KAFKA_CONNECT_TIMEOUT",
            ranged_duration(args.connect_timeout.as_deref(), &KAFKA_CONNECT_TIMEOUT),
        ),
        fetch_wait: collector.collect(
            "KAFKA_FETCH_MAX_WAIT",
            ranged_duration(args.fetch_max_wait.as_deref(), &KAFKA_FETCH_MAX_WAIT),
        ),
    }
}

/// Applies the two rules between Kafka variables and builds the settings when every part is valid.
fn kafka_settings(
    collector: &mut Collector,
    args: &KafkaArgs,
    fields: KafkaFields,
) -> Option<KafkaSettings> {
    collector.connect_within_request(fields.connect_timeout, fields.request_timeout);
    collector.sasl_rule(fields.protocol, args);
    let security = security(
        fields.protocol?,
        fields.mechanism,
        fields.username,
        fields.password,
    )?;
    Some(KafkaSettings {
        bootstrap_servers: fields.bootstrap_servers?,
        topic: fields.topic?,
        client_id: fields.client_id?,
        security,
        request_timeout: fields.request_timeout?,
        connect_timeout: fields.connect_timeout?,
        fetch_wait: fields.fetch_wait?,
    })
}

/// The security of the protocol; a SASL protocol needs all three credentials.
fn security(
    protocol: SecurityProtocol,
    mechanism: Option<SaslMechanism>,
    username: Option<String>,
    password: Option<String>,
) -> Option<KafkaSecurity> {
    let credentials = || {
        Some(SaslCredentials {
            mechanism: mechanism?,
            username: username?,
            password: password?,
        })
    };
    match protocol {
        SecurityProtocol::Plaintext => Some(KafkaSecurity::Plaintext),
        SecurityProtocol::Ssl => Some(KafkaSecurity::Ssl),
        SecurityProtocol::SaslPlaintext => credentials().map(KafkaSecurity::SaslPlaintext),
        SecurityProtocol::SaslSsl => credentials().map(KafkaSecurity::SaslSsl),
    }
}

fn stream_settings(collector: &mut Collector, args: &ServeArgs) -> Option<StreamConfig> {
    let retry_delay = collector.collect(
        "KAFKA_RETRY_DELAY",
        ranged_duration(args.kafka.retry_delay.as_deref(), &KAFKA_RETRY_DELAY),
    );
    let catalog = collector.collect("EVENT_TYPES", event_catalog(args.event_types.as_deref()));
    let events = event_limits(collector, &args.streams);
    let subscribers = subscriber_limits(collector, &args.streams);
    let (events, subscribers) = (events?, subscribers?);
    Some(StreamConfig {
        catalog: catalog?,
        max_data_bytes: events.max_data_bytes,
        window_max_events: events.window_max_events,
        window_max_bytes: events.window_max_bytes,
        start_events: events.start_events,
        replay_max_events: events.replay_max_events,
        replay_permits: events.replay_permits,
        retry_delay: retry_delay?,
        stale_after: subscribers.stale_after,
        buffer: subscribers.buffer,
        max_subscribers: subscribers.max_subscribers,
        keep_alive: subscribers.keep_alive,
    })
}

struct EventLimits {
    max_data_bytes: usize,
    window_max_events: usize,
    window_max_bytes: usize,
    start_events: u64,
    replay_max_events: u64,
    replay_permits: usize,
}

fn event_limits(collector: &mut Collector, args: &StreamArgs) -> Option<EventLimits> {
    let max_data_bytes = collector.collect(
        "EVENT_MAX_DATA_BYTES",
        bounded(args.max_data_bytes.as_deref(), 65_536, 1..=1_048_576),
    );
    let window_max_events = collector.collect(
        "EVENT_WINDOW_MAX_EVENTS",
        bounded(args.window_max_events.as_deref(), 100_000, 1..=10_000_000),
    );
    let window_max_bytes = collector.collect(
        "EVENT_WINDOW_MAX_BYTES",
        bounded(
            args.window_max_bytes.as_deref(),
            67_108_864,
            2_097_152..=17_179_869_184,
        ),
    );
    let start_events = collector.collect(
        "EVENT_WINDOW_START_EVENTS",
        bounded(args.window_start_events.as_deref(), 100_000, 0..=10_000_000),
    );
    let replay_max_events = collector.collect(
        "EVENT_REPLAY_MAX_EVENTS",
        bounded(
            args.replay_max_events.as_deref(),
            1_000_000,
            1..=100_000_000,
        ),
    );
    let replay_permits = collector.collect(
        "EVENT_REPLAY_PERMITS",
        bounded(args.replay_permits.as_deref(), 4, 1..=64),
    );
    Some(EventLimits {
        max_data_bytes: max_data_bytes?,
        window_max_events: window_max_events?,
        window_max_bytes: window_max_bytes?,
        start_events: start_events?,
        replay_max_events: replay_max_events?,
        replay_permits: replay_permits?,
    })
}

struct SubscriberLimits {
    stale_after: Duration,
    buffer: usize,
    max_subscribers: usize,
    keep_alive: Duration,
}

fn subscriber_limits(collector: &mut Collector, args: &StreamArgs) -> Option<SubscriberLimits> {
    let stale_after = collector.collect(
        "EVENT_TAIL_STALE_AFTER",
        ranged_duration(args.tail_stale_after.as_deref(), &EVENT_TAIL_STALE_AFTER),
    );
    let buffer = collector.collect(
        "SUBSCRIBER_BUFFER",
        bounded(args.subscriber_buffer.as_deref(), 256, 1..=65_536),
    );
    let max_subscribers = collector.collect(
        "MAX_SUBSCRIBERS",
        bounded(args.max_subscribers.as_deref(), 100_000, 1..=1_000_000),
    );
    let keep_alive = collector.collect(
        "STREAM_KEEP_ALIVE",
        ranged_duration(args.keep_alive.as_deref(), &STREAM_KEEP_ALIVE),
    );
    Some(SubscriberLimits {
        stale_after: stale_after?,
        buffer: buffer?,
        max_subscribers: max_subscribers?,
        keep_alive: keep_alive?,
    })
}

fn allowed_origins(collector: &mut Collector, args: &ServeArgs) -> Option<AllowedOrigins> {
    collector.collect(
        "CORS_ALLOWED_ORIGINS",
        origins(args.cors_allowed_origins.as_deref()),
    )
}

/// The default and the inclusive range of a duration variable, with the range as the reason
/// names it.
struct DurationRule {
    default: Duration,
    min: Duration,
    max: Duration,
    range: &'static str,
}

const KAFKA_REQUEST_TIMEOUT: DurationRule = DurationRule {
    default: Duration::from_secs(3),
    min: Duration::from_secs(1),
    max: Duration::from_secs(30),
    range: "1s to 30s",
};
const KAFKA_CONNECT_TIMEOUT: DurationRule = DurationRule {
    default: Duration::from_secs(2),
    min: Duration::from_millis(100),
    max: Duration::from_secs(30),
    range: "100ms to 30s",
};
const KAFKA_FETCH_MAX_WAIT: DurationRule = DurationRule {
    default: Duration::from_millis(500),
    min: Duration::from_millis(50),
    max: Duration::from_secs(5),
    range: "50ms to 5s",
};
const KAFKA_RETRY_DELAY: DurationRule = DurationRule {
    default: Duration::from_secs(1),
    min: Duration::from_millis(100),
    max: Duration::from_secs(60),
    range: "100ms to 1m",
};
const EVENT_TAIL_STALE_AFTER: DurationRule = DurationRule {
    default: Duration::from_secs(10),
    min: Duration::from_secs(1),
    max: Duration::from_secs(300),
    range: "1s to 5m",
};
const STREAM_KEEP_ALIVE: DurationRule = DurationRule {
    default: Duration::from_secs(15),
    min: Duration::from_secs(1),
    max: Duration::from_secs(300),
    range: "1s to 5m",
};

/// A duration inside the range of `rule`; a value that is not a duration fails with the same
/// reason as one outside the range.
fn ranged_duration(raw: Option<&str>, rule: &DurationRule) -> Parsed<Duration> {
    let reason = || format!("must be a duration from {}", rule.range);
    let Some(value) = raw else {
        return Ok(rule.default);
    };
    humantime::parse_duration(value)
        .ok()
        .filter(|duration| (rule.min..=rule.max).contains(duration))
        .ok_or_else(reason)
}

/// The most entries of `KAFKA_BOOTSTRAP_SERVERS`.
const MAX_BOOTSTRAP_SERVERS: usize = 16;

/// `KAFKA_BOOTSTRAP_SERVERS`: 1 to 16 `host:port` entries separated by commas.
fn bootstrap_servers(raw: Option<&str>) -> Parsed<String> {
    let value = raw.unwrap_or("localhost:9092");
    let entries: Vec<&str> = value.split(',').collect();
    if entries.len() <= MAX_BOOTSTRAP_SERVERS && entries.iter().all(|entry| is_host_port(entry)) {
        Ok(entries.join(","))
    } else {
        Err("must be a comma-separated list of host:port addresses".to_owned())
    }
}

/// `host:port`: a DNS name, an IPv4 address or a bracketed IPv6 address, then a port from 1 to
/// 65 535.
fn is_host_port(entry: &str) -> bool {
    let Some((host, port)) = entry.rsplit_once(':') else {
        return false;
    };
    let host_ok = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .map_or_else(
            || is_dns_name(host),
            |inner| inner.parse::<Ipv6Addr>().is_ok(),
        );
    host_ok
        && port.bytes().all(|byte| byte.is_ascii_digit())
        && port.parse::<u16>().is_ok_and(|port| port != 0)
}

/// A DNS name (an IPv4 address is one too): 1 to 253 characters, labels of 1 to 63 ASCII letters,
/// digits and hyphens that neither start nor end with a hyphen.
fn is_dns_name(host: &str) -> bool {
    host.len() <= 253
        && host.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

/// The longest Kafka topic name.
const MAX_TOPIC_LENGTH: usize = 249;

/// `KAFKA_TOPIC`: 1 to 249 characters of `A-Za-z0-9._-`, neither `.` nor `..`.
fn topic(raw: Option<&str>) -> Parsed<String> {
    let value = raw.unwrap_or("notifications");
    let valid = (1..=MAX_TOPIC_LENGTH).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && value != "."
        && value != "..";
    if valid {
        Ok(value.to_owned())
    } else {
        Err("must be a Kafka topic name".to_owned())
    }
}

fn security_protocol(raw: Option<&str>) -> Parsed<SecurityProtocol> {
    match raw {
        None | Some("plaintext") => Ok(SecurityProtocol::Plaintext),
        Some("ssl") => Ok(SecurityProtocol::Ssl),
        Some("sasl_plaintext") => Ok(SecurityProtocol::SaslPlaintext),
        Some("sasl_ssl") => Ok(SecurityProtocol::SaslSsl),
        Some(_) => Err("must be one of plaintext, ssl, sasl_plaintext, sasl_ssl".to_owned()),
    }
}

fn sasl_mechanism(raw: Option<&str>) -> Parsed<Option<SaslMechanism>> {
    match raw {
        None => Ok(None),
        Some("PLAIN") => Ok(Some(SaslMechanism::Plain)),
        Some("SCRAM-SHA-256") => Ok(Some(SaslMechanism::ScramSha256)),
        Some("SCRAM-SHA-512") => Ok(Some(SaslMechanism::ScramSha512)),
        Some(_) => Err("must be one of PLAIN, SCRAM-SHA-256, SCRAM-SHA-512".to_owned()),
    }
}

/// An optional text that, when set, is not empty.
fn not_empty(raw: Option<&str>) -> Parsed<Option<String>> {
    raw.map(|value| required(Some(value))).transpose()
}

/// The longest `KAFKA_CLIENT_ID`.
const MAX_CLIENT_ID_LENGTH: usize = 128;

/// `KAFKA_CLIENT_ID`: 1 to 128 characters.
fn client_id(raw: Option<&str>) -> Parsed<String> {
    let value = raw.unwrap_or("notification-service");
    if (1..=MAX_CLIENT_ID_LENGTH).contains(&value.chars().count()) {
        Ok(value.to_owned())
    } else {
        Err("must be 1 to 128 characters".to_owned())
    }
}

/// `CORS_ALLOWED_ORIGINS`: unset is no origin, `*` is every origin, anything else is a list of
/// origins as a browser sends them.
fn origins(raw: Option<&str>) -> Parsed<AllowedOrigins> {
    match raw {
        None => Ok(AllowedOrigins::NoOrigin),
        Some("*") => Ok(AllowedOrigins::Any),
        Some(value) => value
            .split(',')
            .map(origin)
            .collect::<Option<Vec<_>>>()
            .map(AllowedOrigins::List)
            .ok_or_else(|| {
                "must be * or a comma-separated list of origins such as https://app.example.com"
                    .to_owned()
            }),
    }
}

/// One origin as a browser sends it: `http` or `https`, `://`, a lower-case host and an optional
/// port, and nothing else (no user information, path, query or fragment).
fn origin(entry: &str) -> Option<HeaderValue> {
    let authority = entry
        .strip_prefix("https://")
        .or_else(|| entry.strip_prefix("http://"))?;
    let plain = !authority
        .bytes()
        .any(|byte| byte.is_ascii_uppercase() || matches!(byte, b'/' | b'?' | b'#' | b'@'));
    let parsed = Authority::from_str(authority).ok()?;
    (plain && has_host_and_valid_port(&parsed))
        .then(|| HeaderValue::from_str(entry).ok())
        .flatten()
}

/// The largest number of distinct names of `EVENT_TYPES`.
const MAX_EVENT_TYPES: usize = 1_000;

/// `EVENT_TYPES`: unset is an empty catalog; a value is 1 to 1 000 distinct valid names separated
/// by commas, without an empty entry; a repeated name counts once.
fn event_catalog(raw: Option<&str>) -> Parsed<EventCatalog> {
    let Some(value) = raw else {
        return Ok(EventCatalog::default());
    };
    let reason = || "must be a comma-separated list of 1 to 1000 event type names".to_owned();
    let names = value
        .split(',')
        .map(|name| EventType::new(name.to_owned()))
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|_invalid| reason())?;
    if names.len() > MAX_EVENT_TYPES {
        return Err(reason());
    }
    Ok(EventCatalog::new(names))
}

fn app_env(raw: Option<&str>) -> Parsed<AppEnv> {
    match raw {
        None | Some("prod") => Ok(AppEnv::Prod),
        Some("dev") => Ok(AppEnv::Dev),
        Some("test") => Ok(AppEnv::Test),
        Some(_) => Err("must be one of dev, test, prod".to_owned()),
    }
}

fn host(raw: Option<&str>) -> Parsed<IpAddr> {
    parsed(
        raw,
        IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        "must be an IP address",
    )
}

fn port(raw: Option<&str>) -> Parsed<u16> {
    parsed(raw, 8081, "must be an integer from 0 to 65535")
}

fn log_level(raw: Option<&str>) -> Parsed<String> {
    let directives = raw.unwrap_or("info");
    EnvFilter::try_new(directives)
        .map(|_filter| directives.to_owned())
        .map_err(|_error| "must be valid tracing filter directives, for example info".to_owned())
}

fn endpoint(raw: Option<&str>) -> Parsed<Option<String>> {
    let Some(url) = raw else {
        return Ok(None);
    };
    let is_http_url = Uri::from_str(url).is_ok_and(|uri| {
        matches!(uri.scheme_str(), Some("http" | "https"))
            && uri.authority().is_some_and(has_host_and_valid_port)
    });
    if is_http_url {
        Ok(Some(url.to_owned()))
    } else {
        Err("must be an http:// or https:// URL".to_owned())
    }
}

/// `http::Uri` checks the port only when it is read, so the authority (after the optional user
/// information) must be the host alone, or the host followed by `:` and a port from 0 to 65535.
fn has_host_and_valid_port(authority: &Authority) -> bool {
    let text = authority.as_str();
    let address = text.rsplit_once('@').map_or(text, |(_, after)| after);
    let host = authority.host();
    !host.is_empty() && address.strip_prefix(host).is_some_and(is_empty_or_port)
}

/// The text after the host: nothing, or `:` followed by ASCII digits only (no sign) that fit a
/// `u16`.
fn is_empty_or_port(rest: &str) -> bool {
    rest.is_empty()
        || rest.strip_prefix(':').is_some_and(|digits| {
            digits.bytes().all(|byte| byte.is_ascii_digit()) && digits.parse::<u16>().is_ok()
        })
}

fn required(raw: Option<&str>) -> Parsed<String> {
    match raw {
        None => Err("is required".to_owned()),
        Some(value) if value.trim().is_empty() => Err("must not be empty".to_owned()),
        Some(value) => Ok(value.to_owned()),
    }
}

fn duration(raw: Option<&str>, default: Duration) -> Parsed<Duration> {
    raw.map_or(Ok(default), |value| {
        humantime::parse_duration(value)
            .map_err(|_error| "must be a duration such as 500ms, 2s or 1m".to_owned())
    })
}

fn parsed<T: FromStr>(raw: Option<&str>, default: T, reason: &str) -> Parsed<T> {
    raw.map_or(Ok(default), |value| {
        value.parse().map_err(|_error| reason.to_owned())
    })
}

fn bounded<T>(raw: Option<&str>, default: T, bounds: RangeInclusive<T>) -> Parsed<T>
where
    T: FromStr + PartialOrd + Display,
{
    let reason = format!(
        "must be an integer from {} to {}",
        bounds.start(),
        bounds.end()
    );
    let value = parsed(raw, default, &reason)?;
    if bounds.contains(&value) {
        Ok(value)
    } else {
        Err(reason)
    }
}

#[cfg(test)]
mod tests;
