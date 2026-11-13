//! `service migrate` and `service migrate revert` against a database of its own.

use std::fs::File;
use std::process::Stdio;
use std::time::Duration;

use sqlx::PgPool;
use tokio::time::Instant;

use crate::support::{DEAD_DATABASE_URL, Scratch, database_url, silent_listener, stderr, stdout};

const TABLE_EXISTS: &str = "SELECT to_regclass('example_api_resources') IS NOT NULL";
const VERSION: &str = "20261110000000";
/// What a process start and the output handling may add to a wait that is bounded by its timeout.
const MARGIN: Duration = Duration::from_millis(700);

async fn table_exists(pool: &PgPool) -> bool {
    sqlx::query_scalar(TABLE_EXISTS)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn migrate_applies_once_and_changes_nothing_the_second_time(pool: PgPool) {
    let url = database_url(&pool).await;
    let scratch = Scratch::new();

    let first = scratch.run(&["migrate"], &[("DATABASE_URL", &url)]);
    let table_after_first = table_exists(&pool).await;
    let second = scratch.run(&["migrate"], &[("DATABASE_URL", &url)]);

    assert!(first.status.success());
    assert_eq!(stdout(&first), "applied 1 migration(s)\n");
    assert_eq!(stderr(&first), "");
    assert!(table_after_first);
    assert!(second.status.success());
    assert_eq!(stdout(&second), "applied 0 migration(s)\n");
    assert_eq!(stderr(&second), "");
    assert!(table_exists(&pool).await);
}

#[sqlx::test(migrations = false)]
async fn revert_removes_the_table_and_migrate_creates_it_again(pool: PgPool) {
    let url = database_url(&pool).await;
    let scratch = Scratch::new();
    let variables = [("DATABASE_URL", url.as_str())];
    assert!(scratch.run(&["migrate"], &variables).status.success());

    let first = scratch.run(&["migrate", "revert"], &variables);
    let table_after_first = table_exists(&pool).await;
    let second = scratch.run(&["migrate", "revert"], &variables);
    let again = scratch.run(&["migrate"], &variables);

    assert!(first.status.success());
    assert_eq!(stdout(&first), format!("reverted {VERSION}\n"));
    assert_eq!(stderr(&first), "");
    assert!(!table_after_first);
    assert!(second.status.success());
    assert_eq!(stdout(&second), "nothing to revert\n");
    assert!(again.status.success());
    assert_eq!(stdout(&again), "applied 1 migration(s)\n");
    assert!(table_exists(&pool).await);
}

#[sqlx::test(migrations = false)]
async fn revert_on_a_database_that_was_never_migrated_has_nothing_to_revert(pool: PgPool) {
    let url = database_url(&pool).await;

    let output = Scratch::new().run(&["migrate", "revert"], &[("DATABASE_URL", &url)]);

    assert!(output.status.success());
    assert_eq!(stdout(&output), "nothing to revert\n");
    assert!(!table_exists(&pool).await);
}

#[test]
fn an_unreachable_database_is_retried_once_per_second_and_then_fails() {
    let scratch = Scratch::new();
    let started = Instant::now();

    let output = scratch.run(
        &["migrate"],
        &[
            ("DATABASE_URL", DEAD_DATABASE_URL),
            ("DATABASE_WAIT_TIMEOUT", "2s"),
        ],
    );

    let elapsed = started.elapsed();
    let log = stderr(&output);
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(lines.len(), 3, "{log}");
    assert!(
        lines[0].starts_with("waiting for database (attempt 1): "),
        "{log}"
    );
    assert!(
        lines[1].starts_with("waiting for database (attempt 2): "),
        "{log}"
    );
    assert!(lines[2].starts_with("error: "), "{log}");
    assert!(
        !log.contains("unused"),
        "the password is not printed: {log}"
    );
    assert!(elapsed >= Duration::from_secs(1), "{elapsed:?}");
    assert!(elapsed < Duration::from_secs(2) + MARGIN, "{elapsed:?}");
}

#[test]
fn a_wait_shorter_than_a_second_still_makes_one_attempt() {
    let output = Scratch::new().run(
        &["migrate", "revert"],
        &[
            ("DATABASE_URL", DEAD_DATABASE_URL),
            ("DATABASE_WAIT_TIMEOUT", "100ms"),
        ],
    );

    let log = stderr(&output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(log.lines().count(), 2, "{log}");
    assert!(
        log.starts_with("waiting for database (attempt 1): "),
        "{log}"
    );
}

#[test]
fn a_database_that_accepts_but_never_answers_is_a_failed_attempt_after_the_acquire_timeout() {
    let (log, status, _elapsed) = migrate_against_a_silent_database("1s", "300ms");

    assert_eq!(status, Some(1));
    assert_eq!(log.lines().count(), 2, "{log}");
    assert!(
        log.starts_with("waiting for database (attempt 1): "),
        "{log}"
    );
    assert!(
        log.contains("the connection attempt timed out\nerror: "),
        "{log}"
    );
}

/// Runs `service migrate` against a listener that never answers, and returns the log, the status
/// and the time the whole command took.
fn migrate_against_a_silent_database(wait: &str, acquire: &str) -> (String, Option<i32>, Duration) {
    let listener = silent_listener();
    let url = format!(
        "postgres://app:unused@127.0.0.1:{}/app",
        listener.local_addr().unwrap().port()
    );
    let started = Instant::now();

    let output = Scratch::new().run(
        &["migrate"],
        &[
            ("DATABASE_URL", &url),
            ("DATABASE_WAIT_TIMEOUT", wait),
            ("DATABASE_ACQUIRE_TIMEOUT", acquire),
        ],
    );

    (stderr(&output), output.status.code(), started.elapsed())
}

#[test]
fn an_attempt_is_cut_at_the_time_left_of_the_wait() {
    // An attempt that is not cut would last the whole acquire timeout of 10 s, far past the bound.
    let (log, status, elapsed) = migrate_against_a_silent_database("1500ms", "10s");

    assert_eq!(status, Some(1));
    assert_eq!(log.lines().count(), 2, "{log}");
    assert!(
        log.starts_with("waiting for database (attempt 1): "),
        "{log}"
    );
    assert!(elapsed >= Duration::from_millis(1500), "{elapsed:?}");
    assert!(
        elapsed < Duration::from_millis(1500) + MARGIN,
        "the attempt ran past the timeout: {elapsed:?}"
    );
}

#[test]
fn the_whole_wait_ends_within_the_timeout_and_no_attempt_starts_after_it() {
    let (log, status, elapsed) = migrate_against_a_silent_database("2500ms", "2s");

    assert_eq!(status, Some(1));
    assert_eq!(log.lines().count(), 2, "{log}");
    assert!(log.contains("timed out\nerror: "), "{log}");
    assert!(elapsed >= Duration::from_millis(2500), "{elapsed:?}");
    assert!(
        elapsed < Duration::from_millis(2500) + MARGIN,
        "the wait ran past the timeout: {elapsed:?}"
    );
}

#[test]
fn a_silent_database_is_attempted_again_while_time_is_left() {
    let (log, status, elapsed) = migrate_against_a_silent_database("3s", "1s");

    assert_eq!(status, Some(1));
    assert_eq!(log.lines().count(), 3, "{log}");
    assert!(log.contains("(attempt 2): "), "{log}");
    assert!(log.contains("timed out\nerror: "), "{log}");
    assert!(
        elapsed < Duration::from_secs(3) + MARGIN,
        "the wait ran past the timeout: {elapsed:?}"
    );
}

#[test]
fn a_missing_database_url_is_named_and_rejected() {
    let output = Scratch::new().run(&["migrate"], &[]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "DATABASE_URL: is required\n");
}

#[test]
fn an_invalid_wait_timeout_is_named_and_rejected() {
    let output = Scratch::new().run(
        &["migrate", "revert"],
        &[
            ("DATABASE_URL", DEAD_DATABASE_URL),
            ("DATABASE_WAIT_TIMEOUT", "soon"),
        ],
    );

    assert_eq!(output.status.code(), Some(2));
    let log = stderr(&output);
    assert!(log.starts_with("DATABASE_WAIT_TIMEOUT: "), "{log}");
    assert!(!log.contains("soon"), "the value is not echoed: {log}");
}

#[test]
fn a_report_that_cannot_be_written_fails_with_status_1() {
    let scratch = Scratch::new();
    let mut command = scratch.service(&["migrate"], &[]);
    command.stderr(Stdio::from(
        File::options().write(true).open("/dev/full").unwrap(),
    ));

    let status = command.status().unwrap();

    assert_eq!(status.code(), Some(1));
}
