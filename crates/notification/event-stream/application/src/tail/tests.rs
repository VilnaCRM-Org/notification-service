use std::error::Error as _;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use event_stream_domain::{Cursor, StoredEvent};
use test_support::clock::FixedClock;
use test_support::fixture::stored_event;
use test_support::static_topic::StaticTopic;

use super::{EventPublisher, RoundFailed, TailPorts, TailSettings, TailState, TopicFault};

#[derive(Debug, Default)]
struct RecordingPublisher {
    calls: Mutex<Vec<String>>,
}

impl EventPublisher for RecordingPublisher {
    fn started(&self, position: Cursor) {
        self.calls
            .lock()
            .unwrap()
            .push(format!("started {position}"));
    }

    fn publish(&self, event: &StoredEvent) -> usize {
        self.calls
            .lock()
            .unwrap()
            .push(format!("publish {}", event.sequence));
        2
    }

    fn advance(&self, position: Cursor) {
        self.calls
            .lock()
            .unwrap()
            .push(format!("advance {position}"));
    }
}

#[test]
fn the_topic_faults_have_their_texts() {
    assert_eq!(
        TopicFault::Unreachable.to_string(),
        "the topic is unreachable"
    );
    assert_eq!(TopicFault::Missing.to_string(), "the topic does not exist");
    assert_eq!(
        TopicFault::Partitions(3).to_string(),
        "the topic must have exactly one partition"
    );
    assert!(TopicFault::Missing.source().is_none());
}

#[test]
fn a_failed_round_has_its_text() {
    assert_eq!(RoundFailed.to_string(), "the round failed");
}

#[test]
fn the_default_tail_state_knows_nothing_yet() {
    let state = TailState::default();

    assert_eq!(state.fault, None);
    assert!(!state.started);
    assert_eq!(state.position, None);
    assert_eq!(state.earliest, None);
    assert_eq!(state.end, None);
    assert!(!state.broker_reachable);
    assert_eq!(state.observations, 0);
    assert_eq!(state.observed_at, None);
    assert_eq!(state.caught_up_at, None);
    assert_eq!(state.rejected, 0);
    assert_eq!(state.retention_losses, 0);
}

#[test]
fn the_publisher_is_used_through_a_shared_trait_object() {
    let recording = Arc::new(RecordingPublisher::default());
    let publisher: Arc<dyn EventPublisher> = Arc::<RecordingPublisher>::clone(&recording);

    publisher.started(Cursor::new(4));
    let behind = publisher.publish(&stored_event(5, "order.paid", "{}"));
    publisher.advance(Cursor::new(7));

    assert_eq!(behind, 2);
    assert_eq!(
        *recording.calls.lock().unwrap(),
        vec!["started 4", "publish 5", "advance 7"]
    );
}

#[test]
fn the_ports_hold_the_topic_readers_and_the_clock() {
    let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let topic = StaticTopic::empty();
    let ports = TailPorts {
        probe: Arc::new(topic.clone()),
        tail: Box::new(topic),
        clock: Arc::new(FixedClock::at(time)),
    };

    assert_eq!(ports.clock.now(), time);
}

#[test]
fn the_settings_are_copied_values() {
    let settings = TailSettings {
        max_data_bytes: 65_536,
        start_events: 1_000,
    };
    let copy = settings;

    assert_eq!(copy, settings);
    assert_eq!(copy.max_data_bytes, 65_536);
    assert_eq!(copy.start_events, 1_000);
}
