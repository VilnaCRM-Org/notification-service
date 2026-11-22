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
    THREAD_NAME, listen_on, log_file_limit, prepare_connection, raise_file_limit, serving_runtime,
    socket_for,
};

/// The limit of every awaited call; the sockets are real, so the clock is too.
const LIMIT: Duration = Duration::from_secs(10);

/// The soft limit the tests of the open file limit start from.
const LOWERED: u64 = 256;

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
