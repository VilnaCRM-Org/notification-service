//! The hub of this node, which hands every event to the queues of its local subscribers.
//!
//! It never waits, encodes the frame once per event and shares it, counts the connections against
//! the subscriber cap and lets a subscriber whose queue overflowed fall behind and re-arm on the
//! same connection (FR-24, FR-25, NFR-1, NFR-2, NFR-10). `advance` is the method of
//! [`EventPublisher`] and `replay_capped` the method of [`SubscriberCount`], so that each name
//! has one definition.
//!
//! Every method is one synchronous section under the registry mutex; no lock is held across an
//! await, and a poisoned mutex is taken over.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use axum::http::StatusCode;
use event_stream_domain::{Cursor, EventType, StoredEvent};
use shared_application::http::Problem;
use tokio::sync::{mpsc, watch};

use crate::http::frame::Frame;
use crate::status::SubscriberCount;
use crate::tail::EventPublisher;

/// The bounds of the hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HubSettings {
    /// The largest number of connections the node holds (`MAX_SUBSCRIBERS`).
    pub max_subscribers: usize,
    /// The number of frames a subscriber's queue holds (`SUBSCRIBER_BUFFER`); at least 1 is used.
    pub buffer: usize,
}

/// Why the hub refused a subscriber (FR-25).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HubError {
    /// The node has not reached its start goal yet.
    #[error("the node is starting")]
    NotReady,
    /// The node holds `max_subscribers` connections.
    #[error("the node is at its subscriber capacity")]
    AtCapacity,
    /// The node is shutting down.
    #[error("the node is shutting down")]
    Closed,
}

impl From<HubError> for Problem {
    /// The 503 problem with the text of the error as detail, logged at `warn` as
    /// `subscription rejected` (NFR-15).
    fn from(error: HubError) -> Self {
        let detail = error.to_string();
        log_subscription_rejected(&detail);
        Self::new(StatusCode::SERVICE_UNAVAILABLE).with_detail(detail)
    }
}

fn log_subscription_rejected(detail: &str) {
    tracing::warn!(detail = %detail, "subscription rejected");
}

/// What one [`Hub::deliver`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Delivery {
    /// The subscribers whose queue took the frame.
    pub delivered: usize,
    /// The subscribers whose queue was full: they fell behind with this event.
    pub behind: usize,
}

type FrameSender = mpsc::Sender<Arc<Frame>>;

/// One connection: its queue (`None` while it is behind or after the hub closed) and its types.
#[derive(Debug)]
struct Entry {
    sender: Option<FrameSender>,
    types: BTreeSet<EventType>,
}

/// The state behind the mutex of [`Hub`].
///
/// `by_type` holds one clone of an entry's sender under each of the entry's types and under no
/// other key, only while the entry has a sender; it has no empty map. It is only read and written
/// by key.
#[derive(Debug, Default)]
struct Registry {
    closed: bool,
    position: Option<Cursor>,
    next_id: u64,
    subscribers: BTreeMap<u64, Entry>,
    by_type: HashMap<EventType, BTreeMap<u64, FrameSender>>,
}

impl Registry {
    /// `Closed`, then `NotReady`, then `AtCapacity`; else the position a new subscriber gets.
    fn admit(&self, max_subscribers: usize) -> Result<Cursor, HubError> {
        if self.closed {
            return Err(HubError::Closed);
        }
        let position = self.position.ok_or(HubError::NotReady)?;
        if self.subscribers.len() >= max_subscribers {
            return Err(HubError::AtCapacity);
        }
        Ok(position)
    }

    /// Puts a clone of `sender` under each of `types`.
    fn index(&mut self, id: u64, types: &BTreeSet<EventType>, sender: &FrameSender) {
        for event_type in types {
            self.by_type
                .entry(event_type.clone())
                .or_default()
                .insert(id, sender.clone());
        }
    }

    /// Takes the clones of subscriber `id` from under each of `types`; a map left empty goes.
    fn unindex(&mut self, id: u64, types: &BTreeSet<EventType>) {
        for event_type in types {
            if let Some(senders) = self.by_type.get_mut(event_type) {
                senders.remove(&id);
                if senders.is_empty() {
                    self.by_type.remove(event_type);
                }
            }
        }
    }

    /// The subscriber fell behind: its sender goes, its entry and its types stay.
    fn fall_behind(&mut self, id: u64) {
        if let Some(entry) = self.subscribers.get_mut(&id) {
            entry.sender = None;
            let types = entry.types.clone();
            self.unindex(id, &types);
        }
    }
}

/// The hub of this node. See the module documentation for its rules.
#[derive(Debug)]
pub struct Hub {
    settings: HubSettings,
    registry: Mutex<Registry>,
    count: watch::Sender<usize>,
    replay_capped: AtomicU64,
}

impl Hub {
    /// An empty hub without a position: it refuses subscribers until [`Hub::start`].
    #[must_use]
    pub fn new(settings: HubSettings) -> Arc<Self> {
        Arc::new(Self {
            settings,
            registry: Mutex::new(Registry::default()),
            count: watch::Sender::new(0),
            replay_capped: AtomicU64::new(0),
        })
    }

    /// Under one lock, in this order: `Closed`, `NotReady`, `AtCapacity` (FR-25's precedence),
    /// then registers a subscriber of `types` at the hub position and publishes the new count.
    ///
    /// # Errors
    ///
    /// The first of `Closed`, `NotReady` and `AtCapacity` that applies; nothing is registered.
    pub fn register(self: &Arc<Self>, types: &BTreeSet<EventType>) -> Result<Subscriber, HubError> {
        let mut registry = self.lock();
        let position = registry.admit(self.settings.max_subscribers)?;
        let id = registry.next_id;
        registry.next_id = id.wrapping_add(1);
        let (sender, receiver) = mpsc::channel(self.buffer());
        registry.index(id, types, &sender);
        registry.subscribers.insert(
            id,
            Entry {
                sender: Some(sender),
                types: types.clone(),
            },
        );
        self.count.send_replace(registry.subscribers.len());
        drop(registry);
        Ok(Subscriber {
            id,
            position,
            receiver,
            hub: Arc::clone(self),
        })
    }

    /// Sets the position when the hub has none; a second call changes nothing.
    pub fn start(&self, position: Cursor) {
        self.lock().position.get_or_insert(position);
    }

    /// Sets the position to the event's sequence, also when the type has no subscriber; then
    /// queues one shared frame for every subscriber of its type with `try_send`. `None` when the
    /// type has no subscriber: nothing is encoded, and the position has still moved. Before
    /// [`Hub::start`] the position stays unset.
    pub fn deliver(&self, event: &StoredEvent) -> Option<Delivery> {
        let mut registry = self.lock();
        if let Some(position) = registry.position.as_mut() {
            *position = Cursor::from(event.sequence);
        }
        let senders = registry.by_type.get(&event.event_type)?;
        let frame = Arc::new(Frame::encode(event));
        let behind: Vec<u64> = senders
            .iter()
            .filter(|(_id, sender)| sender.try_send(Arc::clone(&frame)).is_err())
            .map(|(id, _sender)| *id)
            .collect();
        let delivered = senders.len().saturating_sub(behind.len());
        for id in &behind {
            registry.fall_behind(*id);
        }
        drop(registry);
        Some(Delivery {
            delivered,
            behind: behind.len(),
        })
    }

    /// Refuses every later subscriber and ends every queue after its queued frames; the entries
    /// stay, so the count and [`Hub::drained`] follow the connections.
    pub fn close(&self) {
        let mut registry = self.lock();
        registry.closed = true;
        registry.by_type.clear();
        for entry in registry.subscribers.values_mut() {
            entry.sender = None;
        }
        drop(registry);
    }

    /// True once [`Hub::close`] ran.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.lock().closed
    }

    /// The number of connections registered.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.lock().subscribers.len()
    }

    /// The number of event types with at least one armed subscriber.
    #[must_use]
    pub fn type_count(&self) -> usize {
        self.lock().by_type.len()
    }

    /// Completes when no subscriber is registered.
    pub async fn drained(&self) {
        let mut count = self.count.subscribe();
        drop(count.wait_for(|subscribers| *subscribers == 0).await);
    }

    fn buffer(&self) -> usize {
        self.settings.buffer.max(1)
    }

    fn lock(&self) -> MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl EventPublisher for Hub {
    fn started(&self, position: Cursor) {
        self.start(position);
    }

    fn publish(&self, event: &StoredEvent) -> usize {
        self.deliver(event).map_or(0, |delivery| delivery.behind)
    }

    /// The position becomes the max of the current one and `position`; no effect before
    /// [`Hub::start`].
    fn advance(&self, position: Cursor) {
        let mut registry = self.lock();
        if let Some(current) = registry.position.as_mut() {
            *current = (*current).max(position);
        }
        drop(registry);
    }
}

impl SubscriberCount for Hub {
    fn closed(&self) -> bool {
        self.is_closed()
    }

    fn count(&self) -> usize {
        self.subscriber_count()
    }

    /// Replays whose floor the cap of `EVENT_REPLAY_MAX_EVENTS` raised (no lock).
    fn replay_capped(&self) -> u64 {
        self.replay_capped.load(Ordering::Relaxed)
    }
}

/// One connection registered in the hub; dropping it frees its slot.
#[derive(Debug)]
pub struct Subscriber {
    id: u64,
    position: Cursor,
    receiver: mpsc::Receiver<Arc<Frame>>,
    hub: Arc<Hub>,
}

impl Subscriber {
    /// The hub position at the last arming (registration or resume).
    #[must_use]
    pub const fn position(&self) -> Cursor {
        self.position
    }

    /// The next queued frame; `None` when the queue ended, because the subscriber fell behind or
    /// the hub closed.
    pub async fn recv(&mut self) -> Option<Arc<Frame>> {
        self.receiver.recv().await
    }

    /// A new, empty queue after [`Subscriber::recv`] returned `None`, indexed under the current
    /// types; returns and stores the hub position at re-arming.
    ///
    /// # Errors
    ///
    /// `Closed` when the hub closed; nothing changes.
    pub fn resume(&mut self) -> Result<Cursor, HubError> {
        let mut registry = self.hub.lock();
        if registry.closed {
            return Err(HubError::Closed);
        }
        let position = registry.position.unwrap_or(self.position);
        let (sender, receiver) = mpsc::channel(self.hub.buffer());
        if let Some(entry) = registry.subscribers.get_mut(&self.id) {
            entry.sender = Some(sender.clone());
            let types = entry.types.clone();
            registry.index(self.id, &types, &sender);
        }
        drop(registry);
        self.receiver = receiver;
        self.position = position;
        Ok(position)
    }

    /// Adds the types; returns the hub position at that moment. While the subscriber is behind or
    /// the hub is closed, only its type set changes.
    #[must_use]
    pub fn subscribe(&self, types: &BTreeSet<EventType>) -> Cursor {
        let mut registry = self.hub.lock();
        let position = registry.position.unwrap_or(self.position);
        let armed = registry.subscribers.get_mut(&self.id).and_then(|entry| {
            entry.types.extend(types.iter().cloned());
            entry.sender.clone()
        });
        if let Some(sender) = armed {
            registry.index(self.id, types, &sender);
        }
        drop(registry);
        position
    }

    /// Removes the types; a type left without a subscriber leaves the index.
    pub fn unsubscribe(&self, types: &BTreeSet<EventType>) {
        let mut registry = self.hub.lock();
        registry
            .subscribers
            .entry(self.id)
            .and_modify(|entry| entry.types.retain(|event_type| !types.contains(event_type)));
        registry.unindex(self.id, types);
        drop(registry);
    }

    /// True once the hub closed.
    #[must_use]
    pub fn hub_closed(&self) -> bool {
        self.hub.is_closed()
    }

    /// Raises the hub's counter of capped replays by one.
    pub fn note_replay_capped(&self) {
        self.hub.replay_capped.fetch_add(1, Ordering::Relaxed);
    }
}

impl Drop for Subscriber {
    /// Removes the entry and its queues and publishes the new count.
    fn drop(&mut self) {
        let mut registry = self.hub.lock();
        if let Some(entry) = registry.subscribers.remove(&self.id) {
            registry.unindex(self.id, &entry.types);
        }
        self.hub.count.send_replace(registry.subscribers.len());
        drop(registry);
    }
}

#[cfg(test)]
mod tests;
