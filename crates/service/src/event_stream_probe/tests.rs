use std::sync::Arc;
use std::time::{Duration, SystemTime};

use event_stream_application::{TailState, TopicFault};
use health_check_domain::{Dependency, HealthProbe as _};
use rstest::rstest;
use shared_domain::error_chain;
use test_support::clock::FixedClock;
use tokio::sync::watch;

use super::{EventStreamNotReady, EventStreamProbe};

/// `EVENT_TAIL_STALE_AFTER` of the tests.
const STALE_AFTER: Duration = Duration::from_secs(10);

/// The time of the clock of the tests.
fn now() -> SystemTime {
    SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(1_700_000_000))
        .unwrap()
}

/// `age` before [`now`].
fn ago(age: Duration) -> Option<SystemTime> {
    now().checked_sub(age)
}

/// A started node whose tail observed the topic and was at its end `age` ago.
fn started(age: Duration) -> TailState {
    TailState {
        started: true,
        observed_at: ago(age),
        caught_up_at: ago(age),
        ..TailState::default()
    }
}

fn probe(state: TailState) -> (EventStreamProbe, watch::Sender<TailState>) {
    let (sender, receiver) = watch::channel(state);
    let probe = EventStreamProbe::new(receiver, Arc::new(FixedClock::at(now())), STALE_AFTER);
    (probe, sender)
}

fn just_over() -> Duration {
    STALE_AFTER.checked_add(Duration::from_millis(1)).unwrap()
}

#[test]
fn the_probe_reports_the_event_stream() {
    let (probe, _sender) = probe(TailState::default());

    assert_eq!(probe.dependency(), Dependency::EventStream);
}

#[rstest]
#[case::now(Duration::ZERO)]
#[case::half(Duration::from_secs(5))]
#[case::exactly_the_bound(STALE_AFTER)]
fn a_started_node_with_fresh_times_passes(#[case] age: Duration) {
    let (probe, _sender) = probe(started(age));

    assert_eq!(probe.readiness(), Ok(()));
}

#[tokio::test]
async fn check_passes_for_a_ready_node_and_fails_with_the_reason_otherwise() {
    let (probe, sender) = probe(started(Duration::ZERO));

    let limit = Duration::from_secs(5);
    let passed = tokio::time::timeout(limit, probe.check()).await.unwrap();
    sender.send_replace(TailState::default());
    let failed = tokio::time::timeout(limit, probe.check())
        .await
        .unwrap()
        .unwrap_err();

    assert!(passed.is_ok());
    assert_eq!(failed.to_string(), "the node is starting");
    assert_eq!(error_chain(failed.as_ref()), "the node is starting");
}

#[test]
fn an_observation_older_than_the_bound_is_an_unreachable_topic() {
    let state = TailState {
        observed_at: ago(just_over()),
        ..started(Duration::ZERO)
    };
    let (probe, _sender) = probe(state);

    let reason = probe.readiness().unwrap_err();

    assert_eq!(reason, EventStreamNotReady::Unreachable);
    assert_eq!(reason.to_string(), "the topic is unreachable");
}

#[test]
fn a_catch_up_older_than_the_bound_is_a_tail_behind() {
    let state = TailState {
        caught_up_at: ago(just_over()),
        ..started(Duration::ZERO)
    };
    let (probe, _sender) = probe(state);

    let reason = probe.readiness().unwrap_err();

    assert_eq!(reason, EventStreamNotReady::Behind);
    assert_eq!(reason.to_string(), "the event tail is behind");
}

#[test]
fn a_missing_observation_is_an_unreachable_topic_and_a_missing_catch_up_a_tail_behind() {
    let (unobserved, _first) = probe(TailState {
        observed_at: None,
        ..started(Duration::ZERO)
    });
    let (never_caught_up, _second) = probe(TailState {
        caught_up_at: None,
        ..started(Duration::ZERO)
    });

    assert_eq!(
        unobserved.readiness(),
        Err(EventStreamNotReady::Unreachable)
    );
    assert_eq!(
        never_caught_up.readiness(),
        Err(EventStreamNotReady::Behind)
    );
}

#[test]
fn both_times_stale_report_the_unreachable_topic_first() {
    let (probe, _sender) = probe(started(just_over()));

    assert_eq!(probe.readiness(), Err(EventStreamNotReady::Unreachable));
}

#[test]
fn a_node_that_is_not_started_is_starting_whatever_its_times() {
    let state = TailState {
        started: false,
        ..started(Duration::ZERO)
    };
    let (fresh, _first) = probe(state);
    let (empty, _second) = probe(TailState::default());

    let reason = fresh.readiness().unwrap_err();

    assert_eq!(reason, EventStreamNotReady::Starting);
    assert_eq!(reason.to_string(), "the node is starting");
    assert_eq!(empty.readiness(), Err(EventStreamNotReady::Starting));
}

#[rstest]
#[case::unreachable(TopicFault::Unreachable, "the topic is unreachable")]
#[case::missing(TopicFault::Missing, "the topic does not exist")]
#[case::partitions(TopicFault::Partitions(3), "the topic must have exactly one partition")]
fn a_fault_is_reported_with_its_text_before_every_other_reason(
    #[case] fault: TopicFault,
    #[case] text: &str,
) {
    let (starting, _first) = probe(TailState {
        fault: Some(fault),
        ..TailState::default()
    });
    let (started, _second) = probe(TailState {
        fault: Some(fault),
        ..started(Duration::ZERO)
    });

    let reason = starting.readiness().unwrap_err();

    assert_eq!(reason, EventStreamNotReady::Fault(fault));
    assert_eq!(reason.to_string(), text);
    assert_eq!(error_chain(&reason), text);
    assert_eq!(started.readiness(), Err(EventStreamNotReady::Fault(fault)));
}

#[test]
fn times_later_than_the_clock_count_as_fresh() {
    let later = now().checked_add(Duration::from_secs(3_600));
    let (probe, _sender) = probe(TailState {
        started: true,
        observed_at: later,
        caught_up_at: later,
        ..TailState::default()
    });

    assert_eq!(probe.readiness(), Ok(()));
}

#[test]
fn the_probe_reads_the_latest_published_state() {
    let (probe, sender) = probe(TailState::default());

    let before = probe.readiness();
    sender.send_replace(started(Duration::ZERO));
    let after = probe.readiness();
    sender.send_replace(started(just_over()));
    let stale = probe.readiness();

    assert_eq!(before, Err(EventStreamNotReady::Starting));
    assert_eq!(after, Ok(()));
    assert_eq!(stale, Err(EventStreamNotReady::Unreachable));
}
