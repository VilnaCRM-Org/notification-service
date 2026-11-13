use tracing::Level;

use super::LogRecorder;

#[test]
fn records_level_message_and_fields_of_an_event() {
    let (recorder, _guard) = LogRecorder::install();
    let id = 42;

    tracing::error!(event_id = %id, "event publication failed");

    let lines = recorder.lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].level, Level::ERROR);
    assert_eq!(lines[0].message, "event publication failed");
    assert_eq!(lines[0].field("event_id"), Some("42"));
    assert_eq!(lines[0].field("missing"), None);
    assert!(lines[0].target.contains("test_support"));
}

#[test]
fn records_string_and_debug_fields_in_declaration_order() {
    let (recorder, _guard) = LogRecorder::install();

    tracing::warn!(name = "plain", count = 3, flag = ?Some(true), "several");

    let lines = recorder.lines();
    assert_eq!(lines[0].level, Level::WARN);
    assert_eq!(
        lines[0].fields,
        [
            ("name".to_owned(), "plain".to_owned()),
            ("count".to_owned(), "3".to_owned()),
            ("flag".to_owned(), "Some(true)".to_owned()),
        ]
    );
}

#[test]
fn an_event_without_a_message_has_an_empty_message() {
    let (recorder, _guard) = LogRecorder::install();

    tracing::info!(only = 1);

    assert_eq!(recorder.lines()[0].message, "");
}

#[test]
fn events_are_recorded_in_order() {
    let (recorder, _guard) = LogRecorder::install();

    tracing::info!("first");
    tracing::debug!("second");

    let messages: Vec<String> = recorder
        .lines()
        .into_iter()
        .map(|line| line.message)
        .collect();
    assert_eq!(messages, ["first", "second"]);
}

#[test]
fn events_of_other_threads_are_not_recorded() {
    let (recorder, _guard) = LogRecorder::install();

    std::thread::spawn(|| tracing::error!("elsewhere"))
        .join()
        .unwrap();

    assert_eq!(recorder.lines().len(), 0);
}

#[test]
fn spans_and_their_bookkeeping_are_accepted() {
    let (recorder, _guard) = LogRecorder::install();

    let span = tracing::info_span!("work", id = 1);
    let other = tracing::info_span!("more");
    span.record("id", 2);
    other.follows_from(&span);
    {
        let _entered = span.enter();
        tracing::info!("inside");
    }

    assert_eq!(recorder.lines().len(), 1);
}

#[test]
fn nothing_is_recorded_after_the_guard_is_dropped() {
    let (recorder, guard) = LogRecorder::install();
    drop(guard);

    tracing::error!("unseen");

    assert_eq!(recorder.lines().len(), 0);
}
