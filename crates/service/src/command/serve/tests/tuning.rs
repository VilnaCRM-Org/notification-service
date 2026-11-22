//! The process and socket tuning of the serve command: the open file limit, the listening socket,
//! the options of an accepted connection and the serving runtime.
//!
//! The tests of the open file limit change a limit of the whole process, so they rely on nextest
//! giving every test its own process.

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use socket2::{Domain, SockRef};
use test_support::log::LogRecorder;
use tokio::net::TcpStream;
use tracing::Level;

use super::{
    LISTEN_BACKLOG, THREAD_NAME, listen_on, log_file_limit, prepare_connection, raise_file_limit,
    serving_runtime, socket_for,
};

/// The limit of every awaited call; the sockets are real, so the clock is too.
const LIMIT: Duration = Duration::from_secs(10);

/// The soft limit the tests of the open file limit start from.
const LOWERED: u64 = 256;

/// The backlog `tokio::net::TcpListener::bind` gives its socket; Linux queues one connection more.
const DEFAULT_BACKLOG: usize = 1024;

/// The connects the backlog test opens at once without accepting: well beyond [`DEFAULT_BACKLOG`].
const BURST: usize = 1500;

/// The limit of the burst. A connect queued by the kernel completes at once on the loopback; one
/// whose SYN is dropped by a full queue is retried after a second and again two seconds later, so
/// it never completes within this limit while nothing is accepted.
const BURST_LIMIT: Duration = Duration::from_secs(2);

/// The cap Linux puts on any listen backlog.
const SOMAXCONN: &str = "/proc/sys/net/core/somaxconn";

/// The workspace manifest, whose release profile builds the served binary.
const WORKSPACE_MANIFEST: &str = include_str!("../../../../../../Cargo.toml");

fn loopback() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
}

/// Awaits `future` within [`LIMIT`].
async fn bounded<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(LIMIT, future)
        .await
        .expect("the call must end within the limit")
}

#[expect(
    clippy::disallowed_methods,
    reason = "the tests are clients of the listening socket"
)]
async fn dial(address: SocketAddr) -> TcpStream {
    bounded(TcpStream::connect(address)).await.unwrap()
}

#[expect(
    clippy::disallowed_methods,
    reason = "the tests are clients of the listening socket"
)]
async fn dial_unbounded(address: SocketAddr) -> io::Result<TcpStream> {
    TcpStream::connect(address).await
}

/// The connects of the backlog test: [`BURST`], capped by the kernel limit of a backlog.
fn burst_size() -> usize {
    let somaxconn: usize = std::fs::read_to_string(SOMAXCONN)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let size = BURST.min(somaxconn);
    assert!(
        size > DEFAULT_BACKLOG + 1,
        "net.core.somaxconn = {somaxconn} caps every backlog at the default one, so the backlog \
         cannot be told apart"
    );
    size
}

/// The `key = value` lines of the `[profile.release]` section of the workspace manifest, up to
/// the next section header, so another profile cannot stand in for it.
fn release_profile() -> Vec<(&'static str, &'static str)> {
    let (_before, rest) = WORKSPACE_MANIFEST
        .split_once("\n[profile.release]\n")
        .expect("the workspace manifest must have a release profile");
    rest.lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (key, value) = line.split_once('=').expect("a key = value line");
            (key.trim(), value.trim())
        })
        .collect()
}

#[test]
fn the_listen_backlog_is_4096() {
    assert_eq!(LISTEN_BACKLOG, 4096);
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn the_listening_socket_queues_a_burst_beyond_the_default_backlog_without_accepting() {
    raise_file_limit().unwrap();
    let size = burst_size();
    let listener = listen_on(loopback()).unwrap();
    let address = listener.local_addr().unwrap();

    let mut connects = tokio::task::JoinSet::new();
    for _ in 0..size {
        connects.spawn(dial_unbounded(address));
    }
    let clients = tokio::time::timeout(BURST_LIMIT, connects.join_all())
        .await
        .expect("every connect of the burst must be queued while nothing is accepted");

    assert_eq!(clients.len(), size);
    assert!(clients.iter().all(Result::is_ok), "a connect failed");
    for _ in 0..size {
        tokio::time::timeout(BURST_LIMIT, listener.accept())
            .await
            .expect("every queued connection must be accepted")
            .unwrap();
    }
}

#[test]
fn the_release_profile_optimises_fully_and_keeps_unwinding() {
    let profile = release_profile();

    for (key, expected) in [
        ("opt-level", "3"),
        ("lto", "\"thin\""),
        ("codegen-units", "1"),
        ("strip", "\"symbols\""),
        ("panic", "\"unwind\""),
    ] {
        let values: Vec<&str> = profile
            .iter()
            .filter(|(name, _value)| *name == key)
            .map(|(_name, value)| *value)
            .collect();
        assert_eq!(values, [expected], "{key} of the release profile");
    }
}

#[test]
fn raising_the_file_limit_lifts_the_soft_limit_to_the_hard_limit() {
    let (_soft, hard) = rlimit::Resource::NOFILE.get().unwrap();
    assert!(
        hard > LOWERED,
        "the hard limit {hard} leaves nothing to raise"
    );
    rlimit::Resource::NOFILE.set(LOWERED, hard).unwrap();

    let raised = raise_file_limit().unwrap();

    assert_eq!(raised, (LOWERED, hard));
    assert_eq!(rlimit::Resource::NOFILE.get().unwrap(), (hard, hard));
}

#[test]
fn raising_a_file_limit_already_at_the_hard_limit_keeps_it() {
    let (_soft, hard) = rlimit::Resource::NOFILE.get().unwrap();
    rlimit::Resource::NOFILE.set(hard, hard).unwrap();

    let raised = raise_file_limit().unwrap();

    assert_eq!(raised, (hard, hard));
}

#[test]
fn a_raised_file_limit_is_logged_with_the_limit_before_and_after() {
    let (logs, _guard) = LogRecorder::install();

    log_file_limit(&Ok((LOWERED, 1_048_576)));

    let lines = logs.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    let line = lines.first().unwrap();
    assert_eq!(line.level, Level::INFO);
    assert_eq!(line.message, "open file limit");
    assert_eq!(
        line.fields,
        [
            ("before".to_owned(), "256".to_owned()),
            ("after".to_owned(), "1048576".to_owned()),
        ]
    );
}

#[test]
fn a_file_limit_that_cannot_be_raised_is_logged_as_a_warning() {
    let (logs, _guard) = LogRecorder::install();

    log_file_limit(&Err(io::Error::other("operation not permitted")));

    let lines = logs.lines();
    assert_eq!(lines.len(), 1, "{lines:?}");
    let line = lines.first().unwrap();
    assert_eq!(line.level, Level::WARN);
    assert_eq!(line.message, "open file limit not raised");
    assert_eq!(
        line.fields,
        [("error".to_owned(), "operation not permitted".to_owned())]
    );
}

#[rstest::rstest]
#[case::v4(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), Domain::IPV4)]
#[case::v6(SocketAddr::from((Ipv6Addr::LOCALHOST, 0)), Domain::IPV6)]
fn the_socket_has_the_address_family_of_the_address(
    #[case] address: SocketAddr,
    #[case] family: Domain,
) {
    let socket = socket_for(address).unwrap();

    assert_eq!(SockRef::from(&socket).domain().unwrap(), family);
}

#[tokio::test]
async fn the_listening_socket_reuses_its_address_and_accepts_connections() {
    let listener = listen_on(loopback()).unwrap();
    let address = listener.local_addr().unwrap();

    let client = dial(address).await;
    let (_accepted, peer) = bounded(listener.accept()).await.unwrap();

    assert_eq!(address.ip(), Ipv4Addr::LOCALHOST);
    assert_ne!(address.port(), 0);
    assert_eq!(peer, client.local_addr().unwrap());
    assert!(SockRef::from(&listener).reuse_address().unwrap());
}

#[tokio::test]
async fn an_address_in_use_is_refused() {
    let taken = listen_on(loopback()).unwrap();

    let refused = listen_on(taken.local_addr().unwrap()).unwrap_err();

    assert_eq!(refused.kind(), io::ErrorKind::AddrInUse);
}

#[tokio::test]
async fn a_prepared_connection_sends_small_writes_at_once() {
    let listener = listen_on(loopback()).unwrap();
    let client = dial(listener.local_addr().unwrap()).await;
    assert!(!client.nodelay().unwrap());

    prepare_connection(&client, Duration::from_secs(15));

    assert!(client.nodelay().unwrap());
}

#[test]
fn the_serving_runtime_names_its_threads() {
    let runtime = serving_runtime().unwrap();

    let worker = runtime
        .block_on(runtime.spawn(async { std::thread::current().name().map(str::to_owned) }))
        .unwrap();
    let blocking = runtime
        .block_on(runtime.spawn_blocking(|| std::thread::current().name().map(str::to_owned)))
        .unwrap();

    assert_eq!(worker.as_deref(), Some(THREAD_NAME));
    assert_eq!(blocking.as_deref(), Some(THREAD_NAME));
    runtime.shutdown_timeout(Duration::from_secs(1));
}
