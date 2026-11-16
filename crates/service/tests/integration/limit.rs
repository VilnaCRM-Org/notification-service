//! The time limit of the tests on the broker of the stack, and the sockets they open, each awaited
//! within it.

use std::net::SocketAddr;
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};

/// The limit of every awaited call of these tests: the broker and the sockets are real, so the
/// clock is too, and a loaded machine may need seconds for a first connection. It is half of the
/// time nextest gives a test (`terminate-after` 2 periods of 30 s), so a call that hangs fails with
/// its own message.
pub(crate) const LIMIT: Duration = Duration::from_secs(30);

/// Awaits `future` within [`LIMIT`].
pub(crate) async fn bounded<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(LIMIT, future)
        .await
        .expect("the call must end within the limit")
}

/// A connection to `address`.
#[expect(
    clippy::disallowed_methods,
    reason = "integration tests are clients of the nodes and of the broker they talk to"
)]
pub(crate) async fn dial<A: ToSocketAddrs>(address: A) -> TcpStream {
    bounded(TcpStream::connect(address)).await.unwrap()
}

/// A listener on `address`, for a node served in process or a forwarder of the test. On Unix the
/// listener has `SO_REUSEADDR`, so it may take the port of a listener closed a moment ago.
#[expect(
    clippy::disallowed_methods,
    reason = "a node served in process and a forwarder of these tests listen on sockets of their own"
)]
pub(crate) async fn listen_on(address: SocketAddr) -> TcpListener {
    bounded(TcpListener::bind(address)).await.unwrap()
}
