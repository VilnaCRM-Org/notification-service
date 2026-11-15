//! The subscription of one streaming connection, read by position (FR-12 to FR-15, FR-23,
//! FR-24).
//!
//! Without a cursor it is live from the node's position. With a cursor it replays from the
//! window inside it, from Kafka below it (under the replay permits, in pages of at most 500
//! records) and waits for the tail ahead of it, clamped to the topic end; then it is live. After
//! falling behind it catches up by the same path on the same connection.
//!
//! The floor is the highest id settled for the connection: every wanted event at or below it was
//! written or deliberately skipped. Every written frame becomes the floor and a queued frame at or
//! below it is dropped, so ids on a connection strictly increase, no offset is written twice and
//! no id at or below the cursor is written. Run-time `subscribe` gives a new type the hub position
//! as its since, so no replay writes an older event of that type.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::iter;
use std::sync::Arc;

use event_stream_domain::{
    Batch, Cursor, EventType, PageLimit, Sequence, StoredEvent, TopicError, TopicRecord,
    TopicReplay,
};
use tokio::sync::{Semaphore, watch};

use crate::envelope::parse_event;
use crate::http::frame::Frame;
use crate::http::hub::{Hub, HubError, Subscriber};
use crate::tail::TailState;
use crate::window::{Window, WindowRead};

/// What a streaming request asks for, on both routes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriptionRequest {
    /// The event types of the subscription.
    pub types: BTreeSet<EventType>,
    /// The cursor of a resume; `None` asks for live events only.
    pub cursor: Option<Cursor>,
}

/// The bounds of the replays of this node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplaySettings {
    /// How far below the node's position a replay reaches (`EVENT_REPLAY_MAX_EVENTS`).
    pub max_events: u64,
    /// The largest record value a replay accepts, in bytes (`EVENT_MAX_DATA_BYTES`).
    pub max_data_bytes: usize,
    /// How many replays read Kafka at the same time (`EVENT_REPLAY_PERMITS`).
    pub permits: usize,
}

/// Opens the subscriptions of this node.
#[derive(Debug)]
pub struct Subscriptions {
    hub: Arc<Hub>,
    window: Arc<Window>,
    replay: Arc<dyn TopicReplay>,
    tail: watch::Receiver<TailState>,
    permits: Arc<Semaphore>,
    settings: ReplaySettings,
}

impl Subscriptions {
    /// Creates the semaphore of `settings.permits` permits, shared by every subscription it opens.
    #[must_use]
    pub fn new(
        hub: Arc<Hub>,
        window: Arc<Window>,
        replay: Arc<dyn TopicReplay>,
        tail: watch::Receiver<TailState>,
        settings: ReplaySettings,
    ) -> Self {
        let permits = settings.permits.min(Semaphore::MAX_PERMITS);
        Self {
            hub,
            window,
            replay,
            tail,
            permits: Arc::new(Semaphore::new(permits)),
            settings,
        }
    }

    /// Registers a subscriber of the requested types in the hub, at the hub position `p`. Without
    /// a cursor the subscription is live above `p`; with a cursor its first step decides where it
    /// reads. No broker read, nothing awaited.
    ///
    /// # Errors
    ///
    /// The refusal of the hub: `Closed`, `NotReady` or `AtCapacity`.
    pub fn open(&self, request: SubscriptionRequest) -> Result<Subscription, HubError> {
        let subscriber = self.hub.register(&request.types)?;
        let position = subscriber.position();
        let (floor, target) = request
            .cursor
            .map_or((position, Some(position)), |cursor| (cursor, None));
        Ok(Subscription {
            subscriber,
            window: Arc::clone(&self.window),
            replay: Arc::clone(&self.replay),
            tail: self.tail.clone(),
            permits: Arc::clone(&self.permits),
            settings: self.settings,
            types: request
                .types
                .into_iter()
                .map(|event_type| (event_type, Cursor::START))
                .collect(),
            floor,
            target,
            pending: VecDeque::new(),
            stalls: 0,
            ahead_since: None,
        })
    }
}

/// Why a subscription ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// The node is shutting down, or its tail stopped.
    HubClosed,
    /// A replay could not read Kafka (FR-14).
    TopicFailed,
}

/// What one [`Subscription::step`] did.
#[derive(Debug)]
pub enum Step {
    /// A frame to write.
    Frame(Arc<Frame>),
    /// The step moved the subscription without a frame to write.
    Idle,
    /// The subscription ended.
    End(EndReason),
}

/// A command would raise the types of a connection above [`Subscription::MAX_TYPES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a connection subscribes to at most 100 event types")]
pub struct TooManyTypes;

/// The subscription of one connection. See the module documentation for its rules.
#[derive(Debug)]
pub struct Subscription {
    subscriber: Subscriber,
    window: Arc<Window>,
    replay: Arc<dyn TopicReplay>,
    tail: watch::Receiver<TailState>,
    permits: Arc<Semaphore>,
    settings: ReplaySettings,
    /// Type -> since: only events above it are written.
    types: BTreeMap<EventType, Cursor>,
    /// Everything at or below it is settled for this connection.
    floor: Cursor,
    /// `None`: a replay must start; `Some(t)`: replay while the floor is below `t`, else live.
    target: Option<Cursor>,
    /// The rest of the last page.
    pending: VecDeque<Arc<StoredEvent>>,
    /// Kafka pages in a row without a record and without progress.
    stalls: u8,
    /// `TailState::observations` when the wait of a cursor ahead began.
    ahead_since: Option<u64>,
}

impl Subscription {
    /// The most event types one connection holds.
    pub const MAX_TYPES: usize = 100;
    /// The bounds of one replay page: 500 events and 1 MiB of event objects.
    pub const REPLAY_PAGE: PageLimit = PageLimit {
        rows: 500,
        bytes: 1_048_576,
    };
    /// Kafka pages in a row without a record and without progress that end a replay.
    const MAX_STALLS: u8 = 3;

    /// One step, the first that applies of: the rest of the last page, the start of a replay,
    /// a page of the window or of Kafka, the live queue.
    pub async fn step(&mut self) -> Step {
        if let Some(event) = self.pending.pop_front() {
            return self.drain(event);
        }
        match self.target {
            None => self.start().await,
            Some(target) if self.floor < target => self.page(target).await,
            Some(_) => self.live().await,
        }
    }

    /// Steps until a frame (`Some`) or the end (`None`). Every path that returns
    /// [`Step::Idle`] without waiting is followed by one that waits (a window page without an
    /// event yields, a Kafka page awaits its permit and its read, the live path its queue), so
    /// this loop cannot spin.
    pub async fn next(&mut self) -> Option<Arc<Frame>> {
        loop {
            match self.step().await {
                Step::Frame(frame) => return Some(frame),
                Step::Idle => {}
                Step::End(_reason) => return None,
            }
        }
    }

    /// True when the next step reads Kafka: a permit wait and one page read.
    #[must_use]
    pub fn replaying(&self) -> bool {
        self.pending.is_empty()
            && self.target.is_some_and(|target| self.floor < target)
            && self.floor < self.window.start()
    }

    /// Adds the types that are new, each with the hub position at this moment as its since; a
    /// held type keeps its since.
    ///
    /// # Errors
    ///
    /// `TooManyTypes` when the new types would raise the set above [`Self::MAX_TYPES`]; nothing
    /// changes.
    pub fn subscribe(&mut self, types: BTreeSet<EventType>) -> Result<(), TooManyTypes> {
        let added: BTreeSet<EventType> = types
            .into_iter()
            .filter(|event_type| !self.types.contains_key(event_type))
            .collect();
        if self.types.len().saturating_add(added.len()) > Self::MAX_TYPES {
            return Err(TooManyTypes);
        }
        let since = self.subscriber.subscribe(&added);
        self.types
            .extend(added.into_iter().map(|event_type| (event_type, since)));
        Ok(())
    }

    /// Removes the types from the hub entry and from the set; an absent type changes nothing.
    pub fn unsubscribe(&mut self, types: &BTreeSet<EventType>) {
        self.subscriber.unsubscribe(types);
        self.types
            .retain(|event_type, _since| !types.contains(event_type));
    }

    /// The number of types held.
    #[must_use]
    pub fn type_count(&self) -> usize {
        self.types.len()
    }

    /// The floor: the highest id settled for this connection.
    #[must_use]
    pub const fn position(&self) -> Cursor {
        self.floor
    }

    /// Takes events from the front of the last page until one is wanted; a page that ran empty
    /// is replaced by a new deque, so its memory goes.
    fn drain(&mut self, first: Arc<StoredEvent>) -> Step {
        let types = &self.types;
        let pending = &mut self.pending;
        let found = iter::once(first)
            .chain(iter::from_fn(|| pending.pop_front()))
            .find(|event| wanted(types, &event.event_type, event.sequence));
        if self.pending.is_empty() {
            self.pending = VecDeque::new();
        }
        found.map_or(Step::Idle, |event| {
            Step::Frame(Arc::new(Frame::encode(&event)))
        })
    }

    /// Decides the floor of a replay up to the subscriber's position `p`: the wait of a cursor
    /// ahead, live at once without a type, else the cap of `max_events` below `p`.
    async fn start(&mut self) -> Step {
        if self.subscriber.hub_closed() {
            return Step::End(EndReason::HubClosed);
        }
        let position = self.subscriber.position();
        if self.floor > position {
            return self.wait_ahead(position).await;
        }
        if self.types.is_empty() {
            self.floor = position;
        } else {
            self.cap(position);
        }
        self.target = Some(position);
        Step::Idle
    }

    /// Waits until the tail published two more observations than when the wait began, then
    /// clamps the floor to the end of that observation (FR-15). The starting count is kept, so
    /// a step future dropped and polled again does not restart the wait.
    async fn wait_ahead(&mut self, position: Cursor) -> Step {
        let tail = &self.tail;
        let since = *self
            .ahead_since
            .get_or_insert_with(|| tail.borrow().observations);
        let observed = self
            .tail
            .wait_for(|state| state.observations >= since.saturating_add(2))
            .await
            .map(|state| state.end);
        let Ok(end) = observed else {
            return Step::End(EndReason::HubClosed);
        };
        self.floor = end.map_or(self.floor, |end| self.floor.min(end));
        self.ahead_since = None;
        self.target = Some(position);
        Step::Idle
    }

    /// Raises a floor more than `max_events` below `position` to that cap (FR-14).
    fn cap(&mut self, position: Cursor) {
        let capped = position.rewound_by(self.settings.max_events);
        if self.floor < capped {
            log_replay_capped(self.floor, capped);
            self.floor = capped;
            self.subscriber.note_replay_capped();
        }
    }

    /// One page of the window, or of Kafka when the floor is below the window floor.
    async fn page(&mut self, target: Cursor) -> Step {
        if self.subscriber.hub_closed() {
            return Step::End(EndReason::HubClosed);
        }
        match self
            .window
            .read(self.floor, target, &self.types, Self::REPLAY_PAGE)
        {
            WindowRead::Page { events, settled } => self.window_page(events, settled).await,
            WindowRead::Evicted => self.kafka_page(target).await,
        }
    }

    /// Settles the floor and keeps the events before it awaits anything, so a dropped step loses
    /// nothing; a page without an event yields.
    async fn window_page(&mut self, events: Vec<Arc<StoredEvent>>, settled: Cursor) -> Step {
        self.floor = settled;
        let empty = events.is_empty();
        self.pending.extend(events);
        if empty {
            tokio::task::yield_now().await;
        }
        Step::Idle
    }

    /// Reads one Kafka page from the floor under a permit, granted in arrival order and held to
    /// the end of the read.
    async fn kafka_page(&mut self, target: Cursor) -> Step {
        let permit = Arc::clone(&self.permits).acquire_owned().await;
        if permit.is_err() || self.subscriber.hub_closed() {
            return Step::End(EndReason::HubClosed);
        }
        let answer = self.replay.page(self.floor).await;
        drop(permit);
        match answer {
            Ok(batch) => self.take_batch(batch, target),
            Err(TopicError::BelowEarliest { earliest }) => self.skip_to(earliest),
            Err(TopicError::Unavailable(_)) => Step::End(EndReason::TopicFailed),
        }
    }

    /// Keeps the wanted events of the page and moves the floor to the end of the page, never
    /// back; three pages in a row without an event kept and without progress end the replay.
    fn take_batch(&mut self, batch: Batch, target: Cursor) -> Step {
        let end = page_end(&batch).min(target).max(self.floor);
        let kept = self.keep(batch.records, target);
        if end == self.floor && kept.is_empty() {
            self.stalls = self.stalls.saturating_add(1);
            return if self.stalls < Self::MAX_STALLS {
                Step::Idle
            } else {
                Step::End(EndReason::TopicFailed)
            };
        }
        self.stalls = 0;
        self.floor = end;
        self.pending = kept;
        Step::Idle
    }

    /// The events of the first [`Self::REPLAY_PAGE`] rows of records that are above the floor,
    /// at or below the target, parse and are wanted. An invalid record is skipped without a log
    /// entry or a count: the tail counted it when it passed it.
    fn keep(&self, records: Vec<TopicRecord>, target: Cursor) -> VecDeque<Arc<StoredEvent>> {
        records
            .into_iter()
            .take(Self::REPLAY_PAGE.rows)
            .filter(|record| {
                let id = Cursor::from(record.offset.sequence());
                self.floor < id && id <= target
            })
            .filter_map(|record| {
                parse_event(
                    record.offset,
                    record.value.as_deref(),
                    self.settings.max_data_bytes,
                )
                .ok()
            })
            .filter(|event| wanted(&self.types, &event.event_type, event.sequence))
            .map(Arc::new)
            .collect()
    }

    /// The floor is below the log start: it continues from the log start, or from the window
    /// floor when that is lower (FR-14).
    fn skip_to(&mut self, earliest: Cursor) -> Step {
        self.floor = self.floor.max(earliest.min(self.window.start()));
        self.stalls = 0;
        Step::Idle
    }

    /// The next queued frame: dropped at or below the floor, else it becomes the floor and is
    /// written when wanted. An ended queue re-arms the subscriber at the hub position and starts
    /// the catch-up (FR-24).
    async fn live(&mut self) -> Step {
        match self.subscriber.recv().await {
            Some(frame) => self.take_frame(frame),
            None => self.rearm(),
        }
    }

    fn take_frame(&mut self, frame: Arc<Frame>) -> Step {
        let id = Cursor::from(frame.sequence);
        if id <= self.floor {
            return Step::Idle;
        }
        self.floor = id;
        if wanted(&self.types, &frame.event_type, frame.sequence) {
            Step::Frame(frame)
        } else {
            Step::Idle
        }
    }

    fn rearm(&mut self) -> Step {
        match self.subscriber.resume() {
            Ok(_position) => {
                self.target = None;
                Step::Idle
            }
            Err(_closed) => Step::End(EndReason::HubClosed),
        }
    }
}

/// The position after a page: the offset of the first record beyond the page limit, else the
/// position the batch reports.
fn page_end(batch: &Batch) -> Cursor {
    batch
        .records
        .get(Subscription::REPLAY_PAGE.rows)
        .map_or(batch.next, |left| Cursor::new(left.offset.value()))
}

/// True when the type is held with a since below the id.
fn wanted(types: &BTreeMap<EventType, Cursor>, event_type: &EventType, id: Sequence) -> bool {
    types
        .get(event_type)
        .is_some_and(|since| *since < Cursor::from(id))
}

fn log_replay_capped(from: Cursor, to: Cursor) {
    tracing::info!(from = from.value(), to = to.value(), "replay capped");
}

#[cfg(test)]
mod tests;
