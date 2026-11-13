//! Bus doubles.

use std::any::TypeId;
use std::sync::{Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use shared_domain::{
    Command, CommandBus, DomainEvent, ErasedBusError, ErasedMessage, EventBus, EventBusError,
    Query, QueryBus, SubscriberFailure, Uuid,
};

type CommandAnswer = Box<dyn Fn(ErasedMessage) -> Result<(), ErasedBusError> + Send + Sync>;
type QueryAnswer =
    Box<dyn Fn(ErasedMessage) -> Result<ErasedMessage, ErasedBusError> + Send + Sync>;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Command bus that answers with a closure and records the names of the dispatched commands.
pub struct FakeCommandBus {
    answer: Option<(TypeId, CommandAnswer)>,
    dispatched: Mutex<Vec<&'static str>>,
}

impl std::fmt::Debug for FakeCommandBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeCommandBus").finish_non_exhaustive()
    }
}

impl FakeCommandBus {
    /// Answers every `C` with the closure's result.
    #[must_use]
    pub fn handling<C, F>(answer: F) -> Self
    where
        C: Command,
        F: Fn(C) -> Result<(), C::Error> + Send + Sync + 'static,
    {
        let erased: CommandAnswer = Box::new(move |message| {
            let command = message
                .downcast::<C>()
                .map_err(|_other| ErasedBusError::NotRegistered { message: C::NAME })?;
            answer(*command).map_err(|error| ErasedBusError::Handler(Box::new(error)))
        });
        Self {
            answer: Some((TypeId::of::<C>(), erased)),
            dispatched: Mutex::new(Vec::new()),
        }
    }

    /// Has no handler: every dispatch answers `NotRegistered`.
    #[must_use]
    pub const fn unregistered() -> Self {
        Self {
            answer: None,
            dispatched: Mutex::new(Vec::new()),
        }
    }

    /// Names of the dispatched commands, in dispatch order.
    #[must_use]
    pub fn dispatched(&self) -> Vec<&'static str> {
        locked(&self.dispatched).clone()
    }
}

#[async_trait]
impl CommandBus for FakeCommandBus {
    async fn dispatch_erased(
        &self,
        command_type: TypeId,
        name: &'static str,
        command: ErasedMessage,
    ) -> Result<(), ErasedBusError> {
        locked(&self.dispatched).push(name);
        match &self.answer {
            Some((handled, answer)) if *handled == command_type => answer(command),
            _ => Err(ErasedBusError::NotRegistered { message: name }),
        }
    }
}

/// Query bus that answers with a closure and records the names of the asked queries.
pub struct FakeQueryBus {
    answer: Option<(TypeId, QueryAnswer)>,
    asked: Mutex<Vec<&'static str>>,
}

impl std::fmt::Debug for FakeQueryBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeQueryBus").finish_non_exhaustive()
    }
}

impl FakeQueryBus {
    /// Answers every `Q` with the closure's result.
    #[must_use]
    pub fn answering<Q, F>(answer: F) -> Self
    where
        Q: Query,
        F: Fn(Q) -> Result<Q::Output, Q::Error> + Send + Sync + 'static,
    {
        let erased: QueryAnswer = Box::new(move |message| {
            let query = message
                .downcast::<Q>()
                .map_err(|_other| ErasedBusError::NotRegistered { message: Q::NAME })?;
            match answer(*query) {
                Ok(output) => Ok(Box::new(output)),
                Err(error) => Err(ErasedBusError::Handler(Box::new(error))),
            }
        });
        Self {
            answer: Some((TypeId::of::<Q>(), erased)),
            asked: Mutex::new(Vec::new()),
        }
    }

    /// Has no handler: every query answers `NotRegistered`.
    #[must_use]
    pub const fn unregistered() -> Self {
        Self {
            answer: None,
            asked: Mutex::new(Vec::new()),
        }
    }

    /// Names of the asked queries, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<&'static str> {
        locked(&self.asked).clone()
    }
}

#[async_trait]
impl QueryBus for FakeQueryBus {
    async fn ask_erased(
        &self,
        query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError> {
        locked(&self.asked).push(name);
        match &self.answer {
            Some((asked, answer)) if *asked == query_type => answer(query),
            _ => Err(ErasedBusError::NotRegistered { message: name }),
        }
    }
}

/// What a [`RecordingEventBus`] keeps of a published event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishedEvent {
    /// Wire name of the event.
    pub event_name: &'static str,
    /// Identifier of the event.
    pub event_id: Uuid,
    /// Identifier of the aggregate that recorded it.
    pub aggregate_id: Uuid,
}

/// Event bus that records the events it is given.
#[derive(Debug, Default)]
pub struct RecordingEventBus {
    failing: bool,
    published: Mutex<Vec<PublishedEvent>>,
}

impl RecordingEventBus {
    /// Records and succeeds.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records and fails with one `SubscriberFailure` per published event.
    #[must_use]
    pub const fn failing() -> Self {
        Self {
            failing: true,
            published: Mutex::new(Vec::new()),
        }
    }

    /// The published events, in publication order.
    #[must_use]
    pub fn published(&self) -> Vec<PublishedEvent> {
        locked(&self.published).clone()
    }
}

#[async_trait]
impl EventBus for RecordingEventBus {
    async fn publish(&self, events: Vec<Box<dyn DomainEvent>>) -> Result<(), EventBusError> {
        let recorded: Vec<PublishedEvent> = events
            .iter()
            .map(|event| PublishedEvent {
                event_name: event.event_name(),
                event_id: event.event_id(),
                aggregate_id: event.aggregate_id(),
            })
            .collect();
        locked(&self.published).extend(recorded.iter().copied());
        if !self.failing || recorded.is_empty() {
            return Ok(());
        }
        Err(EventBusError {
            failures: recorded
                .into_iter()
                .map(|event| SubscriberFailure {
                    event_name: event.event_name,
                    event_id: event.event_id,
                    source: "recording bus failure".into(),
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests;
