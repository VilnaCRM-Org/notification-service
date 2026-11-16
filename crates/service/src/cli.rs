//! The command line of the service binary.
//!
//! Every setting is an `Option<String>` bound to its environment variable, so clap never rejects a
//! value: [`crate::config::Config::from_args`] parses and validates all of them and names the
//! variable of every problem. `--help` names each variable and never prints a value.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Service binary of the template: serves the API and runs the maintenance commands.
#[derive(Debug, Parser)]
#[command(name = "service", version, about)]
pub struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The commands of the binary.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate the configuration, start logging, serve until SIGTERM or SIGINT, then drain.
    Serve(ServeArgs),
    /// Validate the configuration of `serve` without opening a socket or contacting a dependency.
    CheckConfig(ServeArgs),
    /// Write the REST API contract (YAML) to a file or to standard output.
    OpenapiExport(ExportArgs),
    /// Write the GraphQL schema (SDL) to a file or to standard output.
    GraphqlExport(ExportArgs),
    /// Ask a running service whether it is alive (on the loopback interface or the `APP_HOST` address).
    Healthcheck(HealthcheckArgs),
}

/// Settings of `serve` and `check-config`: every service variable.
#[derive(Debug, Clone, Default, Args)]
pub struct ServeArgs {
    /// Deployment environment: `dev`, `test` or `prod` (default `prod`).
    #[arg(long = "app-env", env = "APP_ENV", hide_env_values = true)]
    pub app_env: Option<String>,
    /// IP address to listen on (default `0.0.0.0`).
    #[arg(long = "app-host", env = "APP_HOST", hide_env_values = true)]
    pub app_host: Option<String>,
    /// TCP port to listen on, 0 to 65535; 0 selects any free port (default 8081).
    #[arg(long = "app-port", env = "APP_PORT", hide_env_values = true)]
    pub app_port: Option<String>,
    /// `tracing` filter directives of the log output (default `info`).
    #[arg(long = "log-level", env = "LOG_LEVEL", hide_env_values = true)]
    pub log_level: Option<String>,
    /// AWS region of the message broker (required).
    #[arg(long = "aws-region", env = "AWS_REGION", hide_env_values = true)]
    pub aws_region: Option<String>,
    /// `http://` or `https://` endpoint of SQS (default: the regional AWS endpoint).
    #[arg(
        long = "aws-endpoint-url-sqs",
        env = "AWS_ENDPOINT_URL_SQS",
        hide_env_values = true
    )]
    pub aws_endpoint_url_sqs: Option<String>,
    /// Access key id; only validated as a pair with the secret, the AWS SDK reads it itself.
    #[arg(long, env = "AWS_ACCESS_KEY_ID", hide_env_values = true, hide = true)]
    pub aws_access_key_id: Option<String>,
    /// Secret access key; only validated as a pair with the key id, the AWS SDK reads it itself.
    #[arg(
        long,
        env = "AWS_SECRET_ACCESS_KEY",
        hide_env_values = true,
        hide = true
    )]
    pub aws_secret_access_key: Option<String>,
    /// Timeout of establishing an SQS connection (default 500ms).
    #[arg(
        long = "sqs-connect-timeout",
        env = "SQS_CONNECT_TIMEOUT",
        hide_env_values = true
    )]
    pub sqs_connect_timeout: Option<String>,
    /// Timeout of one SQS attempt and of the whole operation (default 2s).
    #[arg(
        long = "sqs-operation-timeout",
        env = "SQS_OPERATION_TIMEOUT",
        hide_env_values = true
    )]
    pub sqs_operation_timeout: Option<String>,
    /// SQS attempts per operation, 1 to 5 (default 1).
    #[arg(
        long = "sqs-max-attempts",
        env = "SQS_MAX_ATTEMPTS",
        hide_env_values = true
    )]
    pub sqs_max_attempts: Option<String>,
    /// Timeout of one health probe (default 3s).
    #[arg(
        long = "health-probe-timeout",
        env = "HEALTH_PROBE_TIMEOUT",
        hide_env_values = true
    )]
    pub health_probe_timeout: Option<String>,
    /// Budget of one HTTP request (default 10s).
    #[arg(
        long = "http-request-timeout",
        env = "HTTP_REQUEST_TIMEOUT",
        hide_env_values = true
    )]
    pub http_request_timeout: Option<String>,
    /// Largest accepted request body in bytes, 1024 to 10485760 (default 65536).
    #[arg(
        long = "http-max-body-bytes",
        env = "HTTP_MAX_BODY_BYTES",
        hide_env_values = true
    )]
    pub http_max_body_bytes: Option<String>,
    /// How long in-flight requests may finish after a termination signal (default 10s).
    #[arg(
        long = "shutdown-grace-period",
        env = "SHUTDOWN_GRACE_PERIOD",
        hide_env_values = true
    )]
    pub shutdown_grace_period: Option<String>,
    /// Comma-separated names of the subscribable event types, 1 to 1000 distinct names (default:
    /// none).
    #[arg(long = "event-types", env = "EVENT_TYPES", hide_env_values = true)]
    pub event_types: Option<String>,
    /// Settings of the Kafka connection.
    #[command(flatten)]
    pub kafka: KafkaArgs,
    /// Limits of the event stream.
    #[command(flatten)]
    pub streams: StreamArgs,
    /// `*`, or comma-separated origins such as `https://app.example.com` whose pages may use the
    /// service (default: none).
    #[arg(
        long = "cors-allowed-origins",
        env = "CORS_ALLOWED_ORIGINS",
        hide_env_values = true
    )]
    pub cors_allowed_origins: Option<String>,
}

/// Settings of the Kafka connection of the event stream.
#[derive(Debug, Clone, Default, Args)]
pub struct KafkaArgs {
    /// Comma-separated `host:port` addresses of the brokers, 1 to 16 (default `localhost:9092`).
    #[arg(
        long = "kafka-bootstrap-servers",
        env = "KAFKA_BOOTSTRAP_SERVERS",
        hide_env_values = true
    )]
    pub bootstrap_servers: Option<String>,
    /// Topic of the events (default `notifications`).
    #[arg(long = "kafka-topic", env = "KAFKA_TOPIC", hide_env_values = true)]
    pub topic: Option<String>,
    /// `plaintext`, `ssl`, `sasl_plaintext` or `sasl_ssl` (default `plaintext`).
    #[arg(
        long = "kafka-security-protocol",
        env = "KAFKA_SECURITY_PROTOCOL",
        hide_env_values = true
    )]
    pub security_protocol: Option<String>,
    /// `PLAIN`, `SCRAM-SHA-256` or `SCRAM-SHA-512`; only with a SASL protocol.
    #[arg(
        long = "kafka-sasl-mechanism",
        env = "KAFKA_SASL_MECHANISM",
        hide_env_values = true
    )]
    pub sasl_mechanism: Option<String>,
    /// SASL user name; only with a SASL protocol.
    #[arg(
        long = "kafka-sasl-username",
        env = "KAFKA_SASL_USERNAME",
        hide_env_values = true
    )]
    pub sasl_username: Option<String>,
    /// SASL password; only with a SASL protocol, never printed.
    #[arg(
        long = "kafka-sasl-password",
        env = "KAFKA_SASL_PASSWORD",
        hide_env_values = true
    )]
    pub sasl_password: Option<String>,
    /// Client id of every Kafka connection, 1 to 128 characters (default `notification-service`).
    #[arg(
        long = "kafka-client-id",
        env = "KAFKA_CLIENT_ID",
        hide_env_values = true
    )]
    pub client_id: Option<String>,
    /// Request time-out of the client, `1s` to `30s` (default 3s).
    #[arg(
        long = "kafka-request-timeout",
        env = "KAFKA_REQUEST_TIMEOUT",
        hide_env_values = true
    )]
    pub request_timeout: Option<String>,
    /// Connect time-out of the client, `100ms` to `30s` and not longer than the request time-out
    /// (default 2s).
    #[arg(
        long = "kafka-connect-timeout",
        env = "KAFKA_CONNECT_TIMEOUT",
        hide_env_values = true
    )]
    pub connect_timeout: Option<String>,
    /// Long-poll wait of the tail, `50ms` to `5s` (default 500ms).
    #[arg(
        long = "kafka-fetch-max-wait",
        env = "KAFKA_FETCH_MAX_WAIT",
        hide_env_values = true
    )]
    pub fetch_max_wait: Option<String>,
    /// Pause after a failed round of the tail, `100ms` to `1m` (default 1s).
    #[arg(
        long = "kafka-retry-delay",
        env = "KAFKA_RETRY_DELAY",
        hide_env_values = true
    )]
    pub retry_delay: Option<String>,
}

/// Limits of the event stream.
#[derive(Debug, Clone, Default, Args)]
pub struct StreamArgs {
    /// Largest record value in bytes, 1 to 1048576 (default 65536).
    #[arg(
        long = "event-max-data-bytes",
        env = "EVENT_MAX_DATA_BYTES",
        hide_env_values = true
    )]
    pub max_data_bytes: Option<String>,
    /// Events in the memory window, 1 to 10000000 (default 100000).
    #[arg(
        long = "event-window-max-events",
        env = "EVENT_WINDOW_MAX_EVENTS",
        hide_env_values = true
    )]
    pub window_max_events: Option<String>,
    /// Bytes in the memory window, 2097152 to 17179869184 (default 67108864).
    #[arg(
        long = "event-window-max-bytes",
        env = "EVENT_WINDOW_MAX_BYTES",
        hide_env_values = true
    )]
    pub window_max_bytes: Option<String>,
    /// Offsets below the end of the topic read at start, 0 to 10000000 (default 100000).
    #[arg(
        long = "event-window-start-events",
        env = "EVENT_WINDOW_START_EVENTS",
        hide_env_values = true
    )]
    pub window_start_events: Option<String>,
    /// Offsets below the tail a replay may reach, 1 to 100000000 (default 1000000).
    #[arg(
        long = "event-replay-max-events",
        env = "EVENT_REPLAY_MAX_EVENTS",
        hide_env_values = true
    )]
    pub replay_max_events: Option<String>,
    /// Concurrent replay reads of the node, 1 to 64 (default 4).
    #[arg(
        long = "event-replay-permits",
        env = "EVENT_REPLAY_PERMITS",
        hide_env_values = true
    )]
    pub replay_permits: Option<String>,
    /// Age at which the readiness probe of the tail fails, `1s` to `5m` (default 10s).
    #[arg(
        long = "event-tail-stale-after",
        env = "EVENT_TAIL_STALE_AFTER",
        hide_env_values = true
    )]
    pub tail_stale_after: Option<String>,
    /// Queued events per subscriber, 1 to 65536 (default 256).
    #[arg(
        long = "subscriber-buffer",
        env = "SUBSCRIBER_BUFFER",
        hide_env_values = true
    )]
    pub subscriber_buffer: Option<String>,
    /// Subscribers of the node, 1 to 1000000 (default 100000).
    #[arg(
        long = "max-subscribers",
        env = "MAX_SUBSCRIBERS",
        hide_env_values = true
    )]
    pub max_subscribers: Option<String>,
    /// Keep-alive, WebSocket ping, send bound and write deadline, `1s` to `5m` (default 15s).
    #[arg(
        long = "stream-keep-alive",
        env = "STREAM_KEEP_ALIVE",
        hide_env_values = true
    )]
    pub keep_alive: Option<String>,
}

/// Arguments of `openapi-export` and `graphql-export`.
#[derive(Debug, Clone, Args)]
pub struct ExportArgs {
    /// File to write; standard output when absent.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

/// Arguments of `healthcheck`.
#[derive(Debug, Clone, Args)]
pub struct HealthcheckArgs {
    /// TCP port of the service (default 8081).
    #[arg(long = "port", env = "APP_PORT", hide_env_values = true)]
    pub port: Option<String>,
    /// IP address the service listens on; the loopback interface is probed when it is an
    /// unspecified address such as `0.0.0.0` (default `127.0.0.1`).
    #[arg(long = "host", env = "APP_HOST", hide_env_values = true)]
    pub host: Option<String>,
}

#[cfg(test)]
mod tests;
