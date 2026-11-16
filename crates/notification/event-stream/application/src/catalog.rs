//! The catalog of subscribable event types, read from `EVENT_TYPES`.

use std::collections::BTreeSet;

use event_stream_domain::EventType;
use shared_domain::PageRequest;

/// The names of `EVENT_TYPES`, distinct, sorted in byte order (the order of `EventType: Ord`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventCatalog(Vec<EventType>);

impl EventCatalog {
    /// A catalog of the given names, in byte order.
    #[must_use]
    pub fn new(types: BTreeSet<EventType>) -> Self {
        Self(types.into_iter().collect())
    }

    /// The page `page` of `items_per_page` names; empty after the last page.
    #[must_use]
    pub fn page(&self, request: PageRequest) -> Vec<EventType> {
        let skip = usize::try_from(request.offset()).unwrap_or(usize::MAX);
        let take = usize::try_from(request.limit()).unwrap_or(usize::MAX);
        self.0.iter().skip(skip).take(take).cloned().collect()
    }
}

#[cfg(test)]
mod tests;
