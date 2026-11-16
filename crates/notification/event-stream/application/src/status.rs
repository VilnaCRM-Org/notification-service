//! The status of this node, computed from the state it already holds, without a broker read
//! (FR-4).

use std::fmt::Debug;
use std::sync::Arc;

use event_stream_domain::{Cursor, Sequence};
use tokio::sync::watch;

use crate::tail::{TailState, TopicFault};
use crate::window::Window;

/// The life stage of the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    /// The node does not accept subscribers yet.
    Starting,
    /// The node accepts subscribers.
    Ready,
    /// The node stopped accepting subscribers.
    ShuttingDown,
}

/// Why a starting node is not ready yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartReason {
    /// The broker could not be read.
    BrokerUnreachable,
    /// The topic does not exist.
    TopicNotFound,
    /// The topic does not have exactly one partition.
    PartitionCount,
    /// The tail reads the topic and has not reached its start goal yet.
    CatchingUp,
}

/// The status of this node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeStatus {
    /// The life stage.
    pub state: NodeState,
    /// Why the node is not ready, only while it is starting.
    pub reason: Option<StartReason>,
    /// The last watermark read succeeded.
    pub broker_reachable: bool,
    /// The reader position of the tail; `None` before its first seek.
    pub position: Option<Cursor>,
    /// The end of the topic at the last successful watermark read.
    pub end_offset: Option<Cursor>,
    /// How far the position is below the end; `None` while either is unknown.
    pub lag: Option<u64>,
    /// The floor of the memory window, once the position is known.
    pub window_first: Option<Cursor>,
    /// The id of the newest event in the memory window.
    pub window_last: Option<Sequence>,
    /// The number of local subscribers.
    pub subscribers: usize,
    /// Invalid records the tail skipped.
    pub rejected_records: u64,
    /// Jumps to the log start because retention overtook the tail.
    pub retention_losses: u64,
    /// Replays that reached their cap.
    pub replay_capped: u64,
}

/// What the status reads of the hub, without a broker call (FR-4).
pub trait SubscriberCount: Debug + Send + Sync {
    /// True once the hub stopped accepting subscribers.
    fn closed(&self) -> bool;

    /// The number of local subscribers.
    fn count(&self) -> usize;

    /// The number of replays that reached their cap.
    fn replay_capped(&self) -> u64;
}

/// Computes the status of this node from the tail state, the window and the hub.
#[derive(Debug, Clone)]
pub struct StatusSource {
    tail: watch::Receiver<TailState>,
    window: Arc<Window>,
    hub: Arc<dyn SubscriberCount>,
}

impl StatusSource {
    /// A status source over the state of the tail, the window and the hub.
    #[must_use]
    pub fn new(
        tail: watch::Receiver<TailState>,
        window: Arc<Window>,
        hub: Arc<dyn SubscriberCount>,
    ) -> Self {
        Self { tail, window, hub }
    }

    /// The status of this node now; reads only the state the node holds.
    #[must_use]
    pub fn snapshot(&self) -> NodeStatus {
        let tail = self.tail.borrow().clone();
        let state = node_state(self.hub.as_ref(), tail.started);
        NodeStatus {
            state,
            reason: (state == NodeState::Starting).then(|| start_reason(tail.fault)),
            broker_reachable: tail.broker_reachable,
            position: tail.position,
            end_offset: tail.end,
            lag: tail
                .end
                .zip(tail.position)
                .map(|(end, position)| end.distance_from(position)),
            window_first: tail.position.map(|_| self.window.start()),
            window_last: self.window.last(),
            subscribers: self.hub.count(),
            rejected_records: tail.rejected,
            retention_losses: tail.retention_losses,
            replay_capped: self.hub.replay_capped(),
        }
    }
}

/// `ShuttingDown` when the hub is closed, else `Ready` once started, else `Starting`.
fn node_state(hub: &dyn SubscriberCount, started: bool) -> NodeState {
    if hub.closed() {
        NodeState::ShuttingDown
    } else if started {
        NodeState::Ready
    } else {
        NodeState::Starting
    }
}

/// The fault mapped one to one, else `CatchingUp`.
const fn start_reason(fault: Option<TopicFault>) -> StartReason {
    match fault {
        Some(TopicFault::Unreachable) => StartReason::BrokerUnreachable,
        Some(TopicFault::Missing) => StartReason::TopicNotFound,
        Some(TopicFault::Partitions(_)) => StartReason::PartitionCount,
        None => StartReason::CatchingUp,
    }
}

#[cfg(test)]
mod tests;
