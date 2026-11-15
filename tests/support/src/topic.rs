//! Scripted doubles of the topic ports.
//!
//! Each double answers from a script of answers, in order, and records every call before it
//! answers. Once its script is empty it pends for ever (`std::future::pending`) instead of
//! answering, so a loop that waits on it stops and a mutant fails an assertion instead of hanging
//! a test. A double is a shared handle: a clone sees and feeds the same script and the same calls,
//! so a test keeps a clone after it hands the double to the code under test.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use event_stream_domain::{
    Batch, Cursor, TopicError, TopicProbe, TopicReplay, TopicTail, Watermarks,
};

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Records `call`, then takes the next answer of `script`.
fn record_and_take<C, A>(calls: &Mutex<Vec<C>>, call: C, script: &Mutex<VecDeque<A>>) -> Option<A> {
    locked(calls).push(call);
    locked(script).pop_front()
}

/// The answer, or a wait for ever when the script was empty.
async fn answer<A>(next: Option<A>) -> A {
    match next {
        Some(answer) => answer,
        None => std::future::pending().await,
    }
}

/// A call made to a [`ScriptedTopicProbe`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeCall {
    /// `partition_count`.
    PartitionCount,
    /// `watermarks`.
    Watermarks,
}

#[derive(Debug, Default)]
struct ProbeScript {
    partition_counts: Mutex<VecDeque<Result<Option<u32>, TopicError>>>,
    watermarks: Mutex<VecDeque<Result<Watermarks, TopicError>>>,
    calls: Mutex<Vec<ProbeCall>>,
}

/// A [`TopicProbe`] that answers from two scripts, one per method.
#[derive(Debug, Clone, Default)]
pub struct ScriptedTopicProbe(Arc<ProbeScript>);

impl ScriptedTopicProbe {
    /// A probe with empty scripts.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the next answer of `partition_count`.
    pub fn push_partition_count(&self, answer: Result<Option<u32>, TopicError>) {
        locked(&self.0.partition_counts).push_back(answer);
    }

    /// Adds the next answer of `watermarks`.
    pub fn push_watermarks(&self, answer: Result<Watermarks, TopicError>) {
        locked(&self.0.watermarks).push_back(answer);
    }

    /// Every call so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<ProbeCall> {
        locked(&self.0.calls).clone()
    }

    async fn next_partition_count(&self) -> Result<Option<u32>, TopicError> {
        answer(record_and_take(
            &self.0.calls,
            ProbeCall::PartitionCount,
            &self.0.partition_counts,
        ))
        .await
    }

    async fn next_watermarks(&self) -> Result<Watermarks, TopicError> {
        answer(record_and_take(
            &self.0.calls,
            ProbeCall::Watermarks,
            &self.0.watermarks,
        ))
        .await
    }
}

#[async_trait]
impl TopicProbe for ScriptedTopicProbe {
    async fn partition_count(&self) -> Result<Option<u32>, TopicError> {
        self.next_partition_count().await
    }

    async fn watermarks(&self) -> Result<Watermarks, TopicError> {
        self.next_watermarks().await
    }
}

/// A call made to a [`ScriptedTopicTail`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TailCall {
    /// `seek` to the position.
    Seek(Cursor),
    /// `fetch`.
    Fetch,
}

#[derive(Debug, Default)]
struct TailScript {
    seeks: Mutex<VecDeque<Result<(), TopicError>>>,
    fetches: Mutex<VecDeque<Result<Batch, TopicError>>>,
    calls: Mutex<Vec<TailCall>>,
}

/// A [`TopicTail`] that answers from two scripts, one per method, and records the seek positions
/// and the fetches in one ordered list.
#[derive(Debug, Clone, Default)]
pub struct ScriptedTopicTail(Arc<TailScript>);

impl ScriptedTopicTail {
    /// A tail with empty scripts.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the next answer of `seek`.
    pub fn push_seek(&self, answer: Result<(), TopicError>) {
        locked(&self.0.seeks).push_back(answer);
    }

    /// Adds the next answer of `fetch`.
    pub fn push_fetch(&self, answer: Result<Batch, TopicError>) {
        locked(&self.0.fetches).push_back(answer);
    }

    /// Every call so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<TailCall> {
        locked(&self.0.calls).clone()
    }

    async fn next_seek(&self, position: Cursor) -> Result<(), TopicError> {
        answer(record_and_take(
            &self.0.calls,
            TailCall::Seek(position),
            &self.0.seeks,
        ))
        .await
    }

    async fn next_fetch(&self) -> Result<Batch, TopicError> {
        answer(record_and_take(
            &self.0.calls,
            TailCall::Fetch,
            &self.0.fetches,
        ))
        .await
    }
}

#[async_trait]
impl TopicTail for ScriptedTopicTail {
    async fn seek(&mut self, position: Cursor) -> Result<(), TopicError> {
        self.next_seek(position).await
    }

    async fn fetch(&mut self) -> Result<Batch, TopicError> {
        self.next_fetch().await
    }
}

#[derive(Debug, Default)]
struct ReplayScript {
    pages: Mutex<VecDeque<Result<Batch, TopicError>>>,
    calls: Mutex<Vec<Cursor>>,
}

/// A [`TopicReplay`] that answers from a script and records the page positions.
#[derive(Debug, Clone, Default)]
pub struct ScriptedTopicReplay(Arc<ReplayScript>);

impl ScriptedTopicReplay {
    /// A replay with an empty script.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the next answer of `page`.
    pub fn push_page(&self, answer: Result<Batch, TopicError>) {
        locked(&self.0.pages).push_back(answer);
    }

    /// The position of every page read so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<Cursor> {
        locked(&self.0.calls).clone()
    }

    async fn next_page(&self, from: Cursor) -> Result<Batch, TopicError> {
        answer(record_and_take(&self.0.calls, from, &self.0.pages)).await
    }
}

#[async_trait]
impl TopicReplay for ScriptedTopicReplay {
    async fn page(&self, from: Cursor) -> Result<Batch, TopicError> {
        self.next_page(from).await
    }
}

#[cfg(test)]
mod tests;
