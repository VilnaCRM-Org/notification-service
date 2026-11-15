use std::time::{Duration, SystemTime};

use shared_domain::Clock;

use super::{FixedClock, SettableClock};

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

#[test]
fn a_settable_clock_returns_the_time_set_last_through_every_clone() {
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let later = start + Duration::from_secs(11);
    let clock = SettableClock::at(start);
    let handle: &dyn Clock = &clock.clone();

    let before = handle.now();
    clock.set(later);

    assert_eq!(before, start);
    assert_eq!(handle.now(), later);
    assert_eq!(clock.now(), later);
    assert!(format!("{clock:?}").starts_with("SettableClock"));
}
