//! The reader of the metadata and the watermarks of the topic.

use async_trait::async_trait;
use event_stream_domain::{TopicError, TopicProbe, Watermarks};

use super::{Connector, Pool};

/// Reads the partition count and the watermarks of the topic, each call within the request
/// time-out, on a reader of its own that it keeps between calls.
#[derive(Debug)]
pub struct KafkaProbe {
    pool: Pool,
}

impl KafkaProbe {
    pub(super) const fn new(connector: Connector) -> Self {
        Self {
            pool: Pool::new(connector),
        }
    }

    async fn read_partition_count(&self) -> Result<Option<u32>, TopicError> {
        let connector = &self.pool.connector;
        self.pool
            .with_reader(async |reader| connector.partition_count(reader).await)
            .await
    }

    async fn read_watermarks(&self) -> Result<Watermarks, TopicError> {
        let connector = &self.pool.connector;
        self.pool
            .with_reader(async |reader| connector.watermarks(reader).await)
            .await
    }
}

#[async_trait]
impl TopicProbe for KafkaProbe {
    async fn partition_count(&self) -> Result<Option<u32>, TopicError> {
        self.read_partition_count().await
    }

    async fn watermarks(&self) -> Result<Watermarks, TopicError> {
        self.read_watermarks().await
    }
}

#[cfg(test)]
mod tests;
