//! The reader of the tail of the topic.

use async_trait::async_trait;
use event_stream_domain::{Batch, Cursor, TopicError, TopicTail};

use super::{Connector, Reader, keeps};

/// Long-polls the topic from its position, owned by one loop.
///
/// It keeps its reader between calls and drops it after a call that found the topic unavailable;
/// the next call connects a new one and seeks it to the position first. A call that is cancelled
/// drops its reader too.
#[derive(Debug)]
pub struct KafkaTail {
    connector: Connector,
    reader: Option<Reader>,
    position: Cursor,
}

impl KafkaTail {
    pub(super) const fn new(connector: Connector) -> Self {
        Self {
            connector,
            reader: None,
            position: Cursor::START,
        }
    }

    async fn move_to(&mut self, position: Cursor) -> Result<(), TopicError> {
        self.position = position;
        let reader = self.take().await?;
        let result = self.connector.seek(&reader, position).await;
        self.give_back(reader, &result);
        result
    }

    async fn next_batch(&mut self) -> Result<Batch, TopicError> {
        let reader = self.take().await?;
        let wait = self.connector.settings.fetch_wait;
        let result = self.connector.read(&reader, self.position, wait).await;
        if let Ok(batch) = &result {
            self.position = batch.next;
        }
        self.give_back(reader, &result);
        result
    }

    /// The kept reader, or a new one sought to the position.
    async fn take(&mut self) -> Result<Reader, TopicError> {
        if let Some(reader) = self.reader.take() {
            return Ok(reader);
        }
        let reader = self.connector.connect().await?;
        self.connector.seek(&reader, self.position).await?;
        Ok(reader)
    }

    fn give_back<T>(&mut self, reader: Reader, result: &Result<T, TopicError>) {
        if keeps(result) {
            self.reader = Some(reader);
        }
    }
}

#[async_trait]
impl TopicTail for KafkaTail {
    async fn seek(&mut self, position: Cursor) -> Result<(), TopicError> {
        self.move_to(position).await
    }

    async fn fetch(&mut self) -> Result<Batch, TopicError> {
        self.next_batch().await
    }
}

#[cfg(test)]
mod tests;
