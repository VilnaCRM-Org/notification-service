//! The reader of pages of older records.

use std::time::Duration;

use async_trait::async_trait;
use event_stream_domain::{Batch, Cursor, TopicError, TopicReplay};

use super::{Connector, Pool};

/// The wait of the one short poll of a replay page.
pub const REPLAY_WAIT: Duration = Duration::from_millis(100);

/// Reads one page per call: a seek, then one short poll of [`REPLAY_WAIT`].
///
/// A page ends within the request time-out plus that wait. Concurrent pages each take a reader
/// of their own from a pool and give it back after a successful page; a failed reader is
/// dropped. The application bounds the concurrent pages, and so the readers.
#[derive(Debug)]
pub struct KafkaReplay {
    pool: Pool,
}

impl KafkaReplay {
    pub(super) const fn new(connector: Connector) -> Self {
        Self {
            pool: Pool::new(connector),
        }
    }

    async fn read_page(&self, from: Cursor) -> Result<Batch, TopicError> {
        let connector = &self.pool.connector;
        self.pool
            .with_reader(async |reader| {
                connector.seek(reader, from).await?;
                connector.read(reader, from, REPLAY_WAIT).await
            })
            .await
    }
}

#[async_trait]
impl TopicReplay for KafkaReplay {
    async fn page(&self, from: Cursor) -> Result<Batch, TopicError> {
        self.read_page(from).await
    }
}

#[cfg(test)]
mod tests;
