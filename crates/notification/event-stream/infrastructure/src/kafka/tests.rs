//! The adapter's tests against the client's in-memory broker, with paused time.
//!
//! The fake-broker cases that the design files under `tests/integration/fake_broker.rs` live here
//! and in the sibling test modules of the three readers: they assert private state (the reader and
//! the position of the tail, the idle readers of a pool), which an integration test cannot see.

use std::collections::BTreeSet;
use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use event_stream_domain::{
    Cursor, Offset, TopicError, TopicProbe as _, TopicRecord, TopicReplay as _, TopicTail as _,
};
use krafka::auth::SecurityProtocol;
use krafka::producer::{Acks, Producer, Record, RecordMetadata};
use krafka::testing::{ApiKey, Control, FakeBroker, TelemetrySubscription};
use krafka::{ConsumerRecord, KrafkaError, ProtocolErrorKind};
use rstest::rstest;
use tokio::time::Instant;
use tokio::time::error::Elapsed;

use super::*;

/// The topic of every fake broker of these tests.
pub(super) const TOPIC: &str = "notifications";
/// The client id of the readers.
pub(super) const CLIENT_ID: &str = "notification-service";
/// `t`.
pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
/// `c`, below `t` so that the two limits differ.
pub(super) const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
/// The long-poll wait of the tail: not the client's default of 500 ms, so a test sees whether the
/// setting reaches the consumer, and below `t`.
pub(super) const FETCH_WAIT: Duration = Duration::from_millis(1500);
/// The limit of every awaited call of a test.
pub(super) const TEST_LIMIT: Duration = Duration::from_secs(60);

fn credentials(mechanism: SaslMechanism) -> SaslCredentials {
    SaslCredentials {
        mechanism,
        username: "reader".to_owned(),
        password: "not-a-real-pass".to_owned(),
    }
}

/// The settings of the readers of these tests.
pub(super) fn settings(security: KafkaSecurity) -> KafkaSettings {
    KafkaSettings {
        bootstrap_servers: "localhost:9092".to_owned(),
        topic: TOPIC.to_owned(),
        client_id: CLIENT_ID.to_owned(),
        security,
        request_timeout: REQUEST_TIMEOUT,
        connect_timeout: CONNECT_TIMEOUT,
        fetch_wait: FETCH_WAIT,
    }
}

/// Awaits `future` within [`TEST_LIMIT`].
pub(super) async fn bounded_test<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(TEST_LIMIT, future)
        .await
        .expect("the call must end on its own")
}

/// The source of an `Unavailable` error as text; panics on any other result.
pub(super) fn unavailable_source<T: fmt::Debug>(result: Result<T, TopicError>) -> String {
    match result {
        Err(TopicError::Unavailable(source)) => source.to_string(),
        other => panic!("expected Unavailable, got {other:?}"),
    }
}

/// A record of the topic as the ports hand it out.
pub(super) fn record(offset: u64, value: &str) -> TopicRecord {
    TopicRecord {
        offset: Offset::new(offset),
        value: Some(value.as_bytes().to_vec()),
    }
}

/// Sends `record` and checks that it landed in partition 0 of the topic.
async fn send(producer: &Producer, record: Record) {
    let metadata: RecordMetadata = bounded_test(producer.send(record)).await.unwrap();
    assert_eq!((&*metadata.topic, metadata.partition), (TOPIC, PARTITION));
}

/// An in-memory broker of one node that holds the topic with one partition.
pub(super) struct Broker {
    pub(super) fake: Arc<FakeBroker>,
}

impl Broker {
    pub(super) fn start() -> Self {
        let fake = FakeBroker::start_in_memory(1);
        assert!(fake.create_topic(TOPIC, 1));
        Self {
            fake: Arc::new(fake),
        }
    }

    /// The readers of the topic on this broker.
    pub(super) fn topic(&self) -> KafkaTopic {
        self.topic_with(settings(KafkaSecurity::Plaintext))
    }

    /// The readers of `settings` on this broker.
    pub(super) fn topic_with(&self, settings: KafkaSettings) -> KafkaTopic {
        let fake = Arc::clone(&self.fake);
        KafkaTopic::with_builder(settings, move || fake.kafka())
    }

    /// A handle of a test client that is not a reader of the adapter, logged in with the test
    /// credentials when the broker asks for them.
    async fn test_client(&self, security: AuthConfig) -> Kafka {
        let connect = self
            .fake
            .kafka()
            .client_id("test-producer")
            .security(security)
            .connect();
        bounded_test(connect).await.unwrap()
    }

    /// Produces each value as one record of partition 0 with `acks` all.
    pub(super) async fn produce(&self, values: &[&str]) {
        let client = self.test_client(AuthConfig::plaintext()).await;
        let build = client
            .producer()
            .acks(Acks::All)
            .linger(Duration::ZERO)
            .build();
        let producer = bounded_test(build).await.unwrap();
        for value in values {
            let record = Record::new(TOPIC, value.as_bytes().to_vec()).partition(PARTITION);
            send(&producer, record).await;
        }
    }

    /// Produces the record `secured` through a client that logs in with the test credentials.
    async fn produce_with_sasl(&self) {
        let security = AuthConfig::sasl_plain("reader", "not-a-real-pass");
        let client = self.test_client(security).await;
        let producer = bounded_test(client.producer().build()).await.unwrap();
        send(
            &producer,
            Record::new(TOPIC, b"secured".to_vec()).partition(PARTITION),
        )
        .await;
    }

    /// Produces `values` in one transaction, then commits it or aborts it.
    pub(super) async fn produce_in_transaction(&self, id: &str, values: &[&str], commit: bool) {
        let client = self.test_client(AuthConfig::plaintext()).await;
        let producer = bounded_test(client.producer().build_transactional(id))
            .await
            .unwrap();
        producer.begin().unwrap();
        for value in values {
            let record = Record::new(TOPIC, value.as_bytes().to_vec()).partition(PARTITION);
            let metadata = bounded_test(producer.send(record)).await.unwrap();
            assert_eq!(metadata.partition, PARTITION);
        }
        if commit {
            bounded_test(producer.commit()).await.unwrap();
        } else {
            bounded_test(producer.abort()).await.unwrap();
        }
        drop(producer);
    }

    /// Produces a record with a key and without a value.
    pub(super) async fn produce_tombstone(&self) {
        let client = self.test_client(AuthConfig::plaintext()).await;
        let producer = bounded_test(client.producer().build()).await.unwrap();
        send(
            &producer,
            Record::tombstone(TOPIC, b"key".to_vec()).partition(PARTITION),
        )
        .await;
    }

    /// Moves the log start of partition 0 to `offset`.
    pub(super) fn move_log_start(&self, offset: i64) {
        self.fake.with_state(|state| {
            state
                .partition_mut(TOPIC, PARTITION)
                .unwrap()
                .log_start_offset = offset;
        });
    }

    /// The connections the readers of the adapter opened so far.
    pub(super) fn connections(&self) -> usize {
        self.fake
            .requests()
            .iter()
            .filter(|request| request.client_id.as_deref() == Some(CLIENT_ID))
            .map(|request| request.connection)
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// The requests of the readers of the adapter for `api`.
    pub(super) fn requests_of_readers(&self, api: ApiKey) -> usize {
        self.fake
            .requests()
            .iter()
            .filter(|request| request.client_id.as_deref() == Some(CLIENT_ID))
            .filter(|request| request.api_key == api)
            .count()
    }
}

/// A connected reader of `topic`, sought to the start.
async fn reader_at_start(topic: &KafkaTopic) -> Reader {
    let reader = bounded_test(topic.connector.connect()).await.unwrap();
    bounded_test(topic.connector.seek(&reader, Cursor::START))
        .await
        .unwrap();
    reader
}

#[test]
fn the_debug_output_of_the_credentials_redacts_the_password() {
    let printed = format!("{:?}", credentials(SaslMechanism::ScramSha512));

    assert_eq!(
        printed,
        r#"SaslCredentials { mechanism: ScramSha512, username: "reader", password: <redacted> }"#
    );
}

#[test]
fn the_debug_output_of_the_settings_never_holds_the_password() {
    for security in [
        KafkaSecurity::SaslPlaintext(credentials(SaslMechanism::Plain)),
        KafkaSecurity::SaslSsl(credentials(SaslMechanism::ScramSha256)),
    ] {
        let printed = format!("{:?}", settings(security));

        assert!(!printed.contains("not-a-real-pass"), "{printed}");
        assert!(printed.contains("password: <redacted>"), "{printed}");
        assert!(printed.contains(r#"username: "reader""#), "{printed}");
    }
}

#[test]
fn the_redacted_output_is_the_same_for_every_password() {
    let mut other = credentials(SaslMechanism::Plain);
    other.password = "another".to_owned();

    assert_eq!(
        format!("{other:?}"),
        format!("{:?}", credentials(SaslMechanism::Plain))
    );
    assert_ne!(other, credentials(SaslMechanism::Plain));
}

#[test]
fn the_settings_without_credentials_print_every_field() {
    let printed = format!("{:?}", settings(KafkaSecurity::Plaintext));

    assert_eq!(
        printed,
        "KafkaSettings { bootstrap_servers: \"localhost:9092\", topic: \"notifications\", \
         client_id: \"notification-service\", security: Plaintext, request_timeout: 3s, \
         connect_timeout: 2s, fetch_wait: 1.5s }"
    );
}

#[test]
fn the_topic_and_its_readers_print_the_settings_without_the_password() {
    let security = KafkaSecurity::SaslSsl(credentials(SaslMechanism::Plain));
    let topic = KafkaTopic::new(settings(security.clone()));
    let settings_text = format!("{:?}", settings(security));
    let connector = format!("Connector {{ settings: {settings_text}, .. }}");

    let printed_topic = format!("{topic:?}");
    let printed_tail = format!("{:?}", topic.tail());
    let printed_pools = [
        format!("{:?}", topic.probe()),
        format!("{:?}", topic.replay()),
    ];

    assert_eq!(
        printed_topic,
        format!("KafkaTopic {{ connector: {connector} }}")
    );
    assert_eq!(
        printed_tail,
        format!("KafkaTail {{ connector: {connector}, reader: None, position: Cursor(0) }}")
    );
    for printed in printed_pools {
        assert!(printed.contains(&connector), "{printed}");
        assert!(printed.contains("data: []"), "{printed}");
        assert!(!printed.contains("not-a-real-pass"), "{printed}");
    }
}

#[tokio::test(start_paused = true)]
async fn a_reader_prints_its_name_only() {
    let broker = Broker::start();
    let topic = broker.topic();

    let reader = bounded_test(topic.connector.connect()).await.unwrap();
    let printed = format!("{reader:?}");
    drop(reader);

    assert_eq!(printed, "Reader");
}

#[tokio::test(start_paused = true)]
async fn building_the_topic_and_its_readers_starts_no_handle_and_each_reader_starts_its_own() {
    let broker = Broker::start();
    let started = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&started);
    let fake = Arc::clone(&broker.fake);
    let topic = KafkaTopic::with_builder(settings(KafkaSecurity::Plaintext), move || {
        counter.fetch_add(1, Ordering::SeqCst);
        fake.kafka()
    });

    let probe = topic.probe();
    let mut tail = topic.tail();
    let replay = topic.replay();
    let started_before_calls = started.load(Ordering::SeqCst);
    let requests_before_calls = broker.fake.requests().len();
    bounded_test(probe.watermarks()).await.unwrap();
    bounded_test(tail.fetch()).await.unwrap();
    bounded_test(replay.page(Cursor::START)).await.unwrap();
    drop((probe, tail, replay));

    assert_eq!(started_before_calls, 0);
    assert_eq!(requests_before_calls, 0);
    assert_eq!(started.load(Ordering::SeqCst), 3);
    assert_eq!(broker.connections(), 3);
}

#[tokio::test(start_paused = true)]
async fn a_reader_uses_the_client_id_of_the_settings_and_joins_no_group() {
    let broker = Broker::start();
    broker.produce(&["a"]).await;
    let topic = broker.topic();
    let reader = reader_at_start(&topic).await;

    let batch = bounded_test(topic.connector.read(&reader, Cursor::START, FETCH_WAIT))
        .await
        .unwrap();
    drop(reader);

    assert_eq!(batch.records, vec![record(0, "a")]);
    let requests = broker.fake.requests();
    assert_ne!(
        requests
            .iter()
            .filter(|request| request.client_id.as_deref() == Some(CLIENT_ID))
            .count(),
        0
    );
    assert!(
        requests
            .iter()
            .all(|request| request.client_id.as_deref() != Some("krafka"))
    );
    for api in [
        ApiKey::Produce,
        ApiKey::FindCoordinator,
        ApiKey::JoinGroup,
        ApiKey::OffsetCommit,
        ApiKey::OffsetFetch,
        ApiKey::CreateTopics,
    ] {
        assert_eq!(broker.requests_of_readers(api), 0, "{api:?}");
    }
}

#[tokio::test(start_paused = true)]
async fn a_reader_pushes_no_telemetry_to_a_broker_that_asks_for_it() {
    let broker = Broker::start();
    broker.fake.set_telemetry(Some(TelemetrySubscription::new(
        ["org.apache.kafka."],
        Duration::from_millis(100),
    )));
    let topic = broker.topic();
    let reader = reader_at_start(&topic).await;

    for _ in 0..5 {
        bounded_test(topic.connector.read(&reader, Cursor::START, FETCH_WAIT))
            .await
            .unwrap();
    }
    drop(reader);

    assert_eq!(
        broker.requests_of_readers(ApiKey::GetTelemetrySubscriptions),
        0
    );
    assert_eq!(broker.fake.telemetry_pushes().len(), 0);
}

#[tokio::test(start_paused = true)]
async fn a_crashed_broker_is_unavailable_at_once() {
    let broker = Broker::start();
    broker.fake.crash(0);
    let topic = broker.topic();
    let started = Instant::now();

    let source = unavailable_source(bounded_test(topic.connector.connect()).await);

    assert!(source.contains("network error"), "{source}");
    assert_eq!(started.elapsed(), Duration::ZERO);
}

#[tokio::test(start_paused = true)]
async fn a_broker_that_never_answers_the_handshake_is_unavailable_after_the_connect_time_out() {
    let broker = Broker::start();
    broker.fake.on(ApiKey::ApiVersions, |_| Control::Silence);
    let topic = broker.topic();
    let started = Instant::now();

    let source = unavailable_source(bounded_test(topic.connector.connect()).await);

    assert_eq!(source, "deadline has elapsed");
    assert_eq!(started.elapsed(), CONNECT_TIMEOUT);
}

#[tokio::test(start_paused = true)]
async fn a_reader_authenticates_with_the_sasl_plain_credentials_of_the_settings() {
    let broker = Broker::start();
    broker.fake.require_sasl_plain("reader", "not-a-real-pass");
    broker.produce_with_sasl().await;
    let topic = broker.topic_with(settings(KafkaSecurity::SaslPlaintext(credentials(
        SaslMechanism::Plain,
    ))));
    let reader = reader_at_start(&topic).await;

    let batch = bounded_test(topic.connector.read(&reader, Cursor::START, FETCH_WAIT))
        .await
        .unwrap();
    drop(reader);

    assert_eq!(batch.records, vec![record(0, "secured")]);
    assert_ne!(broker.requests_of_readers(ApiKey::SaslAuthenticate), 0);
}

#[tokio::test(start_paused = true)]
async fn wrong_sasl_credentials_are_unavailable() {
    let broker = Broker::start();
    broker.fake.require_sasl_plain("reader", "another-pass");
    let topic = broker.topic_with(settings(KafkaSecurity::SaslPlaintext(credentials(
        SaslMechanism::Plain,
    ))));

    let source = unavailable_source(bounded_test(topic.connector.connect()).await);

    assert!(source.contains("authentication"), "{source}");
}

#[rstest]
#[case::plaintext(KafkaSecurity::Plaintext, SecurityProtocol::Plaintext)]
#[case::ssl(KafkaSecurity::Ssl, SecurityProtocol::Ssl)]
#[case::sasl_plaintext(
    KafkaSecurity::SaslPlaintext(credentials(SaslMechanism::Plain)),
    SecurityProtocol::SaslPlaintext
)]
#[case::sasl_ssl(
    KafkaSecurity::SaslSsl(credentials(SaslMechanism::Plain)),
    SecurityProtocol::SaslSsl
)]
fn each_security_protocol_has_its_client_protocol(
    #[case] security: KafkaSecurity,
    #[case] expected: SecurityProtocol,
) {
    let config = auth_config(&security);

    assert_eq!(config.security_protocol(), &expected);
    assert_eq!(
        config.tls_config().is_some(),
        matches!(expected, SecurityProtocol::Ssl | SecurityProtocol::SaslSsl)
    );
    assert_eq!(
        config.sasl_mechanism().is_some(),
        matches!(
            expected,
            SecurityProtocol::SaslPlaintext | SecurityProtocol::SaslSsl
        )
    );
}

#[rstest]
#[case::plain(SaslMechanism::Plain, krafka::auth::SaslMechanism::Plain)]
#[case::scram_sha_256(SaslMechanism::ScramSha256, krafka::auth::SaslMechanism::ScramSha256)]
#[case::scram_sha_512(SaslMechanism::ScramSha512, krafka::auth::SaslMechanism::ScramSha512)]
fn each_sasl_mechanism_has_its_client_mechanism_over_both_protocols(
    #[case] mechanism: SaslMechanism,
    #[case] expected: krafka::auth::SaslMechanism,
) {
    for security in [
        KafkaSecurity::SaslPlaintext(credentials(mechanism)),
        KafkaSecurity::SaslSsl(credentials(mechanism)),
    ] {
        let config = auth_config(&security);

        assert_eq!(config.sasl_mechanism(), Some(&expected));
        assert_eq!(
            config.scram_credentials().is_some(),
            mechanism != SaslMechanism::Plain
        );
    }
}

#[test]
fn the_plain_credentials_are_the_user_name_and_the_password_of_the_settings() {
    let config = auth_config(&KafkaSecurity::SaslPlaintext(credentials(
        SaslMechanism::Plain,
    )));

    let bytes = config.plain_credentials().unwrap().to_auth_bytes();

    assert_eq!(bytes.as_slice(), b"\0reader\0not-a-real-pass");
}

#[test]
fn no_offset_means_below_the_log_start() {
    let failure = map_error(KrafkaError::no_offset(vec![(TOPIC.to_owned(), 0)]));

    assert!(
        matches!(failure, ClientFailure::BelowLogStart),
        "{failure:?}"
    );
}

#[rstest]
#[case::undecodable_batch(KrafkaError::protocol_kind(ProtocolErrorKind::CrcMismatch, "bad crc"))]
#[case::network(KrafkaError::network(io::Error::from(io::ErrorKind::ConnectionReset)))]
#[case::time_out(KrafkaError::timeout("fetch"))]
#[case::configuration(KrafkaError::config("request_timeout"))]
#[case::unknown_topic(KrafkaError::unknown_topic(TOPIC))]
fn every_other_client_error_is_unavailable_with_the_error_as_its_source(
    #[case] error: KrafkaError,
) {
    let text = error.to_string();

    let ClientFailure::Unavailable(TopicError::Unavailable(source)) = map_error(error) else {
        panic!("expected Unavailable");
    };

    assert_eq!(source.to_string(), text);
    assert!(source.downcast_ref::<KrafkaError>().is_some());
}

#[rstest]
#[case(0, 0)]
#[case(7, 7)]
#[case(i64::MAX, 9_223_372_036_854_775_807)]
fn a_non_negative_offset_is_a_position(#[case] offset: i64, #[case] expected: u64) {
    assert_eq!(to_position(offset).unwrap(), expected);
}

#[rstest]
#[case(-1)]
#[case(i64::MIN)]
fn a_negative_offset_is_unavailable(#[case] offset: i64) {
    let source = unavailable_source(to_position(offset));

    assert_eq!(
        source,
        format!("the client returned the negative offset {offset}")
    );
}

fn consumer_record(offset: i64, value: Option<&str>) -> ConsumerRecord {
    ConsumerRecord::new(
        TOPIC,
        PARTITION,
        offset,
        Some(b"ignored-key".to_vec().into()),
        value.map(|value| value.as_bytes().to_vec().into()),
    )
}

#[test]
fn a_batch_keeps_the_records_at_or_above_the_position_without_their_keys() {
    let records = vec![
        consumer_record(3, Some("three")),
        consumer_record(4, Some("four")),
        consumer_record(5, None),
        consumer_record(6, Some("six")),
    ];

    let batch = batch(Cursor::new(4), records, Some(7)).unwrap();

    assert_eq!(
        batch.records,
        vec![
            record(4, "four"),
            TopicRecord {
                offset: Offset::new(5),
                value: None,
            },
            record(6, "six"),
        ]
    );
    assert_eq!(batch.next, Cursor::new(7));
}

#[test]
fn the_next_position_follows_the_last_record_when_the_client_position_is_behind_or_unknown() {
    let records = vec![consumer_record(9, Some("nine"))];

    assert_eq!(
        batch(Cursor::new(2), records.clone(), Some(4))
            .unwrap()
            .next,
        Cursor::new(10)
    );
    assert_eq!(
        batch(Cursor::new(2), records, None).unwrap().next,
        Cursor::new(10)
    );
}

#[test]
fn the_next_position_follows_the_client_position_past_transaction_markers() {
    let records = vec![consumer_record(2, Some("two"))];

    let batch = batch(Cursor::new(2), records, Some(5)).unwrap();

    assert_eq!(batch.records, vec![record(2, "two")]);
    assert_eq!(batch.next, Cursor::new(5));
}

#[test]
fn the_next_position_is_never_below_the_requested_one() {
    let below = vec![consumer_record(1, Some("one"))];

    let empty_behind = batch(Cursor::new(8), Vec::new(), Some(3)).unwrap();
    let empty_unknown = batch(Cursor::new(8), Vec::new(), None).unwrap();
    let only_below = batch(Cursor::new(8), below, None).unwrap();

    assert_eq!(empty_behind.next, Cursor::new(8));
    assert_eq!(empty_unknown.next, Cursor::new(8));
    assert_eq!(only_below.records, Vec::new());
    assert_eq!(only_below.next, Cursor::new(8));
}

#[test]
fn a_negative_offset_of_a_record_or_of_the_position_is_unavailable() {
    let records = vec![consumer_record(-2, Some("bad"))];

    assert_eq!(
        unavailable_source(batch(Cursor::START, records, Some(1))),
        "the client returned the negative offset -2"
    );
    assert_eq!(
        unavailable_source(batch(Cursor::START, Vec::new(), Some(-1))),
        "the client returned the negative offset -1"
    );
}

#[test]
fn only_an_unavailable_topic_drops_a_reader() {
    assert!(keeps::<()>(&Ok(())));
    assert!(keeps::<()>(&Err(TopicError::BelowEarliest {
        earliest: Cursor::new(3),
    })));
    assert!(!keeps::<()>(&Err(unavailable(KrafkaError::timeout(
        "poll"
    )))));
}

#[tokio::test(start_paused = true)]
async fn an_elapsed_limit_is_unavailable_with_the_elapsed_error_as_its_source() {
    let started = Instant::now();

    let result = bounded(
        Duration::from_secs(1),
        std::future::pending::<Result<(), KrafkaError>>(),
    )
    .await;

    let Err(TopicError::Unavailable(source)) = result else {
        panic!("expected Unavailable");
    };
    assert!(source.downcast_ref::<Elapsed>().is_some());
    assert_eq!(started.elapsed(), Duration::from_secs(1));
}

#[tokio::test(start_paused = true)]
async fn a_call_within_its_limit_hands_back_the_client_result() {
    let answered = bounded(Duration::from_secs(1), async { Ok::<_, KrafkaError>(5) }).await;
    let failed = bounded(Duration::from_secs(1), async {
        Err::<u8, _>(KrafkaError::timeout("poll"))
    })
    .await;

    assert_eq!(answered.unwrap().unwrap(), 5);
    assert!(matches!(failed, Ok(Err(KrafkaError::Timeout { .. }))));
}

#[tokio::test(start_paused = true)]
async fn the_pool_gives_back_a_reader_after_a_successful_call_only() {
    let broker = Broker::start();
    let pool = Pool::new(broker.topic().connector);
    let mut measured_while_taken = None;

    let succeeded = bounded_test(pool.with_reader(async |_| Ok(5))).await;
    let idle_after_success = pool.locked().len();
    let unavailable_topic = bounded_test(pool.with_reader(async |_| {
        measured_while_taken = Some((pool.locked().len(), broker.connections()));
        Err::<(), _>(unavailable(KrafkaError::timeout("poll")))
    }))
    .await;
    let idle_after_unavailable = pool.locked().len();
    let below_earliest = bounded_test(pool.with_reader(async |_| {
        Err::<(), _>(TopicError::BelowEarliest {
            earliest: Cursor::new(3),
        })
    }))
    .await;
    let idle_after_below_earliest = pool.locked().len();
    let after_failures = bounded_test(pool.with_reader(async |_| Ok("again"))).await;
    drop(pool);

    assert_eq!(succeeded.unwrap(), 5);
    assert_eq!(idle_after_success, 1);
    assert_eq!(measured_while_taken, Some((0, 1)));
    assert!(unavailable_source(unavailable_topic).contains("poll"));
    assert_eq!(idle_after_unavailable, 0);
    assert!(
        matches!(below_earliest, Err(TopicError::BelowEarliest { earliest }) if earliest == Cursor::new(3)),
        "{below_earliest:?}"
    );
    assert_eq!(idle_after_below_earliest, 0);
    assert_eq!(after_failures.unwrap(), "again");
    assert_eq!(broker.connections(), 3);
}

#[tokio::test(start_paused = true)]
async fn the_pool_calls_nothing_when_it_cannot_connect_a_reader() {
    let broker = Broker::start();
    broker.fake.crash(0);
    let pool = Pool::new(broker.topic().connector);
    let mut called = false;

    let result = bounded_test(pool.with_reader(async |_| {
        called = true;
        Ok(())
    }))
    .await;
    let idle = pool.locked().len();
    drop(pool);

    assert!(unavailable_source(result).contains("network error"));
    assert!(!called);
    assert_eq!(idle, 0);
}

#[tokio::test(start_paused = true)]
async fn the_metadata_answer_without_the_topic_has_no_partition_count() {
    let broker = Broker::start();
    let settings = KafkaSettings {
        topic: "absent".to_owned(),
        ..settings(KafkaSecurity::Plaintext)
    };
    let probe = broker.topic_with(settings).probe();

    let count = bounded_test(probe.partition_count()).await.unwrap();
    drop(probe);

    assert_eq!(count, None);
    assert_eq!(broker.requests_of_readers(ApiKey::CreateTopics), 0);
    assert!(
        broker
            .fake
            .with_state(|state| state.partition("absent", 0).is_none())
    );
}
