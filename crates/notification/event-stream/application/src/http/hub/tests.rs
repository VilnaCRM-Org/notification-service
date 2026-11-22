use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use event_stream_domain::{Cursor, EventType, Sequence, StoredEvent};
use rstest::rstest;
use shared_application::http::{Problem, TYPE_BLANK};
use test_support::fixture::stored_event;
use test_support::log::LogRecorder;
use tokio::time::timeout;
use tracing::Level;

use super::{Delivery, Hub, HubError, HubSettings, Subscriber};
use crate::http::frame::Frame;
use crate::status::SubscriberCount;
use crate::tail::EventPublisher;

/// Bounds every await of these tests (time is paused, so nothing really waits).
const WAIT: Duration = Duration::from_secs(5);

const SETTINGS: HubSettings = HubSettings {
    max_subscribers: 100,
    buffer: 16,
};

fn types(names: &[&str]) -> BTreeSet<EventType> {
    names
        .iter()
        .map(|name| EventType::new((*name).to_owned()).unwrap())
        .collect()
}

fn event(sequence: u64, event_type: &str) -> StoredEvent {
    stored_event(sequence, event_type, r#"{"n":1}"#)
}

fn started(settings: HubSettings, position: u64) -> Arc<Hub> {
    let hub = Hub::new(settings);
    hub.start(Cursor::new(position));
    hub
}

fn register(hub: &Arc<Hub>, names: &[&str]) -> Subscriber {
    hub.register(&types(names)).unwrap()
}

/// The hub position, read as the position a new subscriber gets.
fn position_of(hub: &Arc<Hub>) -> Result<Cursor, HubError> {
    hub.register(&BTreeSet::new())
        .map(|subscriber| subscriber.position())
}

/// The next frame within `WAIT`; panics when none arrives.
async fn next(subscriber: &mut Subscriber) -> Option<Arc<Frame>> {
    timeout(WAIT, subscriber.recv()).await.unwrap()
}

/// True when no frame and no end of queue arrives within `WAIT`.
async fn stays_silent(subscriber: &mut Subscriber) -> bool {
    timeout(WAIT, subscriber.recv()).await.is_err()
}

fn sequence(value: u64) -> Sequence {
    Sequence::new(value).unwrap()
}

fn published_count(hub: &Hub) -> usize {
    *hub.count.borrow()
}

// ----- register --------------------------------------------------------------------------------

#[test]
fn a_new_hub_refuses_a_subscriber_as_not_ready_and_registers_nothing() {
    let hub = Hub::new(SETTINGS);

    let refused = hub.register(&types(&["a"]));

    assert_eq!(refused.unwrap_err(), HubError::NotReady);
    assert_eq!(hub.subscriber_count(), 0);
    assert_eq!(hub.type_count(), 0);
    assert_eq!(published_count(&hub), 0);
}

#[test]
fn a_registered_subscriber_gets_the_hub_position_and_the_count_rises_and_is_published() {
    let hub = started(SETTINGS, 41);

    let first = register(&hub, &["a"]);
    assert_eq!(first.position(), Cursor::new(41));
    assert_eq!(hub.subscriber_count(), 1);
    assert_eq!(published_count(&hub), 1);

    let second = register(&hub, &["b"]);
    assert_eq!(second.position(), Cursor::new(41));
    assert_eq!(hub.subscriber_count(), 2);
    assert_eq!(published_count(&hub), 2);
    assert_eq!(hub.type_count(), 2);
}

#[test]
fn the_subscriber_below_the_cap_is_registered_and_the_one_at_the_cap_is_refused() {
    let hub = started(
        HubSettings {
            max_subscribers: 2,
            buffer: 1,
        },
        0,
    );
    let _first = register(&hub, &["a"]);
    let _second = register(&hub, &["a"]);

    let refused = hub.register(&types(&["a"]));

    assert_eq!(refused.unwrap_err(), HubError::AtCapacity);
    assert_eq!(hub.subscriber_count(), 2);
    assert_eq!(published_count(&hub), 2);
}

#[test]
fn a_closed_hub_refuses_a_subscriber_as_closed() {
    let hub = started(SETTINGS, 0);
    hub.close();

    assert_eq!(hub.register(&types(&["a"])).unwrap_err(), HubError::Closed);
    assert_eq!(hub.subscriber_count(), 0);
}

#[rstest]
#[case::closed_before_not_ready(true, None, 5, HubError::Closed)]
#[case::closed_before_at_capacity(true, Some(Cursor::START), 0, HubError::Closed)]
#[case::closed_before_both(true, None, 0, HubError::Closed)]
#[case::not_ready_before_at_capacity(false, None, 0, HubError::NotReady)]
#[case::at_capacity(false, Some(Cursor::START), 0, HubError::AtCapacity)]
fn the_refusals_apply_in_the_order_closed_not_ready_at_capacity(
    #[case] closed: bool,
    #[case] start: Option<Cursor>,
    #[case] max_subscribers: usize,
    #[case] expected: HubError,
) {
    let hub = Hub::new(HubSettings {
        max_subscribers,
        buffer: 1,
    });
    if let Some(position) = start {
        hub.start(position);
    }
    if closed {
        hub.close();
    }

    assert_eq!(hub.register(&types(&["a"])).unwrap_err(), expected);
}

// ----- problems --------------------------------------------------------------------------------

#[rstest]
#[case::not_ready(HubError::NotReady, "the node is starting")]
#[case::at_capacity(HubError::AtCapacity, "the node is at its subscriber capacity")]
#[case::closed(HubError::Closed, "the node is shutting down")]
fn every_refusal_is_a_503_problem_logged_once_as_subscription_rejected(
    #[case] error: HubError,
    #[case] detail: &str,
) {
    let (recorder, _guard) = LogRecorder::install();

    let problem = Problem::from(error);

    assert_eq!(problem.status, 503);
    assert_eq!(problem.r#type, TYPE_BLANK);
    assert_eq!(problem.title, "Service Unavailable");
    assert_eq!(problem.detail.as_deref(), Some(detail));
    assert_eq!(error.to_string(), detail);
    let lines = recorder.lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].level, Level::WARN);
    assert_eq!(lines[0].message, "subscription rejected");
    assert_eq!(lines[0].field("detail"), Some(detail));
}

// ----- deliver ---------------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn an_event_reaches_only_the_subscribers_of_its_type_as_one_shared_frame() {
    let hub = started(SETTINGS, 0);
    let mut first_of_a = register(&hub, &["a"]);
    let mut second_of_a = register(&hub, &["a", "b"]);
    let mut of_b = register(&hub, &["b"]);
    let mut of_none = register(&hub, &[]);

    let delivery = hub.deliver(&event(5, "a"));

    assert_eq!(
        delivery,
        Some(Delivery {
            delivered: 2,
            behind: 0
        })
    );
    let first = next(&mut first_of_a).await.unwrap();
    let second = next(&mut second_of_a).await.unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.sequence(), sequence(5));
    assert_eq!(
        first.json().as_str(),
        r#"{"id":"5","type":"a","data":{"n":1}}"#
    );
    assert!(stays_silent(&mut of_b).await);
    assert!(stays_silent(&mut of_none).await);
    assert_eq!(position_of(&hub), Ok(Cursor::new(5)));
}

#[tokio::test(start_paused = true)]
async fn an_event_of_a_type_without_subscriber_returns_none_and_still_moves_the_position() {
    let hub = started(SETTINGS, 0);
    let mut of_b = register(&hub, &["b"]);

    assert_eq!(hub.deliver(&event(9, "a")), None);

    assert_eq!(position_of(&hub), Ok(Cursor::new(9)));
    assert!(stays_silent(&mut of_b).await);
}

#[test]
fn an_event_before_start_leaves_the_hub_without_position() {
    let hub = Hub::new(SETTINGS);

    assert_eq!(hub.deliver(&event(9, "a")), None);

    assert_eq!(position_of(&hub), Err(HubError::NotReady));
}

// ----- falling behind --------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_full_queue_makes_the_subscriber_fall_behind_and_resume_rearms_it() {
    let hub = started(
        HubSettings {
            max_subscribers: 10,
            buffer: 2,
        },
        0,
    );
    let mut slow = register(&hub, &["a"]);
    let mut other = register(&hub, &["a"]);

    let one = Some(Delivery {
        delivered: 2,
        behind: 0,
    });
    assert_eq!(hub.deliver(&event(1, "a")), one);
    assert_eq!(next(&mut other).await.unwrap().sequence(), sequence(1));
    assert_eq!(hub.deliver(&event(2, "a")), one);
    assert_eq!(next(&mut other).await.unwrap().sequence(), sequence(2));
    assert_eq!(
        hub.deliver(&event(3, "a")),
        Some(Delivery {
            delivered: 1,
            behind: 1
        })
    );

    assert_eq!(hub.subscriber_count(), 2);
    assert_eq!(published_count(&hub), 2);
    assert_eq!(hub.lock().subscribers[&slow.id].types, types(&["a"]));
    assert_eq!(
        hub.lock().by_type[&types(&["a"]).pop_first().unwrap()].len(),
        1
    );
    assert_eq!(next(&mut slow).await.unwrap().sequence(), sequence(1));
    assert_eq!(next(&mut slow).await.unwrap().sequence(), sequence(2));
    assert!(next(&mut slow).await.is_none());
    assert_eq!(next(&mut other).await.unwrap().sequence(), sequence(3));

    hub.advance(Cursor::new(7));
    assert_eq!(slow.resume(), Ok(Cursor::new(7)));
    assert_eq!(slow.position(), Cursor::new(7));
    assert_eq!(
        hub.lock().by_type[&types(&["a"]).pop_first().unwrap()].len(),
        2
    );

    for id in 8..=9 {
        hub.deliver(&event(id, "a"));
        assert_eq!(next(&mut other).await.unwrap().sequence(), sequence(id));
    }
    assert_eq!(
        hub.deliver(&event(10, "a")),
        Some(Delivery {
            delivered: 1,
            behind: 1
        })
    );
    assert_eq!(next(&mut slow).await.unwrap().sequence(), sequence(8));
    assert_eq!(next(&mut slow).await.unwrap().sequence(), sequence(9));
    assert!(next(&mut slow).await.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_subscriber_behind_changes_only_its_types_until_resume_indexes_exactly_them() {
    let hub = started(
        HubSettings {
            max_subscribers: 10,
            buffer: 1,
        },
        0,
    );
    let mut slow = register(&hub, &["a", "b"]);
    hub.deliver(&event(1, "a"));
    hub.deliver(&event(2, "a"));
    assert_eq!(hub.type_count(), 0);

    let since = slow.subscribe(&types(&["c"]));
    slow.unsubscribe(&types(&["b"]));

    assert_eq!(since, Cursor::new(2));
    assert_eq!(hub.type_count(), 0);
    assert_eq!(hub.lock().subscribers[&slow.id].types, types(&["a", "c"]));
    assert_eq!(next(&mut slow).await.unwrap().sequence(), sequence(1));
    assert!(next(&mut slow).await.is_none());

    assert_eq!(slow.resume(), Ok(Cursor::new(2)));

    assert_eq!(hub.type_count(), 2);
    assert_eq!(hub.deliver(&event(3, "b")), None);
    assert_eq!(hub.deliver(&event(4, "c")).unwrap().delivered, 1);
    assert_eq!(next(&mut slow).await.unwrap().sequence(), sequence(4));
}

#[tokio::test(start_paused = true)]
async fn a_buffer_of_zero_is_used_as_one() {
    let hub = started(
        HubSettings {
            max_subscribers: 10,
            buffer: 0,
        },
        0,
    );
    let mut subscriber = register(&hub, &["a"]);

    assert_eq!(hub.deliver(&event(1, "a")).unwrap().behind, 0);
    assert_eq!(hub.deliver(&event(2, "a")).unwrap().behind, 1);

    assert_eq!(next(&mut subscriber).await.unwrap().sequence(), sequence(1));
    assert!(next(&mut subscriber).await.is_none());
}

// ----- subscribe and unsubscribe ---------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn subscribe_returns_the_hub_position_and_adds_the_types_and_unsubscribe_removes_them() {
    let hub = started(SETTINGS, 3);
    let mut subscriber = register(&hub, &[]);
    let _other = register(&hub, &["b"]);
    assert_eq!(hub.type_count(), 1);
    assert!(stays_silent(&mut subscriber).await);

    hub.advance(Cursor::new(6));
    let since = subscriber.subscribe(&types(&["a", "b"]));

    assert_eq!(since, Cursor::new(6));
    assert_eq!(subscriber.position(), Cursor::new(3));
    assert_eq!(hub.type_count(), 2);
    assert_eq!(hub.deliver(&event(7, "a")).unwrap().delivered, 1);
    assert_eq!(hub.deliver(&event(8, "b")).unwrap().delivered, 2);
    assert_eq!(next(&mut subscriber).await.unwrap().sequence(), sequence(7));
    assert_eq!(next(&mut subscriber).await.unwrap().sequence(), sequence(8));

    subscriber.unsubscribe(&types(&["a", "b"]));

    assert_eq!(hub.type_count(), 1);
    assert_eq!(hub.lock().subscribers[&subscriber.id].types, types(&[]));
    assert_eq!(hub.deliver(&event(9, "a")), None);
    assert_eq!(hub.deliver(&event(10, "b")).unwrap().delivered, 1);
    assert!(stays_silent(&mut subscriber).await);
}

#[test]
fn unsubscribing_a_type_held_only_by_another_subscriber_leaves_it_indexed() {
    let hub = started(SETTINGS, 0);
    let subscriber = register(&hub, &["a"]);
    let _other = register(&hub, &["b"]);

    subscriber.unsubscribe(&types(&["b"]));

    assert_eq!(hub.type_count(), 2);
    assert_eq!(hub.deliver(&event(1, "b")).unwrap().delivered, 1);
}

// ----- position --------------------------------------------------------------------------------

#[test]
fn advance_before_start_changes_nothing() {
    let hub = Hub::new(SETTINGS);

    hub.advance(Cursor::new(5));

    assert_eq!(position_of(&hub), Err(HubError::NotReady));
}

#[rstest]
#[case::lower(5, 10)]
#[case::equal(10, 10)]
#[case::higher(20, 20)]
fn advance_after_start_keeps_the_max(#[case] given: u64, #[case] expected: u64) {
    let hub = started(SETTINGS, 10);

    hub.advance(Cursor::new(given));

    assert_eq!(position_of(&hub), Ok(Cursor::new(expected)));
}

#[test]
fn a_second_start_keeps_the_first_position() {
    let hub = started(SETTINGS, 10);

    hub.start(Cursor::new(20));

    assert_eq!(position_of(&hub), Ok(Cursor::new(10)));
}

// ----- drop and drained ------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn a_dropped_subscriber_frees_its_slot_and_drained_completes_at_zero() {
    let hub = started(
        HubSettings {
            max_subscribers: 2,
            buffer: 1,
        },
        0,
    );
    let first = register(&hub, &["a", "b"]);
    let mut second = register(&hub, &["a"]);
    assert!(timeout(WAIT, hub.drained()).await.is_err());

    drop(first);

    assert_eq!(hub.subscriber_count(), 1);
    assert_eq!(published_count(&hub), 1);
    assert_eq!(hub.type_count(), 1);
    assert!(!hub.lock().subscribers.contains_key(&0));
    assert!(timeout(WAIT, hub.drained()).await.is_err());
    assert_eq!(hub.deliver(&event(1, "a")).unwrap().delivered, 1);
    assert_eq!(next(&mut second).await.unwrap().sequence(), sequence(1));
    let third = register(&hub, &["a"]);

    drop(second);
    drop(third);

    assert_eq!(hub.subscriber_count(), 0);
    assert_eq!(published_count(&hub), 0);
    assert_eq!(hub.type_count(), 0);
    assert!(timeout(WAIT, hub.drained()).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn drained_completes_at_once_without_subscriber() {
    let hub = Hub::new(SETTINGS);

    assert!(timeout(WAIT, hub.drained()).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn try_recv_takes_the_queued_frames_in_order_without_waiting_and_answers_none_when_empty() {
    let hub = started(SETTINGS, 0);
    let mut subscriber = register(&hub, &["a"]);
    assert!(subscriber.try_recv().is_none());

    hub.deliver(&event(1, "a"));
    hub.deliver(&event(2, "a"));

    assert_eq!(subscriber.try_recv().unwrap().sequence(), sequence(1));
    assert_eq!(subscriber.try_recv().unwrap().sequence(), sequence(2));
    assert!(subscriber.try_recv().is_none());
    hub.close();
    assert!(subscriber.try_recv().is_none());
    assert!(next(&mut subscriber).await.is_none());
}

#[test]
fn the_count_and_the_closed_flag_are_read_while_another_thread_holds_the_registry() {
    let hub = started(SETTINGS, 0);
    let _subscriber = register(&hub, &["a"]);
    let registry = hub.lock();

    let (count, closed) = std::thread::scope(|scope| {
        scope
            .spawn(|| (hub.subscriber_count(), hub.is_closed()))
            .join()
            .unwrap()
    });

    drop(registry);
    assert_eq!((count, closed), (1, false));
}

// ----- close -----------------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn close_ends_every_queue_after_its_frames_and_keeps_the_count_until_the_drop() {
    let hub = started(SETTINGS, 0);
    let mut subscriber = register(&hub, &["a"]);
    hub.deliver(&event(1, "a"));
    hub.deliver(&event(2, "a"));
    assert!(!subscriber.hub_closed());
    assert!(!hub.is_closed());

    hub.close();

    assert!(hub.is_closed());
    assert!(subscriber.hub_closed());
    assert_eq!(hub.type_count(), 0);
    assert_eq!(hub.subscriber_count(), 1);
    assert_eq!(published_count(&hub), 1);
    assert_eq!(next(&mut subscriber).await.unwrap().sequence(), sequence(1));
    assert_eq!(next(&mut subscriber).await.unwrap().sequence(), sequence(2));
    assert!(next(&mut subscriber).await.is_none());
    assert_eq!(subscriber.resume(), Err(HubError::Closed));
    assert_eq!(subscriber.position(), Cursor::START);
    assert_eq!(hub.deliver(&event(3, "a")), None);

    assert_eq!(subscriber.subscribe(&types(&["b"])), Cursor::new(3));
    subscriber.unsubscribe(&types(&["a"]));
    assert_eq!(hub.type_count(), 0);
    assert_eq!(hub.lock().subscribers[&subscriber.id].types, types(&["b"]));

    drop(subscriber);

    assert_eq!(hub.subscriber_count(), 0);
    assert_eq!(published_count(&hub), 0);
}

// ----- counters and traits ---------------------------------------------------------------------

#[test]
fn every_noted_capped_replay_raises_the_counter() {
    let hub = started(SETTINGS, 0);
    let subscriber = register(&hub, &["a"]);
    assert_eq!(hub.replay_capped(), 0);

    subscriber.note_replay_capped();
    subscriber.note_replay_capped();

    assert_eq!(hub.replay_capped(), 2);
    assert_eq!(SubscriberCount::replay_capped(hub.as_ref()), 2);
}

#[tokio::test(start_paused = true)]
async fn the_publisher_maps_started_publish_and_advance_to_the_hub() {
    let hub = Hub::new(HubSettings {
        max_subscribers: 10,
        buffer: 1,
    });
    let publisher: Arc<dyn EventPublisher> = Arc::<Hub>::clone(&hub);

    publisher.started(Cursor::new(4));
    assert_eq!(position_of(&hub), Ok(Cursor::new(4)));
    let mut first = register(&hub, &["a"]);
    let _second = register(&hub, &["a"]);

    assert_eq!(publisher.publish(&event(5, "b")), 0);
    assert_eq!(publisher.publish(&event(6, "a")), 0);
    assert_eq!(publisher.publish(&event(7, "a")), 2);
    assert_eq!(next(&mut first).await.unwrap().sequence(), sequence(6));

    publisher.advance(Cursor::new(3));
    assert_eq!(position_of(&hub), Ok(Cursor::new(7)));
    publisher.advance(Cursor::new(12));
    assert_eq!(position_of(&hub), Ok(Cursor::new(12)));
}

#[test]
fn the_subscriber_count_maps_to_closed_count_and_replay_capped() {
    let hub = started(SETTINGS, 0);
    let first = register(&hub, &["a"]);
    let _second = register(&hub, &["b"]);
    first.note_replay_capped();
    first.note_replay_capped();
    first.note_replay_capped();
    let count: Arc<dyn SubscriberCount> = Arc::<Hub>::clone(&hub);

    assert!(!count.closed());
    assert_eq!(count.count(), 2);
    assert_eq!(count.replay_capped(), 3);

    hub.close();

    assert!(count.closed());
    assert_eq!(count.count(), 2);
}

// ----- structure -------------------------------------------------------------------------------

#[tokio::test(start_paused = true)]
async fn one_event_reaches_ten_thousand_queues_of_its_type_as_one_frame_encoded_once() {
    const SUBSCRIBERS: usize = 10_000;
    let hub = started(
        HubSettings {
            max_subscribers: SUBSCRIBERS,
            buffer: 1,
        },
        0,
    );
    let mut subscribers: Vec<Subscriber> =
        (0..SUBSCRIBERS).map(|_| register(&hub, &["a"])).collect();
    assert_eq!(
        hub.register(&types(&["a"])).unwrap_err(),
        HubError::AtCapacity
    );

    let delivery = hub.deliver(&event(1, "a"));

    assert_eq!(
        delivery,
        Some(Delivery {
            delivered: SUBSCRIBERS,
            behind: 0
        })
    );
    let first = next(&mut subscribers[0]).await.unwrap();
    for subscriber in &mut subscribers[1..] {
        let frame = next(subscriber).await.unwrap();
        assert!(Arc::ptr_eq(&first, &frame));
    }
    assert_eq!(Arc::strong_count(&first), 1);
}

#[tokio::test(start_paused = true)]
async fn a_poisoned_registry_is_taken_over() {
    let hub = started(SETTINGS, 0);
    let poisoner = Arc::clone(&hub);
    let outcome = std::thread::spawn(move || {
        let _guard = poisoner.registry.lock().unwrap();
        panic!("poison the registry");
    })
    .join();
    assert!(outcome.is_err());
    assert!(hub.registry.is_poisoned());

    let mut subscriber = register(&hub, &["a"]);
    hub.advance(Cursor::new(2));
    assert_eq!(subscriber.subscribe(&types(&["b"])), Cursor::new(2));
    assert_eq!(hub.deliver(&event(3, "b")).unwrap().delivered, 1);
    assert_eq!(next(&mut subscriber).await.unwrap().sequence(), sequence(3));
    assert_eq!(hub.type_count(), 2);
    assert_eq!(hub.subscriber_count(), 1);
    hub.close();
    assert!(hub.is_closed());
    drop(subscriber);
    assert!(timeout(WAIT, hub.drained()).await.is_ok());
}
