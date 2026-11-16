//! The limits of the event stream: every integer and duration at both sides of its range.

use rstest::rstest;

use super::*;

type Field = fn(&mut StreamArgs, &str);

const DATA: Field = |a, v| a.max_data_bytes = some(v);
const WINDOW_EVENTS: Field = |a, v| a.window_max_events = some(v);
const WINDOW_BYTES: Field = |a, v| a.window_max_bytes = some(v);
const START: Field = |a, v| a.window_start_events = some(v);
const REPLAY: Field = |a, v| a.replay_max_events = some(v);
const PERMITS: Field = |a, v| a.replay_permits = some(v);
const BUFFER: Field = |a, v| a.subscriber_buffer = some(v);
const SUBSCRIBERS: Field = |a, v| a.max_subscribers = some(v);
const STALE: Field = |a, v| a.tail_stale_after = some(v);
const KEEP_ALIVE: Field = |a, v| a.keep_alive = some(v);

fn with_streams(field: Field, raw: &str) -> ServeArgs {
    let mut args = valid();
    field(&mut args.streams, raw);
    args
}

fn wide(value: usize) -> u128 {
    u128::try_from(value).unwrap()
}

/// Reads the value a field was parsed into, as a number of units (bytes, events or milliseconds).
type Read = fn(&StreamConfig) -> u128;

#[rstest]
#[case::data_low(DATA, "1", |s: &StreamConfig| wide(s.max_data_bytes), 1)]
#[case::data_high(DATA, "1048576", |s: &StreamConfig| wide(s.max_data_bytes), 1_048_576)]
#[case::window_events_low(WINDOW_EVENTS, "1", |s: &StreamConfig| wide(s.window_max_events), 1)]
#[case::window_events_high(WINDOW_EVENTS, "10000000", |s: &StreamConfig| wide(s.window_max_events), 10_000_000)]
#[case::window_bytes_low(WINDOW_BYTES, "2097152", |s: &StreamConfig| wide(s.window_max_bytes), 2_097_152)]
#[case::window_bytes_high(WINDOW_BYTES, "17179869184", |s: &StreamConfig| wide(s.window_max_bytes), 17_179_869_184)]
#[case::start_low(START, "0", |s: &StreamConfig| u128::from(s.start_events), 0)]
#[case::start_high(START, "10000000", |s: &StreamConfig| u128::from(s.start_events), 10_000_000)]
#[case::replay_low(REPLAY, "1", |s: &StreamConfig| u128::from(s.replay_max_events), 1)]
#[case::replay_high(REPLAY, "100000000", |s: &StreamConfig| u128::from(s.replay_max_events), 100_000_000)]
#[case::permits_low(PERMITS, "1", |s: &StreamConfig| wide(s.replay_permits), 1)]
#[case::permits_high(PERMITS, "64", |s: &StreamConfig| wide(s.replay_permits), 64)]
#[case::buffer_low(BUFFER, "1", |s: &StreamConfig| wide(s.buffer), 1)]
#[case::buffer_high(BUFFER, "65536", |s: &StreamConfig| wide(s.buffer), 65_536)]
#[case::subscribers_low(SUBSCRIBERS, "1", |s: &StreamConfig| wide(s.max_subscribers), 1)]
#[case::subscribers_high(SUBSCRIBERS, "1000000", |s: &StreamConfig| wide(s.max_subscribers), 1_000_000)]
#[case::stale_low(STALE, "1s", |s: &StreamConfig| s.stale_after.as_millis(), 1_000)]
#[case::stale_high(STALE, "5m", |s: &StreamConfig| s.stale_after.as_millis(), 300_000)]
#[case::keep_alive_low(KEEP_ALIVE, "1s", |s: &StreamConfig| s.keep_alive.as_millis(), 1_000)]
#[case::keep_alive_high(KEEP_ALIVE, "5m", |s: &StreamConfig| s.keep_alive.as_millis(), 300_000)]
fn each_limit_accepts_its_bounds(
    #[case] field: Field,
    #[case] raw: &str,
    #[case] read: Read,
    #[case] expected: u128,
) {
    let config = Config::from_args(with_streams(field, raw)).unwrap();

    assert_eq!(read(&config.streams), expected, "{raw}");
}

#[rstest]
#[case::data_low(DATA, "0", "EVENT_MAX_DATA_BYTES", "1 to 1048576")]
#[case::data_high(DATA, "1048577", "EVENT_MAX_DATA_BYTES", "1 to 1048576")]
#[case::window_events_low(WINDOW_EVENTS, "0", "EVENT_WINDOW_MAX_EVENTS", "1 to 10000000")]
#[case::window_events_high(WINDOW_EVENTS, "10000001", "EVENT_WINDOW_MAX_EVENTS", "1 to 10000000")]
#[case::window_bytes_low(
    WINDOW_BYTES,
    "2097151",
    "EVENT_WINDOW_MAX_BYTES",
    "2097152 to 17179869184"
)]
#[case::window_bytes_high(
    WINDOW_BYTES,
    "17179869185",
    "EVENT_WINDOW_MAX_BYTES",
    "2097152 to 17179869184"
)]
#[case::start_high(START, "10000001", "EVENT_WINDOW_START_EVENTS", "0 to 10000000")]
#[case::start_negative(START, "-1", "EVENT_WINDOW_START_EVENTS", "0 to 10000000")]
#[case::replay_low(REPLAY, "0", "EVENT_REPLAY_MAX_EVENTS", "1 to 100000000")]
#[case::replay_high(REPLAY, "100000001", "EVENT_REPLAY_MAX_EVENTS", "1 to 100000000")]
#[case::permits_low(PERMITS, "0", "EVENT_REPLAY_PERMITS", "1 to 64")]
#[case::permits_high(PERMITS, "65", "EVENT_REPLAY_PERMITS", "1 to 64")]
#[case::buffer_low(BUFFER, "0", "SUBSCRIBER_BUFFER", "1 to 65536")]
#[case::buffer_high(BUFFER, "65537", "SUBSCRIBER_BUFFER", "1 to 65536")]
#[case::subscribers_low(SUBSCRIBERS, "0", "MAX_SUBSCRIBERS", "1 to 1000000")]
#[case::subscribers_high(SUBSCRIBERS, "1000001", "MAX_SUBSCRIBERS", "1 to 1000000")]
#[case::subscribers_text(SUBSCRIBERS, "many", "MAX_SUBSCRIBERS", "1 to 1000000")]
fn each_integer_limit_rejects_a_value_outside_its_range(
    #[case] field: Field,
    #[case] raw: &str,
    #[case] variable: &str,
    #[case] range: &str,
) {
    let reported = problems(with_streams(field, raw));

    assert_eq!(
        reported,
        [(variable, format!("must be an integer from {range}"))]
    );
}

#[rstest]
#[case::stale_low(STALE, "999ms", "EVENT_TAIL_STALE_AFTER")]
#[case::stale_high(STALE, "301s", "EVENT_TAIL_STALE_AFTER")]
#[case::keep_alive_low(KEEP_ALIVE, "999ms", "STREAM_KEEP_ALIVE")]
#[case::keep_alive_high(KEEP_ALIVE, "301s", "STREAM_KEEP_ALIVE")]
#[case::keep_alive_text(KEEP_ALIVE, "often", "STREAM_KEEP_ALIVE")]
fn each_duration_limit_rejects_a_value_outside_its_range(
    #[case] field: Field,
    #[case] raw: &str,
    #[case] variable: &str,
) {
    let reported = problems(with_streams(field, raw));

    assert_eq!(
        reported,
        [(variable, "must be a duration from 1s to 5m".to_owned())]
    );
}

#[test]
fn the_limits_are_reported_in_the_order_of_the_table() {
    let mut args = ServeArgs {
        event_types: some(""),
        cors_allowed_origins: some("x"),
        ..valid()
    };
    args.kafka.retry_delay = some("x");
    args.streams = StreamArgs {
        max_data_bytes: some("0"),
        window_max_events: some("0"),
        window_max_bytes: some("0"),
        window_start_events: some("x"),
        replay_max_events: some("0"),
        replay_permits: some("0"),
        tail_stale_after: some("0s"),
        subscriber_buffer: some("0"),
        max_subscribers: some("0"),
        keep_alive: some("0s"),
    };

    assert_eq!(
        variables(args),
        [
            "KAFKA_RETRY_DELAY",
            "EVENT_TYPES",
            "EVENT_MAX_DATA_BYTES",
            "EVENT_WINDOW_MAX_EVENTS",
            "EVENT_WINDOW_MAX_BYTES",
            "EVENT_WINDOW_START_EVENTS",
            "EVENT_REPLAY_MAX_EVENTS",
            "EVENT_REPLAY_PERMITS",
            "EVENT_TAIL_STALE_AFTER",
            "SUBSCRIBER_BUFFER",
            "MAX_SUBSCRIBERS",
            "STREAM_KEEP_ALIVE",
            "CORS_ALLOWED_ORIGINS",
        ]
    );
}

#[test]
fn the_kafka_variables_are_reported_after_the_limits_of_the_template() {
    let mut args = ServeArgs {
        shutdown_grace_period: some("later"),
        ..valid()
    };
    args.kafka.bootstrap_servers = some("");
    args.kafka.fetch_max_wait = some("0s");

    assert_eq!(
        variables(args),
        [
            "SHUTDOWN_GRACE_PERIOD",
            "KAFKA_BOOTSTRAP_SERVERS",
            "KAFKA_FETCH_MAX_WAIT"
        ]
    );
}
