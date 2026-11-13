use std::time::{Duration, SystemTime};

use shared_domain::Clock;
use shared_infrastructure::clock::SystemClock;

/// 2026-01-01T00:00:00Z as seconds since the Unix epoch.
const START_OF_2026: u64 = 1_767_225_600;

#[test]
fn two_readings_do_not_go_backwards() {
    let clock: &dyn Clock = &SystemClock;

    let first = clock.now();
    let second = clock.now();

    assert!(second >= first);
}

#[test]
fn the_reading_lies_after_the_start_of_2026() {
    let after = SystemTime::UNIX_EPOCH + Duration::from_secs(START_OF_2026);

    assert!(SystemClock.now() > after);
}
