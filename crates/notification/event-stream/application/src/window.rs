//! The memory window of the most recent valid events of this node (FR-13).

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use event_stream_domain::{Cursor, EventType, PageLimit, Sequence, StoredEvent};

/// The events held and their floor, behind the lock of [`Window`].
#[derive(Debug)]
struct Inner {
    events: VecDeque<Arc<StoredEvent>>,
    bytes: usize,
    start: Cursor,
}

/// The most recent valid events of this node, in id order, bounded by a count and by bytes
/// (FR-13). At least one event is kept, so that a single large event does not empty the window.
///
/// The lock is held for one call and never across an await; a poisoned lock is taken over.
#[derive(Debug)]
pub struct Window {
    inner: RwLock<Inner>,
    max_events: usize,
    max_bytes: usize,
}

/// The answer of [`Window::read`].
#[derive(Debug, PartialEq, Eq)]
pub enum WindowRead {
    /// `after` is below the floor of the window: the events after it are no longer all held.
    Evicted,
    /// The events collected and the cursor up to which the window was settled for the reader.
    Page {
        /// The collected events, in id order.
        events: Vec<Arc<StoredEvent>>,
        /// Every wanted event with an id in `(after, settled]` is in `events`.
        settled: Cursor,
    },
}

impl Window {
    /// The largest number of entries one [`Window::read`] examines.
    pub const SCAN: usize = 10_000;

    /// An empty window with the floor [`Cursor::START`].
    #[must_use]
    pub const fn new(max_events: usize, max_bytes: usize) -> Self {
        Self {
            inner: RwLock::new(Inner {
                events: VecDeque::new(),
                bytes: 0,
                start: Cursor::START,
            }),
            max_events,
            max_bytes,
        }
    }

    /// Clears the window and sets its floor (the start position of the tail, FR-20).
    pub fn restart(&self, start: Cursor) {
        let mut inner = self.exclusive();
        inner.events.clear();
        inner.bytes = 0;
        inner.start = start;
    }

    /// Appends the event, then pops the oldest events while more than one is held and either bound
    /// is exceeded; each pop moves the floor to the popped event's sequence.
    pub fn push(&self, event: Arc<StoredEvent>) {
        let mut inner = self.exclusive();
        inner.bytes = inner.bytes.saturating_add(event.cost());
        inner.events.push_back(event);
        while inner.events.len() > 1 && self.exceeded(&inner) {
            if let Some(popped) = inner.events.pop_front() {
                inner.bytes = inner.bytes.saturating_sub(popped.cost());
                inner.start = Cursor::from(popped.sequence);
            }
        }
        drop(inner);
    }

    /// The floor F: every valid event with an id in (F, tail position] is held.
    #[must_use]
    pub fn start(&self) -> Cursor {
        self.shared().start
    }

    /// The id of the newest event held, `None` when empty.
    #[must_use]
    pub fn last(&self) -> Option<Sequence> {
        self.shared().events.back().map(|event| event.sequence)
    }

    /// The summed cost of the events held.
    #[must_use]
    pub fn bytes(&self) -> usize {
        self.shared().bytes
    }

    /// Collects, in id order, the events with an id in `(after, up_to]` whose type is in `types`
    /// with a since below the id, until `limit` is reached or [`Self::SCAN`] entries were
    /// examined.
    #[must_use]
    pub fn read(
        &self,
        after: Cursor,
        up_to: Cursor,
        types: &BTreeMap<EventType, Cursor>,
        limit: PageLimit,
    ) -> WindowRead {
        let inner = self.shared();
        if after < inner.start {
            return WindowRead::Evicted;
        }
        if types.is_empty() {
            return WindowRead::Page {
                events: Vec::new(),
                settled: up_to,
            };
        }
        collect(&inner.events, after, up_to, types, limit)
    }

    fn exceeded(&self, inner: &Inner) -> bool {
        inner.events.len() > self.max_events || inner.bytes > self.max_bytes
    }

    fn shared(&self) -> RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn exclusive(&self) -> RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Examines the held events with an id in `(after, up_to]` in id order; see [`Window::read`].
fn collect(
    held: &VecDeque<Arc<StoredEvent>>,
    after: Cursor,
    up_to: Cursor,
    types: &BTreeMap<EventType, Cursor>,
    limit: PageLimit,
) -> WindowRead {
    let mut events = Vec::new();
    let mut bytes: usize = 0;
    let mut last_examined = after;
    let first = held.partition_point(|event| Cursor::from(event.sequence) <= after);
    let candidates = held
        .range(first..)
        .take_while(|event| Cursor::from(event.sequence) <= up_to);
    for (examined, event) in candidates.enumerate() {
        if examined == Window::SCAN {
            return WindowRead::Page {
                events,
                settled: last_examined,
            };
        }
        let id = Cursor::from(event.sequence);
        last_examined = id;
        if !wanted(types, event, id) {
            continue;
        }
        bytes = bytes.saturating_add(event.json.len());
        events.push(Arc::clone(event));
        if limit.reached(events.len(), bytes) {
            return WindowRead::Page {
                events,
                settled: id,
            };
        }
    }
    WindowRead::Page {
        events,
        settled: up_to,
    }
}

/// True when the event's type is in `types` with a since below the event's id.
fn wanted(types: &BTreeMap<EventType, Cursor>, event: &StoredEvent, id: Cursor) -> bool {
    types
        .get(&event.event_type)
        .is_some_and(|since| *since < id)
}

#[cfg(test)]
mod tests;
