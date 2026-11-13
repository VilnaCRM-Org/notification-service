use std::time::{Duration, SystemTime};

use shared_domain::Clock;

use super::FixedClock;

#[test]
fn always_returns_the_time_it_was_fixed_at() {
    let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let clock: &dyn Clock = &FixedClock::at(time);

    assert_eq!(clock.now(), time);
    assert_eq!(clock.now(), time);
}

#[test]
fn is_debug_and_copy() {
    let clock = FixedClock::at(SystemTime::UNIX_EPOCH);
    let copy = clock;

    assert_eq!(format!("{clock:?}"), format!("{copy:?}"));
}
