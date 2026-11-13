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
    /// Wait for the database and apply the pending migrations, or revert the last one.
    Migrate(MigrateArgs),
    /// Validate the configuration of `serve` without opening a socket or contacting a dependency.
    CheckConfig(ServeArgs),
    /// Write the REST API contract (YAML) to a file or to standard output.
    OpenapiExport(ExportArgs),
    /// Write the GraphQL schema (SDL) to a file or to standard output.
    GraphqlExport(ExportArgs),
    /// Ask a running service whether it is alive (on the loopback interface or the `APP_HOST` address).
    Healthcheck(HealthcheckArgs),
}

/// What `service migrate` does besides applying the pending migrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum MigrateAction {
    /// Revert the most recently applied migration.
    Revert,
}

/// Settings of the PostgreSQL pool.
#[derive(Debug, Clone, Default, Args)]
pub struct DatabaseArgs {
    /// `postgres://` or `postgresql://` URL of the database (required).
    #[arg(long = "database-url", env = "DATABASE_URL", hide_env_values = true)]
    pub url: Option<String>,
    /// Upper bound of open connections, 1 to 100 (default 10).
    #[arg(
        long = "database-max-connections",
        env = "DATABASE_MAX_CONNECTIONS",
        hide_env_values = true
    )]
    pub max_connections: Option<String>,
    /// How long a caller waits for a free connection (default 2s).
    #[arg(
        long = "database-acquire-timeout",
        env = "DATABASE_ACQUIRE_TIMEOUT",
        hide_env_values = true
    )]
    pub acquire_timeout: Option<String>,
    /// `statement_timeout` of every connection (default 5s).
    #[arg(
        long = "database-statement-timeout",
        env = "DATABASE_STATEMENT_TIMEOUT",
        hide_env_values = true
    )]
    pub statement_timeout: Option<String>,
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
    /// Settings of the PostgreSQL pool.
    #[command(flatten)]
    pub database: DatabaseArgs,
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
}

/// Settings of `migrate` and `migrate revert`.
#[derive(Debug, Clone, Args)]
pub struct MigrateArgs {
    /// Settings of the PostgreSQL pool.
    #[command(flatten)]
    pub database: DatabaseArgs,
    /// How long to wait for the database, one attempt per second (default 30s).
    #[arg(
        long = "wait-timeout",
        env = "DATABASE_WAIT_TIMEOUT",
        hide_env_values = true
    )]
    pub wait_timeout: Option<String>,
    /// Revert instead of apply.
    #[command(subcommand)]
    pub action: Option<MigrateAction>,
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
