//! `service serve`: binds the listener, serves until SIGTERM or SIGINT and drains (AD-15).
//!
//! Every line it writes is one JSON log entry on standard output (NFR31).

use std::net::SocketAddr;
use std::process::ExitCode;
use std::time::Duration;

use shared_domain::BoxError;
use tokio::net::TcpListener;
use tokio::signal::unix::{Signal, SignalKind, signal};
use tokio::sync::watch;

use crate::bootstrap::{self, Adapters};
use crate::config::{AppEnv, Config, ConfigErrors, ConfigProblem};
use crate::telemetry;

/// Time the runtime gets to drop what is left after a forced shutdown.
const FORCED_EXIT_LIMIT: Duration = Duration::from_secs(1);

/// Level filter of the report of an invalid configuration: `LOG_LEVEL` is not known then.
const REPORT_LOG_LEVEL: &str = "info";

/// How the server stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shutdown {
    /// Every connection finished within the grace period.
    Drained,
    /// The grace period ended with a connection still open.
    Forced,
}

/// The handlers of SIGTERM and SIGINT. They are installed before the `listening` entry is
/// written, so a signal sent as soon as that entry is read is never lost.
struct Signals {
    terminate: Signal,
    interrupt: Signal,
}

impl Signals {
    fn install() -> std::io::Result<Self> {
        Ok(Self {
            terminate: signal(SignalKind::terminate())?,
            interrupt: signal(SignalKind::interrupt())?,
        })
    }

    /// Waits for the first signal and names it.
    async fn received(mut self) -> &'static str {
        tokio::select! {
            _ = self.terminate.recv() => "SIGTERM",
            _ = self.interrupt.recv() => "SIGINT",
        }
    }
}

/// The mode of FR1 as it is written in `APP_ENV`.
const fn mode(app_env: AppEnv) -> &'static str {
    match app_env {
        AppEnv::Dev => "dev",
        AppEnv::Test => "test",
        AppEnv::Prod => "prod",
    }
}

fn log_listening(addr: SocketAddr, env: &str) {
    tracing::info!(%addr, env, version = env!("CARGO_PKG_VERSION"), "listening");
}

fn log_signal(name: &str) {
    tracing::info!(signal = name, "shutdown signal received");
}

fn log_forced(grace_period: Duration) {
    tracing::warn!(?grace_period, "grace period elapsed, forcing shutdown");
}

fn log_stopped() {
    tracing::info!("stopped");
}

fn log_invalid(problem: &ConfigProblem) {
    tracing::error!(variable = problem.variable, reason = %problem.reason, "invalid configuration");
}

fn log_failed(error: &BoxError) {
    tracing::error!(error = %error, "serve failed");
}

/// Completes when the stop flag is set; the graceful shutdown of the server waits for it.
async fn stopped(mut stopping: watch::Receiver<bool>) {
    drop(stopping.wait_for(|stopping| *stopping).await);
}

#[expect(
    clippy::disallowed_methods,
    reason = "the serve command owns the listening socket"
)]
async fn bind(config: &Config) -> std::io::Result<TcpListener> {
    TcpListener::bind((config.host, config.port)).await
}

/// Wires the application, binds, serves and drains. Nothing here contacts a dependency.
async fn serve(config: &Config) -> Result<Shutdown, BoxError> {
    let signals = Signals::install()?;
    let adapters = Adapters::connect(config).await?;
    let router = bootstrap::router(adapters, config)?;
    let listener = bind(config).await?;
    let addr = listener.local_addr()?;
    log_listening(addr, mode(config.app_env));
    let (stop, stopping) = watch::channel(false);
    let server = axum::serve(listener, router).with_graceful_shutdown(stopped(stopping));
    let server = tokio::spawn(async move { server.await });
    log_signal(signals.received().await);
    stop.send_replace(true);
    match tokio::time::timeout(config.shutdown_grace_period, server).await {
        Ok(joined) => {
            joined??;
            log_stopped();
            Ok(Shutdown::Drained)
        }
        Err(_elapsed) => {
            log_forced(config.shutdown_grace_period);
            Ok(Shutdown::Forced)
        }
    }
}

fn start(config: &Config) -> Result<Shutdown, BoxError> {
    telemetry::init(&config.log_level, std::io::stdout)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let outcome = runtime.block_on(serve(config));
    // Connections that are still open are dropped here, within a bounded time.
    runtime.shutdown_timeout(FORCED_EXIT_LIMIT);
    outcome
}

fn failed(error: &BoxError) -> ExitCode {
    log_failed(error);
    ExitCode::FAILURE
}

/// Serves until a signal arrives: 0 when every request finished, 1 when the grace period ended
/// with a connection open or the service failed (an entry `serve failed` says why).
#[must_use]
pub fn run(config: &Config) -> ExitCode {
    match start(config) {
        Ok(Shutdown::Drained) => ExitCode::SUCCESS,
        Ok(Shutdown::Forced) => ExitCode::FAILURE,
        Err(error) => failed(&error),
    }
}

fn report(errors: &ConfigErrors) -> Result<(), BoxError> {
    telemetry::init(REPORT_LOG_LEVEL, std::io::stdout)?;
    errors.problems().iter().for_each(log_invalid);
    Ok(())
}

/// Reports an invalid configuration as one JSON entry per problem and returns 2.
#[must_use]
pub fn reject(errors: &ConfigErrors) -> ExitCode {
    drop(report(errors));
    ExitCode::from(crate::command::REJECTED)
}
