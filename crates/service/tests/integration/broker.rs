//! The broker of the test stack, read from the environment the Make targets export: topics of one
//! test each, envelopes produced with the krafka producer, and a TCP forwarder between a served
//! binary and the broker that a test stops and starts again.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher as _;
use std::net::{Ipv4Addr, SocketAddr};
use std::ops::RangeInclusive;
use std::sync::Arc;
use std::time::Duration;

use event_stream_domain::{Cursor, TopicProbe as _};
use event_stream_infrastructure::{KafkaSecurity, KafkaSettings, KafkaTopic};
use krafka::Kafka;
use krafka::admin::{CreateTopicsOptions, DeleteTopicsOptions, DescribeClusterOptions, NewTopic};
use krafka::producer::{Acks, Record};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::{JoinHandle, JoinSet};

use crate::limit::{bounded, dial, listen_on};
use crate::support::stack_variable;

/// The event type of every envelope and of every subscription of these tests.
pub(crate) const TYPE: &str = "user.created";

/// The request time-out of the test clients: generous, so that a call that must succeed does not
/// depend on the speed of the machine.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The connect time-out of the readers that wait for a topic.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// The pause between two reads of the metadata or the watermarks of a topic.
const POLL: Duration = Duration::from_millis(100);

/// The bootstrap servers of the broker of the test stack (`KAFKA_BOOTSTRAP_SERVERS`).
pub(crate) fn bootstrap_servers() -> String {
    stack_variable("KAFKA_BOOTSTRAP_SERVERS")
}

/// A topic name no other test and no earlier run uses: the test's name and a random number of
/// the process (std's hash seed), so parallel and repeated runs never share a topic.
pub(crate) fn unique_name(test: &str) -> String {
    let random = RandomState::new().hash_one(test);
    format!("service-it.{test}.{random:016x}")
}

/// The envelope id of envelope `number`.
pub(crate) fn envelope_id(number: u64) -> String {
    format!("00000000-0000-4000-8000-{number:012x}")
}

/// A valid envelope of [`TYPE`] numbered `number`.
pub(crate) fn envelope(number: u64) -> Vec<u8> {
    let id = envelope_id(number);
    format!(
        r#"{{"id":"{id}","type":"{TYPE}","occurredAt":"2026-01-01T00:00:00Z","data":{{"n":{number}}}}}"#
    )
    .into_bytes()
}

/// The envelopes numbered `numbers`, in order.
pub(crate) fn envelopes(numbers: RangeInclusive<u64>) -> Vec<Vec<u8>> {
    numbers.map(envelope).collect()
}

/// The settings of readers of `topic` on the broker of the test stack.
fn settings(topic: &str) -> KafkaSettings {
    KafkaSettings {
        bootstrap_servers: bootstrap_servers(),
        topic: topic.to_owned(),
        client_id: "service-integration".to_owned(),
        security: KafkaSecurity::Plaintext,
        request_timeout: REQUEST_TIMEOUT,
        connect_timeout: CONNECT_TIMEOUT,
        fetch_wait: Duration::from_millis(500),
    }
}

/// A client of the broker of the test stack that administers topics.
async fn admin() -> Kafka {
    bounded(
        Kafka::builder(bootstrap_servers())
            .client_id("service-integration-admin")
            .request_timeout(REQUEST_TIMEOUT)
            .connect(),
    )
    .await
    .unwrap()
}

/// A topic of one test on the broker of the test stack, with the client that created it.
///
/// A test removes its topic with [`TestTopic::delete`]; a topic dropped before that, by a test that
/// failed or a mutant it killed, is removed by the drop, so no topic stays on the shared broker.
pub(crate) struct TestTopic {
    pub(crate) name: String,
    kafka: Kafka,
    /// Whether [`TestTopic::delete`] removed the topic.
    deleted: bool,
}

impl TestTopic {
    /// Creates a topic of `partitions` partitions named after `test`.
    pub(crate) async fn create(test: &str, partitions: i32) -> Self {
        Self::create_named(unique_name(test), partitions).await
    }

    /// Creates the topic `name` with `partitions` partitions and waits until its metadata names
    /// them.
    pub(crate) async fn create_named(name: String, partitions: i32) -> Self {
        let kafka = admin().await;
        let created = bounded(kafka.admin().create_topics(
            [NewTopic::new(name.as_str(), partitions, 1).unwrap()],
            CreateTopicsOptions::default(),
        ))
        .await
        .unwrap();
        created[&name].clone().unwrap();
        let topic = Self {
            name,
            kafka,
            deleted: false,
        };
        topic.wait_for_partitions(partitions).await;
        topic
    }

    async fn wait_for_partitions(&self, partitions: i32) {
        let probe = KafkaTopic::new(settings(&self.name)).probe();
        let expected = u32::try_from(partitions).ok();
        bounded(async {
            while bounded(probe.partition_count()).await.unwrap() != expected {
                tokio::time::sleep(POLL).await;
            }
        })
        .await;
    }

    /// Produces each value as one record of partition 0, `acks` all, in order.
    pub(crate) async fn produce(&self, values: &[Vec<u8>]) {
        let producer = bounded(
            self.kafka
                .producer()
                .acks(Acks::All)
                .linger(Duration::ZERO)
                .build(),
        )
        .await
        .unwrap();
        let mut deliveries = Vec::with_capacity(values.len());
        for value in values {
            let record = Record::new(self.name.as_str(), value.clone()).partition(0);
            deliveries.push(bounded(producer.enqueue(record)).await.unwrap());
        }
        for delivery in deliveries {
            assert_eq!(bounded(delivery).await.unwrap().partition, 0);
        }
        bounded(producer.close()).await.unwrap();
        drop(producer);
    }

    /// Produces `values` (at least one) in one transaction of id `id`, then commits it or aborts
    /// it, and waits until the end of partition 0 passes the transaction marker.
    pub(crate) async fn produce_in_transaction(&self, id: &str, values: &[Vec<u8>], commit: bool) {
        let transactional_id = format!("{}.{id}", self.name);
        let producer = bounded(self.kafka.producer().build_transactional(transactional_id))
            .await
            .unwrap();
        producer.begin().unwrap();
        let mut last = None;
        for value in values {
            let record = Record::new(self.name.as_str(), value.clone()).partition(0);
            let delivered = bounded(producer.send(record)).await.unwrap();
            assert_eq!(delivered.partition, 0);
            last = Some(u64::try_from(delivered.offset).expect("the broker confirms the offset"));
        }
        if commit {
            bounded(producer.commit()).await.unwrap();
        } else {
            bounded(producer.abort()).await.unwrap();
        }
        bounded(producer.close()).await.unwrap();
        drop(producer);
        let last = last.expect("a transaction holds at least one record");
        // The marker takes the offset after the last record; the end follows the marker.
        self.wait_for_end(Cursor::new(last.saturating_add(2))).await;
    }

    /// Waits until the end of partition 0 reaches `end`. Kafka answers a commit or an abort
    /// before it writes the transaction markers, so right after it the end can still stop at the
    /// first record of the transaction.
    async fn wait_for_end(&self, end: Cursor) {
        let probe = KafkaTopic::new(settings(&self.name)).probe();
        bounded(async {
            while bounded(probe.watermarks()).await.unwrap().end < end {
                tokio::time::sleep(POLL).await;
            }
        })
        .await;
    }

    /// The host and the port the broker advertises to the clients of the test stack's listener.
    async fn advertised(&self) -> (String, i32) {
        let cluster = bounded(
            self.kafka
                .admin()
                .describe_cluster(DescribeClusterOptions::default()),
        )
        .await
        .unwrap();
        assert_eq!(cluster.brokers.len(), 1, "{cluster:?}");
        let broker = &cluster.brokers[0];
        (broker.host.clone(), broker.port)
    }

    /// Removes the topic; the drop that follows changes nothing.
    pub(crate) async fn delete(&mut self) {
        delete_topic(&self.kafka, &self.name).await;
        self.deleted = true;
    }
}

impl Drop for TestTopic {
    /// Removes the topic that [`TestTopic::delete`] did not remove. The runtime of the test may be
    /// unwinding, so the removal runs on a thread and a runtime of its own, and its failure is
    /// ignored: a second panic would abort the test binary.
    fn drop(&mut self) {
        if self.deleted {
            return;
        }
        let name = self.name.clone();
        let _removal = std::thread::spawn(move || remove_on_own_runtime(&name)).join();
    }
}

/// Removes the topic `name` through `kafka`.
async fn delete_topic(kafka: &Kafka, name: &str) {
    bounded(
        kafka
            .admin()
            .delete_topics([name], DeleteTopicsOptions::default()),
    )
    .await
    .unwrap();
}

/// Removes the topic `name` with a new client on a new runtime of the calling thread.
fn remove_on_own_runtime(name: &str) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async { delete_topic(&admin().await, name).await });
}

/// The host of the forwarder, which every rewritten answer names for the broker.
const FORWARDER_HOST: &str = "127.0.0.1";

/// The replacement of the broker's advertised address by the forwarder's in every answer of the
/// broker.
///
/// A client reaches the broker at the address the metadata advertises, not at its bootstrap
/// address, so a forwarder that only relayed bytes would see the bootstrap connection alone. Every
/// answer that names a broker (`Metadata`, `DescribeCluster`, `FindCoordinator`) writes its host
/// as a string, a `STRING` (INT16 length) or a `COMPACT_STRING` (unsigned varint of the length
/// plus 1), followed by its port (INT32). The replacement is [`FORWARDER_HOST`] and the
/// forwarder's port whatever host the broker advertises (`localhost` on the host, `kafka` inside
/// the compose network), with the length prefix rewritten, so the frame size changes. Both prefixes
/// end in one byte that gives the length of a host shorter than 127 bytes; an entry whose prefix
/// is not such a byte fails the test. The answers that name a broker in a tagged field, whose size
/// would change too (the `NodeEndpoints` of `Fetch` and `Produce`), carry them only after a leader
/// change, which the broker of one node of the stack never has.
#[derive(Debug)]
struct Rewrite {
    /// The advertised host followed by the advertised port, as the answers write them.
    advertised: Vec<u8>,
    /// The length of the advertised host.
    host_length: usize,
    /// The forwarder's port, as the answers write a port.
    port: [u8; 4],
}

impl Rewrite {
    fn new(host: &str, port: i32, forwarder_port: u16) -> Self {
        Self {
            advertised: [host.as_bytes(), &port.to_be_bytes()].concat(),
            host_length: host.len(),
            port: i32::from(forwarder_port).to_be_bytes(),
        }
    }

    /// `frame` with every advertised address replaced by the forwarder's.
    fn apply(&self, frame: &[u8]) -> Vec<u8> {
        let mut rewritten = Vec::with_capacity(frame.len());
        let mut copied = 0_usize;
        while let Some(start) = self.find(frame, copied) {
            let (prefix, length) = self.prefix(frame, start, copied);
            rewritten.extend_from_slice(&frame[copied..prefix]);
            rewritten.push(length);
            rewritten.extend_from_slice(FORWARDER_HOST.as_bytes());
            rewritten.extend_from_slice(&self.port);
            copied = start.saturating_add(self.advertised.len());
        }
        rewritten.extend_from_slice(&frame[copied..]);
        rewritten
    }

    /// The position of the next advertised address of `frame` at or after `from`.
    fn find(&self, frame: &[u8], from: usize) -> Option<usize> {
        let found = frame[from..]
            .windows(self.advertised.len())
            .position(|window| window == self.advertised)?;
        Some(from.saturating_add(found))
    }

    /// The position of the last byte of the length prefix of the host at `start`, which lies at or
    /// after `copied`, and its replacement: the length of [`FORWARDER_HOST`] for a `STRING` (its
    /// first byte, 0, stays), that length plus 1 for a `COMPACT_STRING`.
    fn prefix(&self, frame: &[u8], start: usize, copied: usize) -> (usize, u8) {
        let length = u8::try_from(FORWARDER_HOST.len()).unwrap();
        let Some(at) = start.checked_sub(1).filter(|at| *at >= copied) else {
            unrewritable(frame, start)
        };
        let byte = usize::from(frame[at]);
        let first = at.checked_sub(1).and_then(|before| frame.get(before));
        if byte == self.host_length.saturating_add(1) {
            (at, length.saturating_add(1))
        } else if byte == self.host_length && first == Some(&0) {
            (at, length)
        } else {
            unrewritable(frame, start)
        }
    }
}

/// Fails the test on the broker entry at `start` of `frame`, whose length prefix is not one byte
/// that gives the length of the advertised host.
fn unrewritable(frame: &[u8], start: usize) -> ! {
    panic!(
        "the broker entry at byte {start} of an answer has a length prefix the forwarder cannot rewrite: {frame:?}"
    )
}

/// A TCP forwarder on a loopback port between the clients of a served binary and the broker of the
/// test stack. Stopping it closes every forwarded connection and the port, which is a broker
/// outage for the binary; starting it again listens on the same port.
#[derive(Debug)]
pub(crate) struct Forwarder {
    address: SocketAddr,
    rewrite: Arc<Rewrite>,
    running: Option<(oneshot::Sender<()>, JoinHandle<()>)>,
}

impl Forwarder {
    /// Starts a forwarder to the broker of `topic` on a free loopback port.
    pub(crate) async fn start(topic: &TestTopic) -> Self {
        let (host, port) = topic.advertised().await;
        let listener = listen_on(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await;
        let address = listener.local_addr().unwrap();
        let mut forwarder = Self {
            address,
            rewrite: Arc::new(Rewrite::new(&host, port, address.port())),
            running: None,
        };
        forwarder.serve(listener);
        forwarder
    }

    /// The address the clients of the binary connect to.
    pub(crate) const fn address(&self) -> SocketAddr {
        self.address
    }

    /// Closes the port and every forwarded connection, and returns once all of them are closed.
    pub(crate) async fn stop(&mut self) {
        let (stop, accepting) = self.running.take().expect("the forwarder runs");
        stop.send(()).unwrap();
        bounded(accepting).await.unwrap();
    }

    /// Listens on the same port again.
    pub(crate) async fn restart(&mut self) {
        assert!(self.running.is_none(), "the forwarder runs");
        let listener = listen_on(self.address).await;
        self.serve(listener);
    }

    fn serve(&mut self, listener: TcpListener) {
        let (stop, stopped) = oneshot::channel();
        let accepting = tokio::spawn(accept(
            listener,
            bootstrap_servers(),
            Arc::clone(&self.rewrite),
            stopped,
        ));
        self.running = Some((stop, accepting));
    }
}

/// Forwards every connection `listener` accepts to `upstream` until `stopped`, then closes them
/// all.
async fn accept(
    listener: TcpListener,
    upstream: String,
    rewrite: Arc<Rewrite>,
    mut stopped: oneshot::Receiver<()>,
) {
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            _stop = &mut stopped => break,
            accepted = listener.accept() => {
                let (client, _peer) = accepted.unwrap();
                connections.spawn(forward(client, upstream.clone(), Arc::clone(&rewrite)));
            }
        }
    }
    connections.shutdown().await;
}

/// Relays the requests of `client` to the broker unchanged and the answers back with the
/// advertised address rewritten, until either side closes.
async fn forward(client: TcpStream, upstream: String, rewrite: Arc<Rewrite>) {
    let broker = dial(upstream.as_str()).await;
    let (mut client_reader, mut client_writer) = client.into_split();
    let (mut broker_reader, mut broker_writer) = broker.into_split();
    tokio::select! {
        _requests = tokio::io::copy(&mut client_reader, &mut broker_writer) => {}
        _answers = relay(&mut broker_reader, &mut client_writer, &rewrite) => {}
    }
}

/// Copies the size-prefixed frames of the broker to the client, each rewritten and prefixed with
/// its new size.
async fn relay(
    broker: &mut OwnedReadHalf,
    client: &mut OwnedWriteHalf,
    rewrite: &Rewrite,
) -> std::io::Result<()> {
    let mut size = [0_u8; 4];
    while broker.read_exact(&mut size).await.is_ok() {
        let length = usize::try_from(i32::from_be_bytes(size)).unwrap();
        let mut frame = vec![0_u8; length];
        broker.read_exact(&mut frame).await?;
        let rewritten = rewrite.apply(&frame);
        let size = i32::try_from(rewritten.len()).unwrap().to_be_bytes();
        client.write_all(&size).await?;
        client.write_all(&rewritten).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
