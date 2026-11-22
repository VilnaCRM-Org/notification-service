//! `service serve`: binds the listener, runs the tail loop, serves until SIGTERM, SIGINT or a dead
//! loop and shuts down in the order of FR-27 (AD-15).
//!
//! Every line it writes is one JSON log entry on standard output (NFR31).

use std::net::SocketAddr;
#[cfg(target_os = "linux")]
use std::os::fd::AsFd;
use std::process::ExitCode;
use std::time::Duration;

use axum::serve::{Listener, ListenerExt as _};
use shared_domain::BoxError;
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::runtime::Runtime;
use tokio::signal::unix::{Signal, SignalKind, signal};
use tokio::sync::watch;
use tokio::task::{JoinError, JoinHandle};

use crate::background::Running;
use crate::bootstrap::{self, Adapters, Application};
use crate::config::{AppEnv, Config, ConfigErrors, ConfigProblem, StreamConfig};
use crate::telemetry;

/// Time the runtime gets to drop what is left after a forced shutdown.
const FORCED_EXIT_LIMIT: Duration = Duration::from_secs(1);

/// Level filter of the report of an invalid configuration: `LOG_LEVEL` is not known then.
const REPORT_LOG_LEVEL: &str = "info";

/// Length of the queue of connections the kernel completed and the server has not accepted yet.
/// A burst of connects beyond it has its SYNs dropped and retried a second later; the kernel caps
/// it at `net.core.somaxconn`.
const LISTEN_BACKLOG: u32 = 4096;

/// The most threads the runtime starts for blocking work (name resolution and file reads only).
/// It stays under the process limit of a container (`pids`), which the default of 512 exceeds.
const MAX_BLOCKING_THREADS: usize = 64;

/// The name of every thread of the serving runtime.
const THREAD_NAME: &str = "service-worker";

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

/// Raises the soft limit of open files to the hard limit and returns the soft limit before and
/// after: every connection holds a descriptor, and a default soft limit of 1024 refuses
/// connections with `EMFILE` long before `MAX_SUBSCRIBERS` is reached.
fn raise_file_limit() -> std::io::Result<(u64, u64)> {
    let (before, _hard) = rlimit::Resource::NOFILE.get()?;
    let after = rlimit::increase_nofile_limit(u64::MAX)?;
    Ok((before, after))
}

fn log_file_limit_raised(before: u64, after: u64) {
    tracing::info!(before, after, "open file limit");
}

fn log_file_limit_kept(error: &std::io::Error) {
    tracing::warn!(error = %error, "open file limit not raised");
}

/// Writes the outcome of [`raise_file_limit`]; a limit that cannot be raised is served as it is.
fn log_file_limit(outcome: &std::io::Result<(u64, u64)>) {
    match outcome {
        Ok((before, after)) => log_file_limit_raised(*before, *after),
        Err(error) => log_file_limit_kept(error),
    }
}

/// Completes when the stop flag is set; the graceful shutdown of the server waits for it.
async fn stopped(mut stopping: watch::Receiver<bool>) {
    drop(stopping.wait_for(|stopping| *stopping).await);
}

/// A TCP socket of the address family of `address`.
fn socket_for(address: SocketAddr) -> std::io::Result<TcpSocket> {
    if address.is_ipv4() {
        TcpSocket::new_v4()
    } else {
        TcpSocket::new_v6()
    }
}

/// The listening socket of `address`, with its address reusable at once after a restart, as
/// `TcpListener::bind` makes it, and a queue of [`LISTEN_BACKLOG`] connections.
fn listen_on(address: SocketAddr) -> std::io::Result<TcpListener> {
    let socket = socket_for(address)?;
    socket.set_reuseaddr(true)?;
    socket.bind(address)?;
    socket.listen(LISTEN_BACKLOG)
}

/// Bounds a stalled peer at the socket: the kernel ends a connection whose peer leaves sent data
/// unacknowledged, or announces a zero window, for longer than `limit` (FR-24). The result is
/// dropped: a socket that refuses the option is served without it.
#[cfg(target_os = "linux")]
fn bound_stalled_peer<S: AsFd>(socket: &S, limit: Duration) {
    drop(socket2::SockRef::from(socket).set_tcp_user_timeout(Some(limit)));
}

/// The transport bound exists on Linux only.
#[cfg(not(target_os = "linux"))]
fn bound_stalled_peer<S>(_socket: &S, _limit: Duration) {}

/// Prepares an accepted connection: Nagle's algorithm is off, so a small frame written after
/// another leaves at once instead of waiting for the acknowledgement of the first, and a stalled
/// peer is bounded at `limit` by [`bound_stalled_peer`]. A socket that refuses an option is served
/// without it.
fn prepare_connection(stream: &TcpStream, limit: Duration) {
    drop(stream.set_nodelay(true));
    bound_stalled_peer(stream, limit);
}

/// `listener` with every connection it accepts prepared by [`prepare_connection`], its stalled
/// peer bounded at `STREAM_KEEP_ALIVE` (FR-24).
fn bounded_listener(
    listener: TcpListener,
    streams: &StreamConfig,
) -> impl Listener<Io = TcpStream, Addr = SocketAddr> {
    let keep_alive = streams.keep_alive;
    listener.tap_io(move |stream| prepare_connection(stream, keep_alive))
}

/// The shutdown up to the end of the HTTP drain: the background work, then the server.
async fn drain(
    running: Running,
    server: JoinHandle<std::io::Result<()>>,
) -> Result<std::io::Result<()>, JoinError> {
    running.shutdown().await;
    server.await
}

/// Wires the application, binds and serves until a signal. Nothing here contacts a dependency;
/// the tail loop connects on its first round.
async fn serve(config: &Config) -> Result<Shutdown, BoxError> {
    let signals = Signals::install()?;
    let adapters = Adapters::connect(config).await;
    let application = bootstrap::application(adapters, config)?;
    let listener = listen_on(SocketAddr::new(config.host, config.port))?;
    run_until(application, listener, config, signals.received()).await
}

/// Serves `application` on `listener` with its tail loop until `signal` arrives or the loop dies,
/// then refuses new connections, stops the loop, closes the hub, waits for the streams and drains
/// HTTP, all within `SHUTDOWN_GRACE_PERIOD`.
async fn run_until<S: Future<Output = &'static str> + Send>(
    application: Application,
    listener: TcpListener,
    config: &Config,
    signal: S,
) -> Result<Shutdown, BoxError> {
    log_listening(listener.local_addr()?, mode(config.app_env));
    let listener = bounded_listener(listener, &config.streams);
    let (stop, stopping) = watch::channel(false);
    let server =
        axum::serve(listener, application.router).with_graceful_shutdown(stopped(stopping));
    let server = tokio::spawn(async move { server.await });
    let mut running = application.background.spawn();
    let cause = running.stopped(signal).await;
    stop.send_replace(true);
    match tokio::time::timeout(config.shutdown_grace_period, drain(running, server)).await {
        Ok(joined) => {
            joined??;
            cause.result()?;
            log_stopped();
            Ok(Shutdown::Drained)
        }
        Err(_elapsed) => {
            log_forced(config.shutdown_grace_period);
            Ok(Shutdown::Forced)
        }
    }
}

/// The runtime of the serve command: I/O, timers and one worker thread per CPU the process may
/// use (the cgroup quota counts), at most [`MAX_BLOCKING_THREADS`] blocking threads, every thread
/// named [`THREAD_NAME`].
fn serving_runtime() -> std::io::Result<Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(MAX_BLOCKING_THREADS)
        .thread_name(THREAD_NAME)
        .build()
}

/// Runs `serving` to its end on `runtime`, then ends the runtime: what is still open, such as a
/// WebSocket whose client takes no data, is dropped within [`FORCED_EXIT_LIMIT`], so that client is
/// cut without a close frame (FR-27).
fn serve_on<F: Future>(runtime: Runtime, serving: F) -> F::Output {
    let outcome = runtime.block_on(serving);
    runtime.shutdown_timeout(FORCED_EXIT_LIMIT);
    outcome
}

fn start(config: &Config) -> Result<Shutdown, BoxError> {
    telemetry::init(&config.log_level, std::io::stdout)?;
    log_file_limit(&raise_file_limit());
    serve_on(serving_runtime()?, serve(config))
}

fn failed(error: &BoxError) -> ExitCode {
    log_failed(error);
    ExitCode::FAILURE
}

/// Serves until a signal arrives or the tail loop dies, and returns the exit status.
///
/// The status is 0 when everything ended within the grace period after a signal, and 1 when the
/// grace period ended first, the loop died or the service failed (an entry `serve failed` says
/// why).
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

#[cfg(test)]
mod tests;
