//! The Kafka adapter of the topic ports.
//!
//! [`KafkaTopic`] builds the three readers from the settings and touches no network. Each reader
//! connects a consumer of its own, on a [`Kafka`] handle of its own, before its first call and
//! again after a call that found the topic unavailable: the client keeps one connection per broker
//! and handle, and a parked long poll would delay every other call on the same handle. A consumer
//! joins no group, commits nothing, reads committed records only and never resets its offset on
//! its own. Every client call runs inside a time limit of the service, because the client bounds
//! a poll by its request time-out and not by its argument.

pub mod probe;
pub mod replay;
pub mod tail;

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use event_stream_domain::{Batch, Cursor, Offset, TopicError, TopicRecord, Watermarks};
use krafka::auth::{AuthConfig, TlsConfig};
use krafka::consumer::{AutoOffsetReset, Consumer, IsolationLevel};
use krafka::{ConsumerRecord, Kafka, KafkaBuilder, KrafkaError};
use shared_domain::BoxError;

pub use probe::KafkaProbe;
pub use replay::{KafkaReplay, REPLAY_WAIT};
pub use tail::KafkaTail;

/// The most records one poll hands out and the most the client buffers: one replay page.
const MAX_RECORDS: i32 = 500;

/// The soft bound in bytes of one fetch, per partition and in all: one replay page.
const MAX_BYTES: i32 = 1_048_576;

/// The only partition of the topic.
const PARTITION: i32 = 0;

/// The SASL mechanism of `KAFKA_SASL_MECHANISM`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaslMechanism {
    /// `PLAIN`.
    Plain,
    /// `SCRAM-SHA-256`.
    ScramSha256,
    /// `SCRAM-SHA-512`.
    ScramSha512,
}

/// The SASL credentials of a connection. The `Debug` output prints the password as
/// `<redacted>`, so a logged or printed value never holds it.
#[derive(Clone, PartialEq, Eq)]
pub struct SaslCredentials {
    /// The mechanism.
    pub mechanism: SaslMechanism,
    /// The user name (`KAFKA_SASL_USERNAME`).
    pub username: String,
    /// The password (`KAFKA_SASL_PASSWORD`); never logged.
    pub password: String,
}

impl fmt::Debug for SaslCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SaslCredentials")
            .field("mechanism", &self.mechanism)
            .field("username", &self.username)
            .field("password", &format_args!("<redacted>"))
            .finish()
    }
}

/// The security protocol of `KAFKA_SECURITY_PROTOCOL`, with the credentials of the SASL ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KafkaSecurity {
    /// `plaintext`.
    Plaintext,
    /// `ssl`.
    Ssl,
    /// `sasl_plaintext`.
    SaslPlaintext(SaslCredentials),
    /// `sasl_ssl`.
    SaslSsl(SaslCredentials),
}

/// The connection settings of every Kafka client of the node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KafkaSettings {
    /// The validated `host:port` entries of `KAFKA_BOOTSTRAP_SERVERS`, joined with `,`.
    pub bootstrap_servers: String,
    /// The topic (`KAFKA_TOPIC`).
    pub topic: String,
    /// The client id of every connection (`KAFKA_CLIENT_ID`).
    pub client_id: String,
    /// The security protocol and its credentials.
    pub security: KafkaSecurity,
    /// The request time-out of the client (`KAFKA_REQUEST_TIMEOUT`).
    pub request_timeout: Duration,
    /// The connect time-out of the client (`KAFKA_CONNECT_TIMEOUT`), at most the request time-out.
    pub connect_timeout: Duration,
    /// The long-poll wait of the tail (`KAFKA_FETCH_MAX_WAIT`).
    pub fetch_wait: Duration,
}

/// Makes the builder of every new handle: its bootstrap servers and its transport.
type Handles = Arc<dyn Fn() -> KafkaBuilder + Send + Sync>;

/// The factory of the three readers of the topic. It builds no connection: each reader connects
/// on its own when it is first called.
#[derive(Clone, Debug)]
pub struct KafkaTopic {
    connector: Connector,
}

impl KafkaTopic {
    /// The readers of the topic of `settings`, on the brokers of its bootstrap servers.
    #[must_use]
    pub fn new(settings: KafkaSettings) -> Self {
        let bootstrap_servers = settings.bootstrap_servers.clone();
        Self::with_builder(settings, move || Kafka::builder(bootstrap_servers.as_str()))
    }

    /// The readers of the topic of `settings`, on the handles that `builder` starts: an
    /// in-process test broker hands out builders of its own transport. Every other setting of
    /// a handle comes from `settings`; their bootstrap servers are not read.
    #[must_use]
    pub(crate) fn with_builder<B>(settings: KafkaSettings, builder: B) -> Self
    where
        B: Fn() -> KafkaBuilder + Send + Sync + 'static,
    {
        Self {
            connector: Connector {
                settings: Arc::new(settings),
                handles: Arc::new(builder),
            },
        }
    }

    /// The reader of the metadata and the watermarks of the topic.
    #[must_use]
    pub fn probe(&self) -> KafkaProbe {
        KafkaProbe::new(self.connector.clone())
    }

    /// The reader of the tail of the topic.
    #[must_use]
    pub fn tail(&self) -> KafkaTail {
        KafkaTail::new(self.connector.clone())
    }

    /// The reader of pages of older records.
    #[must_use]
    pub fn replay(&self) -> KafkaReplay {
        KafkaReplay::new(self.connector.clone())
    }
}

/// What every reader shares: the settings, and how to start a handle and read through it.
#[derive(Clone)]
struct Connector {
    settings: Arc<KafkaSettings>,
    handles: Handles,
}

impl fmt::Debug for Connector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Connector")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl Connector {
    /// A consumer of partition 0 of the topic on a new handle.
    async fn connect(&self) -> Result<Reader, TopicError> {
        connect_consumer(&self.settings, (self.handles)()).await
    }

    /// The partition count of the topic, `None` when the metadata holds no such topic.
    async fn partition_count(&self, reader: &Reader) -> Result<Option<u32>, TopicError> {
        let topic = self.settings.topic.as_str();
        let metadata = bounded(
            self.settings.request_timeout,
            reader.0.fetch_metadata(Some(topic)),
        )
        .await?
        .map_err(unavailable)?;
        Ok(metadata
            .topics
            .iter()
            .find(|info| info.name == topic)
            .map(|info| u32::try_from(info.partition_count()).unwrap_or(u32::MAX)))
    }

    /// The log start offset and the last stable offset of partition 0.
    async fn watermarks(&self, reader: &Reader) -> Result<Watermarks, TopicError> {
        let read = reader.0.fetch_watermarks(&self.settings.topic, PARTITION);
        let (low, high) = bounded(self.settings.request_timeout, read)
            .await?
            .map_err(unavailable)?;
        Ok(Watermarks {
            earliest: Cursor::new(to_position(low)?),
            end: Cursor::new(to_position(high)?),
        })
    }

    /// Moves the reader to `position`; the client also clears its fetch backoff.
    async fn seek(&self, reader: &Reader, position: Cursor) -> Result<(), TopicError> {
        let offset = i64::try_from(position.value()).unwrap_or(i64::MAX);
        let seek = reader.0.seek(&self.settings.topic, PARTITION, offset);
        bounded(self.settings.request_timeout, seek)
            .await?
            .map_err(unavailable)
    }

    /// One poll of at most `wait` from the reader's position `from` and what follows it, all
    /// within one limit of the request time-out plus `wait`: the read of the log start after a
    /// position below it included.
    ///
    /// The limit is awaited in place rather than through [`bounded`], whose argument would hold
    /// a second copy of the poll's future, the read of the log start included.
    async fn read(
        &self,
        reader: &Reader,
        from: Cursor,
        wait: Duration,
    ) -> Result<Batch, TopicError> {
        let limit = self.settings.request_timeout.saturating_add(wait);
        tokio::time::timeout(limit, self.poll(reader, from, wait))
            .await
            .map_err(unavailable)?
    }

    /// One poll of at most `wait` from the reader's position `from`, then the position after it;
    /// a failed poll is the port's error for it.
    async fn poll(
        &self,
        reader: &Reader,
        from: Cursor,
        wait: Duration,
    ) -> Result<Batch, TopicError> {
        match reader.0.poll(wait).await {
            Ok(records) => {
                let position = reader.0.position(&self.settings.topic, PARTITION).await;
                batch(from, records, position)
            }
            Err(error) => Err(self.failure(reader, error).await),
        }
    }

    /// The port's error for a failed poll: a position below the log start is `BelowEarliest`
    /// with the log start read just now, every other error is `Unavailable`.
    async fn failure(&self, reader: &Reader, error: KrafkaError) -> TopicError {
        match map_error(error) {
            ClientFailure::BelowLogStart => match self.watermarks(reader).await {
                Ok(watermarks) => TopicError::BelowEarliest {
                    earliest: watermarks.earliest,
                },
                Err(error) => error,
            },
            ClientFailure::Unavailable(error) => error,
        }
    }
}

/// A connected consumer of partition 0 of the topic, alone on its handle.
struct Reader(Consumer);

impl fmt::Debug for Reader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Reader")
    }
}

/// Idle readers: a call takes one, or connects a new one when none is idle, and gives it back
/// after a successful call; a failed reader is dropped.
#[derive(Debug)]
struct Pool {
    connector: Connector,
    idle: Mutex<Vec<Reader>>,
}

impl Pool {
    const fn new(connector: Connector) -> Self {
        Self {
            connector,
            idle: Mutex::new(Vec::new()),
        }
    }

    /// An idle reader, or a new one.
    async fn take(&self) -> Result<Reader, TopicError> {
        let idle = self.locked().pop();
        match idle {
            Some(reader) => Ok(reader),
            None => self.connector.connect().await,
        }
    }

    /// Runs `call` on an idle reader, or on a new one, and keeps the reader for a later call
    /// after a success; a failed reader is dropped.
    async fn with_reader<T>(
        &self,
        call: impl AsyncFnOnce(&Reader) -> Result<T, TopicError>,
    ) -> Result<T, TopicError> {
        let reader = self.take().await?;
        let result = call(&reader).await;
        if result.is_ok() {
            self.locked().push(reader);
        }
        result
    }

    fn locked(&self) -> MutexGuard<'_, Vec<Reader>> {
        self.idle.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Connects a handle from `builder` with the client id, the security and the time-outs of
/// `settings`, and builds a consumer without a group, assigned partition 0 of the topic.
async fn connect_consumer(
    settings: &KafkaSettings,
    builder: KafkaBuilder,
) -> Result<Reader, TopicError> {
    let connect = builder
        .client_id(settings.client_id.as_str())
        .security(auth_config(&settings.security))
        .request_timeout(settings.request_timeout)
        .connect_timeout(settings.connect_timeout)
        .connect();
    let kafka = bounded(settings.connect_timeout, connect)
        .await?
        .map_err(unavailable)?;
    let consumer = kafka
        .consumer_without_group()
        .isolation_level(IsolationLevel::ReadCommitted)
        .auto_offset_reset(AutoOffsetReset::None)
        .metrics_push(false)
        .fetch_min_bytes(1)
        .fetch_max_wait(settings.fetch_wait)
        .max_poll_records(MAX_RECORDS)
        .max_buffered_records(MAX_RECORDS)
        .max_partition_fetch_bytes(MAX_BYTES)
        .fetch_max_bytes(MAX_BYTES)
        .build()
        .await
        .map_err(unavailable)?;
    let assign = consumer.assign(&settings.topic, vec![PARTITION]);
    bounded(settings.request_timeout, assign)
        .await?
        .map_err(unavailable)?;
    Ok(Reader(consumer))
}

/// The security configuration of the client for `security`.
fn auth_config(security: &KafkaSecurity) -> AuthConfig {
    match security {
        KafkaSecurity::Plaintext => AuthConfig::plaintext(),
        KafkaSecurity::Ssl => AuthConfig::ssl(TlsConfig::new()),
        KafkaSecurity::SaslPlaintext(credentials) => sasl(credentials),
        KafkaSecurity::SaslSsl(credentials) => sasl(credentials).with_tls(TlsConfig::new()),
    }
}

/// The SASL configuration over cleartext for `credentials`.
fn sasl(credentials: &SaslCredentials) -> AuthConfig {
    let (username, password) = (credentials.username.as_str(), credentials.password.as_str());
    match credentials.mechanism {
        SaslMechanism::Plain => AuthConfig::sasl_plain(username, password),
        SaslMechanism::ScramSha256 => AuthConfig::sasl_scram_sha256(username, password),
        SaslMechanism::ScramSha512 => AuthConfig::sasl_scram_sha512(username, password),
    }
}

/// Runs a client call within `limit`; an elapsed limit is `Unavailable` with the elapsed error as
/// its source, and the client's own result is handed back.
async fn bounded<T, F>(limit: Duration, call: F) -> Result<Result<T, KrafkaError>, TopicError>
where
    F: Future<Output = Result<T, KrafkaError>>,
{
    tokio::time::timeout(limit, call).await.map_err(unavailable)
}

/// What a failed client call means for a reader.
#[derive(Debug)]
enum ClientFailure {
    /// The position is below the log start: the client has no offset and must not reset it.
    BelowLogStart,
    /// The topic is unavailable; the client error is the source.
    Unavailable(TopicError),
}

/// Classifies a client error: `NoOffset` means the position is below the log start, every other
/// error (an undecodable batch included) makes the topic unavailable.
fn map_error(error: KrafkaError) -> ClientFailure {
    if matches!(error, KrafkaError::NoOffset { .. }) {
        ClientFailure::BelowLogStart
    } else {
        ClientFailure::Unavailable(unavailable(error))
    }
}

/// The topic is unavailable because of `error`, which is kept as the source.
fn unavailable<E: Into<BoxError>>(error: E) -> TopicError {
    TopicError::Unavailable(error.into())
}

/// A negative offset handed out by the client.
#[derive(Debug, thiserror::Error)]
#[error("the client returned the negative offset {0}")]
struct NegativeOffset(i64);

/// An offset of the client as a position; a negative one is `Unavailable`.
fn to_position(offset: i64) -> Result<u64, TopicError> {
    u64::try_from(offset)
        .ok()
        .ok_or_else(|| unavailable(NegativeOffset(offset)))
}

/// The batch of a poll that read from `from`: the records at or above `from`, in the order of the
/// client, without key and headers, and the reader position after them, which is at least `from`,
/// at least the last offset plus 1 and at least the client's `position`.
fn batch(
    from: Cursor,
    records: Vec<ConsumerRecord>,
    position: Option<i64>,
) -> Result<Batch, TopicError> {
    let mut next = from;
    let mut kept = Vec::with_capacity(records.len());
    for record in records {
        let offset = to_position(record.offset)?;
        if offset >= from.value() {
            next = next.max(Cursor::new(offset).advanced_by(1));
            kept.push(TopicRecord {
                offset: Offset::new(offset),
                // Takes the allocation over when the record owns it alone, else copies it.
                value: record.value.map(Vec::from),
            });
        }
    }
    if let Some(position) = position {
        next = next.max(Cursor::new(to_position(position)?));
    }
    Ok(Batch {
        records: kept,
        next,
    })
}

/// Whether the tail may keep its reader after `result`: only `Unavailable` drops it, so the next
/// call connects again; after `BelowEarliest` the reader serves the seek that follows.
const fn keeps<T>(result: &Result<T, TopicError>) -> bool {
    !matches!(result, Err(TopicError::Unavailable(_)))
}

#[cfg(test)]
mod tests;
