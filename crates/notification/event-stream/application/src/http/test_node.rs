//! A started node for the unit tests of the subscription, the session and the pump: the hub, the
//! window, the sender of the tail state and the subscriptions, wired as the composition root
//! wires them.

use std::collections::BTreeSet;
use std::sync::Arc;

use event_stream_domain::{Cursor, EventType, TopicReplay};
use test_support::fixture::stored_event;
use test_support::static_topic::StaticTopic;
use tokio::sync::watch;

use crate::http::hub::{Hub, HubSettings};
use crate::http::session::Session;
use crate::http::subscription::{ReplaySettings, Subscription, SubscriptionRequest, Subscriptions};
use crate::tail::{EventPublisher as _, TailState};
use crate::window::Window;

/// The replay bounds of the tests: a cap no test reaches unless it lowers it, four permits.
pub(crate) const REPLAY: ReplaySettings = ReplaySettings {
    max_events: 1_000_000,
    max_data_bytes: 65_536,
    permits: 4,
};

/// The hub position and the window floor of the node of [`node_with`].
pub(crate) const POSITION: u64 = 1_000;

/// The hub of the node of [`node_with`]: eight subscribers, a queue of 64 frames.
const CONNECTION_HUB: HubSettings = HubSettings {
    max_subscribers: 8,
    buffer: 64,
};

/// The window of the node of [`node_with`], in events.
const CONNECTION_WINDOW: usize = 100;

/// The event types of `names`, which must be valid.
pub(crate) fn names(names: &[&str]) -> BTreeSet<EventType> {
    names
        .iter()
        .map(|name| EventType::new((*name).to_owned()).unwrap())
        .collect()
}

/// The data of the event `id`, as the envelope parser writes it.
pub(crate) fn event_json(id: u64) -> String {
    format!(r#"{{"n":{id}}}"#)
}

/// A started node: the hub, the window, the sender of the tail state and the subscriptions.
pub(crate) struct Node {
    pub(crate) hub: Arc<Hub>,
    pub(crate) window: Arc<Window>,
    pub(crate) tail: watch::Sender<TailState>,
    pub(crate) subscriptions: Subscriptions,
}

/// The node of the session and pump tests: hub position and window floor [`POSITION`], a window
/// of 100 events, eight subscribers with a queue of 64 frames, replays that read `replay`.
pub(crate) fn node_with(replay: Arc<dyn TopicReplay>) -> Node {
    Node::started(POSITION, replay, REPLAY, CONNECTION_HUB, CONNECTION_WINDOW)
}

/// The node of [`node_with`] whose replays read an empty topic.
pub(crate) fn node() -> Node {
    node_with(Arc::new(StaticTopic::empty()))
}

impl Node {
    /// A node whose hub position and window floor are `floor`, with a window of `window_events`
    /// events and unbounded bytes.
    pub(crate) fn started(
        floor: u64,
        replay: Arc<dyn TopicReplay>,
        settings: ReplaySettings,
        hub: HubSettings,
        window_events: usize,
    ) -> Self {
        let hub = Hub::new(hub);
        hub.start(Cursor::new(floor));
        let window = Arc::new(Window::new(window_events, usize::MAX));
        window.restart(Cursor::new(floor));
        let (tail, receiver) = watch::channel(TailState::default());
        let subscriptions = Subscriptions::new(
            Arc::clone(&hub),
            Arc::clone(&window),
            replay,
            receiver,
            settings,
        );
        Self {
            hub,
            window,
            tail,
            subscriptions,
        }
    }

    /// Opens the subscription of `types` resuming after `cursor`; the hub must accept it.
    pub(crate) fn open(&self, types: &[&str], cursor: Option<u64>) -> Subscription {
        self.subscriptions
            .open(SubscriptionRequest {
                types: names(types),
                cursor: cursor.map(Cursor::new),
            })
            .unwrap()
    }

    /// The session of the subscription [`Node::open`] opens.
    pub(crate) fn session(&self, types: &[&str], cursor: Option<u64>) -> Session {
        Session::new(self.open(types, cursor))
    }

    /// What the tail does with a valid event: the window first, then the hub.
    pub(crate) fn pass(&self, id: u64, event_type: &str) {
        let event = stored_event(id, event_type, &event_json(id));
        self.window.push(Arc::new(event.clone()));
        self.hub.publish(&event);
    }
}
