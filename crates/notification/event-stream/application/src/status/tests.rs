use std::sync::Arc;

use event_stream_domain::{Cursor, Sequence};
use rstest::rstest;
use test_support::fixture::stored_event;
use tokio::sync::watch;

use super::{NodeState, NodeStatus, StartReason, StatusSource, SubscriberCount};
use crate::tail::{TailState, TopicFault};
use crate::window::Window;

#[derive(Debug, Clone, Copy)]
struct TestHub {
    closed: bool,
    count: usize,
    replay_capped: u64,
}

impl SubscriberCount for TestHub {
    fn closed(&self) -> bool {
        self.closed
    }

    fn count(&self) -> usize {
        self.count
    }

    fn replay_capped(&self) -> u64 {
        self.replay_capped
    }
}

const OPEN: TestHub = TestHub {
    closed: false,
    count: 3,
    replay_capped: 4,
};

const CLOSED: TestHub = TestHub {
    closed: true,
    ..OPEN
};

fn snapshot_of(state: TailState, window: Arc<Window>, hub: TestHub) -> NodeStatus {
    let (_sender, receiver) = watch::channel(state);
    StatusSource::new(receiver, window, Arc::new(hub)).snapshot()
}

fn snapshot(state: TailState, hub: TestHub) -> NodeStatus {
    snapshot_of(state, Arc::new(Window::new(10, usize::MAX)), hub)
}

#[rstest]
#[case::catching_up(None, StartReason::CatchingUp)]
#[case::unreachable(Some(TopicFault::Unreachable), StartReason::BrokerUnreachable)]
#[case::missing(Some(TopicFault::Missing), StartReason::TopicNotFound)]
#[case::partitions(Some(TopicFault::Partitions(3)), StartReason::PartitionCount)]
fn a_node_not_started_is_starting_with_its_fault_as_reason(
    #[case] fault: Option<TopicFault>,
    #[case] reason: StartReason,
) {
    let status = snapshot(
        TailState {
            fault,
            ..TailState::default()
        },
        OPEN,
    );

    assert_eq!(status.state, NodeState::Starting);
    assert_eq!(status.reason, Some(reason));
}

#[rstest]
#[case::no_fault(None)]
#[case::with_a_later_fault(Some(TopicFault::Unreachable))]
fn a_started_node_is_ready_without_a_reason(#[case] fault: Option<TopicFault>) {
    let status = snapshot(
        TailState {
            fault,
            started: true,
            ..TailState::default()
        },
        OPEN,
    );

    assert_eq!(status.state, NodeState::Ready);
    assert_eq!(status.reason, None);
}

#[rstest]
#[case::starting(false, Some(TopicFault::Missing))]
#[case::started(true, None)]
fn a_closed_hub_is_shutting_down_whatever_the_tail_state(
    #[case] started: bool,
    #[case] fault: Option<TopicFault>,
) {
    let status = snapshot(
        TailState {
            fault,
            started,
            ..TailState::default()
        },
        CLOSED,
    );

    assert_eq!(status.state, NodeState::ShuttingDown);
    assert_eq!(status.reason, None);
}

#[rstest]
#[case::below_the_end(Some(90), Some(100), Some(10))]
#[case::at_the_end(Some(100), Some(100), Some(0))]
#[case::above_the_end(Some(110), Some(100), Some(0))]
#[case::position_unknown(None, Some(100), None)]
#[case::end_unknown(Some(90), None, None)]
#[case::both_unknown(None, None, None)]
fn the_lag_is_the_distance_from_the_position_to_the_end(
    #[case] position: Option<u64>,
    #[case] end: Option<u64>,
    #[case] lag: Option<u64>,
) {
    let status = snapshot(
        TailState {
            position: position.map(Cursor::new),
            end: end.map(Cursor::new),
            ..TailState::default()
        },
        OPEN,
    );

    assert_eq!(status.lag, lag);
    assert_eq!(status.position, position.map(Cursor::new));
    assert_eq!(status.end_offset, end.map(Cursor::new));
}

#[test]
fn the_window_floor_is_reported_only_once_the_position_is_known() {
    let window = Arc::new(Window::new(10, usize::MAX));
    window.restart(Cursor::new(5));

    let unknown = snapshot_of(TailState::default(), Arc::clone(&window), OPEN);
    let known = snapshot_of(
        TailState {
            position: Some(Cursor::new(7)),
            ..TailState::default()
        },
        Arc::clone(&window),
        OPEN,
    );

    assert_eq!(unknown.window_first, None);
    assert_eq!(known.window_first, Some(Cursor::new(5)));
}

#[test]
fn the_newest_window_event_is_reported() {
    let window = Arc::new(Window::new(10, usize::MAX));

    let empty = snapshot_of(TailState::default(), Arc::clone(&window), OPEN);
    window.push(Arc::new(stored_event(6, "order.paid", "{}")));
    window.push(Arc::new(stored_event(9, "order.paid", "{}")));
    let filled = snapshot_of(TailState::default(), Arc::clone(&window), OPEN);

    assert_eq!(empty.window_last, None);
    assert_eq!(filled.window_last, Some(Sequence::new(9).unwrap()));
}

#[test]
fn every_counter_and_flag_is_copied() {
    let status = snapshot(
        TailState {
            broker_reachable: true,
            rejected: 11,
            retention_losses: 12,
            observations: 13,
            ..TailState::default()
        },
        TestHub {
            closed: false,
            count: 21,
            replay_capped: 22,
        },
    );

    assert_eq!(
        status,
        NodeStatus {
            state: NodeState::Starting,
            reason: Some(StartReason::CatchingUp),
            broker_reachable: true,
            position: None,
            end_offset: None,
            lag: None,
            window_first: None,
            window_last: None,
            subscribers: 21,
            rejected_records: 11,
            retention_losses: 12,
            replay_capped: 22,
        }
    );
}

#[test]
fn an_unreachable_broker_is_reported() {
    let status = snapshot(TailState::default(), OPEN);

    assert!(!status.broker_reachable);
    assert_eq!(status.subscribers, 3);
    assert_eq!(status.replay_capped, 4);
}

#[test]
fn every_snapshot_reads_the_latest_tail_state() {
    let (sender, receiver) = watch::channel(TailState::default());
    let window = Arc::new(Window::new(10, usize::MAX));
    let source = StatusSource::new(receiver, window, Arc::new(OPEN));
    let copy = source.clone();

    let before = source.snapshot();
    sender.send_replace(TailState {
        started: true,
        position: Some(Cursor::new(3)),
        ..TailState::default()
    });
    let after = copy.snapshot();

    assert_eq!(before.state, NodeState::Starting);
    assert_eq!(before.position, None);
    assert_eq!(after.state, NodeState::Ready);
    assert_eq!(after.position, Some(Cursor::new(3)));
}
