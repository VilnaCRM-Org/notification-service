//! The broker of the test stack, read from the environment the Make targets export, and topics of
//! one test each.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher as _;
use std::time::Duration;

use event_stream_domain::{Cursor, TopicProbe as _, TopicRecord};
use event_stream_infrastructure::{KafkaSecurity, KafkaSettings, KafkaTopic};
use krafka::Kafka;
use krafka::admin::{CreateTopicsOptions, DeleteRecordsOptions, DeleteTopicsOptions, NewTopic};
use krafka::consumer::TopicPartition;
use krafka::producer::{Acks, Producer, Record};

/// The limit of every awaited call of a test.
pub(crate) const TEST_LIMIT: Duration = Duration::from_secs(60);

/// The request time-out of the readers: generous, so a call that must succeed does not depend on
/// the speed of the machine.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The connect time-out of the readers.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Bootstrap address that nothing listens on: the "unreachable broker" case. No container is
/// stopped.
pub(crate) const DEAD_BOOTSTRAP_SERVERS: &str = "127.0.0.1:1";

#[expect(
    clippy::disallowed_methods,
    reason = "the broker of the test stack is described by the environment"
)]
fn bootstrap_servers() -> String {
    std::env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| {
        panic!("KAFKA_BOOTSTRAP_SERVERS is not set; run the tests through `make`")
    })
}

/// Awaits `future` within [`TEST_LIMIT`].
pub(crate) async fn bounded<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(TEST_LIMIT, future)
        .await
        .expect("the call must end on its own")
}

/// The settings of readers of `topic` on the broker of the test stack.
pub(crate) fn settings(topic: &str) -> KafkaSettings {
    KafkaSettings {
        bootstrap_servers: bootstrap_servers(),
        topic: topic.to_owned(),
        client_id: "event-stream-integration".to_owned(),
        security: KafkaSecurity::Plaintext,
        request_timeout: REQUEST_TIMEOUT,
        connect_timeout: CONNECT_TIMEOUT,
        fetch_wait: Duration::from_millis(500),
    }
}

/// A topic name no other test and no earlier run uses: the test's name and a random number of
/// the process (std's hash seed), so parallel and repeated runs never share a topic.
pub(crate) fn unique_name(test: &str) -> String {
    let random = RandomState::new().hash_one(test);
    format!("event-stream-it.{test}.{random:016x}")
}

/// A handle of a test client on the broker of the test stack.
async fn test_client() -> Kafka {
    bounded(
        Kafka::builder(bootstrap_servers())
            .client_id("event-stream-integration-admin")
            .request_timeout(REQUEST_TIMEOUT)
            .connect(),
    )
    .await
    .unwrap()
}

/// A topic of one test, created on the broker of the test stack; [`TestTopic::delete`] removes it.
pub(crate) struct TestTopic {
    pub(crate) name: String,
    kafka: Kafka,
}

impl TestTopic {
    /// Creates a topic of `partitions` partitions named after `test`, and waits until its
    /// metadata names them.
    pub(crate) async fn create(test: &str, partitions: i32) -> Self {
        let name = unique_name(test);
        let kafka = test_client().await;
        let created = bounded(kafka.admin().create_topics(
            [NewTopic::new(name.as_str(), partitions, 1).unwrap()],
            CreateTopicsOptions::default(),
        ))
        .await
        .unwrap();
        created[&name].clone().unwrap();
        let topic = Self { name, kafka };
        topic.wait_for_partitions(partitions).await;
        topic
    }

    async fn wait_for_partitions(&self, partitions: i32) {
        let probe = KafkaTopic::new(settings(&self.name)).probe();
        let expected = u32::try_from(partitions).ok();
        bounded(async {
            while bounded(probe.partition_count()).await.unwrap() != expected {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
    }

    /// The readers of this topic.
    pub(crate) fn readers(&self) -> KafkaTopic {
        KafkaTopic::new(settings(&self.name))
    }

    async fn producer(&self) -> Producer {
        bounded(
            self.kafka
                .producer()
                .acks(Acks::All)
                .linger(Duration::ZERO)
                .build(),
        )
        .await
        .unwrap()
    }

    /// Produces each value as one record of partition 0, with a key and a header, `acks` all.
    pub(crate) async fn produce(&self, values: &[String]) {
        let producer = self.producer().await;
        let mut deliveries = Vec::with_capacity(values.len());
        for value in values {
            let record = Record::new(self.name.as_str(), value.clone().into_bytes())
                .partition(0)
                .key(b"key".to_vec())
                .header("source", b"test".to_vec());
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
    pub(crate) async fn produce_in_transaction(&self, id: &str, values: &[&str], commit: bool) {
        let transactional_id = format!("{}.{id}", self.name);
        let producer = bounded(self.kafka.producer().build_transactional(transactional_id))
            .await
            .unwrap();
        producer.begin().unwrap();
        let mut last = None;
        for value in values {
            let record = Record::new(self.name.as_str(), value.as_bytes().to_vec()).partition(0);
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
        let probe = self.readers().probe();
        bounded(async {
            while bounded(probe.watermarks()).await.unwrap().end < end {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
    }

    /// Deletes the records of partition 0 below `offset`, which moves the log start there.
    pub(crate) async fn delete_records_before(&self, offset: i64) {
        let deleted = bounded(self.kafka.admin().delete_records(
            [(TopicPartition::new(self.name.as_str(), 0), offset)],
            DeleteRecordsOptions::default(),
        ))
        .await
        .unwrap();
        assert!(deleted.values().all(Result::is_ok), "{deleted:?}");
    }

    /// Removes the topic.
    pub(crate) async fn delete(&self) {
        bounded(
            self.kafka
                .admin()
                .delete_topics([self.name.as_str()], DeleteTopicsOptions::default()),
        )
        .await
        .unwrap();
    }
}

/// The values of `records` as text.
pub(crate) fn values(records: &[TopicRecord]) -> Vec<String> {
    records
        .iter()
        .map(|record| String::from_utf8(record.value.clone().unwrap()).unwrap())
        .collect()
}

/// The values `prefix-0` to `prefix-(count - 1)`.
pub(crate) fn numbered(prefix: &str, count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("{prefix}-{index}"))
        .collect()
}
