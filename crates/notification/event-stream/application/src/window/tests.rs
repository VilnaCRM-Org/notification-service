use std::collections::BTreeMap;
use std::sync::Arc;

use event_stream_domain::{Cursor, EventType, PageLimit, Sequence, StoredEvent};
use test_support::fixture::stored_event;

use super::{Window, WindowRead};

/// A json of `json_bytes` bytes under the one-byte type `t`: the event costs
/// `json_bytes + 1 + 128`.
fn sized(sequence: u64, json_bytes: usize) -> Arc<StoredEvent> {
    Arc::new(stored_event(sequence, "t", &"x".repeat(json_bytes)))
}

fn typed(sequence: u64, event_type: &str) -> Arc<StoredEvent> {
    Arc::new(stored_event(sequence, event_type, "{}"))
}

fn sequence(value: u64) -> Sequence {
    Sequence::new(value).unwrap()
}

fn cursor(value: u64) -> Cursor {
    Cursor::new(value)
}

fn types(entries: &[(&str, u64)]) -> BTreeMap<EventType, Cursor> {
    entries
        .iter()
        .map(|(name, since)| (EventType::new((*name).to_owned()).unwrap(), cursor(*since)))
        .collect()
}

const UNLIMITED: PageLimit = PageLimit {
    rows: usize::MAX,
    bytes: usize::MAX,
};

fn unbounded() -> Window {
    Window::new(usize::MAX, usize::MAX)
}

/// The ids of every event held, read through `read` with an unlimited page.
fn held_ids(window: &Window) -> Vec<u64> {
    match window.read(
        window.start(),
        cursor(u64::MAX),
        &types(&[("t", 0), ("a", 0), ("b", 0)]),
        UNLIMITED,
    ) {
        WindowRead::Page { events, .. } => ids(&events),
        WindowRead::Evicted => panic!("reading from the floor is never evicted"),
    }
}

fn ids(events: &[Arc<StoredEvent>]) -> Vec<u64> {
    events.iter().map(|event| event.sequence.value()).collect()
}

fn page(read: WindowRead) -> (Vec<u64>, Cursor) {
    match read {
        WindowRead::Page { events, settled } => (ids(&events), settled),
        WindowRead::Evicted => panic!("expected a page, got Evicted"),
    }
}

#[test]
fn a_new_window_is_empty_with_the_start_floor() {
    let window = Window::new(3, 1_000);

    assert_eq!(window.start(), Cursor::START);
    assert_eq!(window.last(), None);
    assert_eq!(window.bytes(), 0);
    assert_eq!(held_ids(&window), Vec::<u64>::new());
}

#[test]
fn restart_empties_the_window_and_sets_the_floor() {
    let window = unbounded();
    window.push(sized(1, 10));
    window.push(sized(2, 10));

    window.restart(cursor(41));

    assert_eq!(window.start(), cursor(41));
    assert_eq!(window.last(), None);
    assert_eq!(window.bytes(), 0);
    assert_eq!(held_ids(&window), Vec::<u64>::new());
}

#[test]
fn pushed_events_are_held_in_order_with_their_summed_cost() {
    let window = Window::new(3, 1_000);

    window.push(sized(1, 10));
    window.push(sized(2, 20));
    window.push(sized(3, 30));

    assert_eq!(held_ids(&window), vec![1, 2, 3]);
    assert_eq!(window.start(), Cursor::START);
    assert_eq!(window.last(), Some(sequence(3)));
    assert_eq!(window.bytes(), 139 + 149 + 159);
}

#[test]
fn the_count_bound_pops_the_oldest_event_and_moves_the_floor_to_it() {
    let window = Window::new(3, usize::MAX);
    for id in 1..=3 {
        window.push(sized(id, 10));
    }
    assert_eq!(window.start(), Cursor::START);

    window.push(sized(4, 20));

    assert_eq!(held_ids(&window), vec![2, 3, 4]);
    assert_eq!(window.start(), cursor(1));
    assert_eq!(window.last(), Some(sequence(4)));
    assert_eq!(window.bytes(), 139 + 139 + 149);
}

#[test]
fn the_byte_bound_is_kept_at_exactly_max_bytes_and_popped_one_above() {
    let window = Window::new(usize::MAX, 600);
    for id in 1..=3 {
        window.push(sized(id, 71));
    }
    assert_eq!(window.bytes(), 600);
    assert_eq!(window.start(), Cursor::START);
    assert_eq!(held_ids(&window), vec![1, 2, 3]);

    window.push(sized(4, 72));

    assert_eq!(held_ids(&window), vec![3, 4]);
    assert_eq!(window.start(), cursor(2));
    assert_eq!(window.bytes(), 200 + 201);
}

#[test]
fn one_push_pops_several_events_until_both_bounds_hold() {
    let window = Window::new(usize::MAX, 450);
    for id in 1..=3 {
        window.push(sized(id, 21));
    }
    assert_eq!(window.bytes(), 450);

    window.push(sized(4, 271));

    assert_eq!(held_ids(&window), vec![4]);
    assert_eq!(window.start(), cursor(3));
    assert_eq!(window.bytes(), 400);
}

#[test]
fn one_event_larger_than_max_bytes_is_kept_alone() {
    let window = Window::new(usize::MAX, 100);

    window.push(sized(1, 71));

    assert_eq!(held_ids(&window), vec![1]);
    assert_eq!(window.start(), Cursor::START);
    assert_eq!(window.bytes(), 200);

    window.push(sized(2, 72));

    assert_eq!(held_ids(&window), vec![2]);
    assert_eq!(window.start(), cursor(1));
    assert_eq!(window.last(), Some(sequence(2)));
    assert_eq!(window.bytes(), 201);
}

#[test]
fn a_count_bound_of_zero_still_keeps_one_event() {
    let window = Window::new(0, usize::MAX);

    window.push(sized(1, 10));
    window.push(sized(2, 10));

    assert_eq!(held_ids(&window), vec![2]);
    assert_eq!(window.start(), cursor(1));
    assert_eq!(window.bytes(), 139);
}

#[test]
fn a_read_after_a_cursor_below_the_floor_is_evicted() {
    let window = Window::new(2, usize::MAX);
    for id in 1..=4 {
        window.push(typed(id, "a"));
    }
    assert_eq!(window.start(), cursor(2));

    let below = window.read(cursor(1), cursor(4), &types(&[("a", 0)]), UNLIMITED);
    let at_floor = window.read(cursor(2), cursor(4), &types(&[("a", 0)]), UNLIMITED);

    assert_eq!(below, WindowRead::Evicted);
    assert_eq!(page(at_floor), (vec![3, 4], cursor(4)));
}

#[test]
fn a_restarted_floor_evicts_reads_below_it() {
    let window = unbounded();
    window.restart(cursor(10));

    assert_eq!(
        window.read(cursor(9), cursor(20), &types(&[("a", 0)]), UNLIMITED),
        WindowRead::Evicted
    );
    assert_eq!(
        page(window.read(cursor(10), cursor(20), &types(&[("a", 0)]), UNLIMITED)),
        (vec![], cursor(20))
    );
}

#[test]
fn no_types_gives_an_empty_page_settled_at_up_to() {
    let window = unbounded();
    for id in 1..=3 {
        window.push(typed(id, "a"));
    }

    let read = window.read(Cursor::START, cursor(2), &BTreeMap::new(), UNLIMITED);

    assert_eq!(page(read), (vec![], cursor(2)));
}

#[test]
fn only_events_of_the_wanted_types_in_after_up_to_are_collected() {
    let window = unbounded();
    for (id, name) in [(1, "a"), (2, "b"), (3, "a"), (4, "c"), (5, "a"), (6, "c")] {
        window.push(typed(id, name));
    }

    let read = window.read(
        cursor(1),
        cursor(5),
        &types(&[("a", 0), ("c", 0)]),
        UNLIMITED,
    );

    assert_eq!(page(read), (vec![3, 4, 5], cursor(5)));
}

#[test]
fn an_event_is_collected_only_when_its_type_since_is_below_its_id() {
    let window = unbounded();
    for (id, name) in [(1, "a"), (2, "b"), (3, "a"), (4, "b"), (5, "a")] {
        window.push(typed(id, name));
    }

    let read = window.read(
        Cursor::START,
        cursor(5),
        &types(&[("a", 3), ("b", 1)]),
        UNLIMITED,
    );

    assert_eq!(page(read), (vec![2, 4, 5], cursor(5)));
}

#[test]
fn the_returned_events_are_the_held_ones() {
    let window = unbounded();
    let event = typed(1, "a");
    window.push(Arc::clone(&event));

    match window.read(Cursor::START, cursor(1), &types(&[("a", 0)]), UNLIMITED) {
        WindowRead::Page { events, settled } => {
            assert_eq!(events.len(), 1);
            assert!(Arc::ptr_eq(&events[0], &event));
            assert_eq!(settled, cursor(1));
        }
        WindowRead::Evicted => panic!("expected a page"),
    }
}

#[test]
fn the_row_limit_cuts_at_the_last_collected_id() {
    let window = unbounded();
    for (id, name) in [(1, "a"), (2, "b"), (3, "a"), (4, "b"), (5, "a")] {
        window.push(typed(id, name));
    }
    let wanted = types(&[("a", 0)]);
    let rows = |rows| {
        page(window.read(
            Cursor::START,
            cursor(6),
            &wanted,
            PageLimit {
                rows,
                bytes: usize::MAX,
            },
        ))
    };

    assert_eq!(rows(2), (vec![1, 3], cursor(3)));
    assert_eq!(rows(3), (vec![1, 3, 5], cursor(5)));
    assert_eq!(rows(4), (vec![1, 3, 5], cursor(6)));
}

#[test]
fn the_byte_limit_counts_json_bytes_and_cuts_once_reached() {
    let window = unbounded();
    for id in 1..=4 {
        window.push(sized(id, 10));
    }
    let wanted = types(&[("t", 0)]);
    let limit = |bytes| PageLimit {
        rows: usize::MAX,
        bytes,
    };

    let reached = window.read(Cursor::START, cursor(4), &wanted, limit(20));
    let one_above = window.read(Cursor::START, cursor(4), &wanted, limit(21));

    assert_eq!(page(reached), (vec![1, 2], cursor(2)));
    assert_eq!(page(one_above), (vec![1, 2, 3], cursor(3)));
}

#[test]
fn a_page_of_nothing_wanted_is_settled_at_up_to() {
    let window = unbounded();
    for id in 1..=3 {
        window.push(typed(id, "a"));
    }

    let read = window.read(Cursor::START, cursor(7), &types(&[("b", 0)]), UNLIMITED);

    assert_eq!(page(read), (vec![], cursor(7)));
}

#[test]
fn the_scan_bound_settles_at_the_last_examined_id() {
    let window = unbounded();
    let scan = u64::try_from(Window::SCAN).unwrap();
    for id in 1..=scan + 1 {
        window.push(typed(id, "a"));
    }

    let nothing_wanted = window.read(
        Cursor::START,
        cursor(scan + 1),
        &types(&[("b", 0)]),
        UNLIMITED,
    );
    let (collected, settled) = page(window.read(
        Cursor::START,
        cursor(scan + 1),
        &types(&[("a", 0)]),
        UNLIMITED,
    ));

    assert_eq!(page(nothing_wanted), (vec![], cursor(scan)));
    assert_eq!(collected.len(), Window::SCAN);
    assert_eq!(collected.last(), Some(&scan));
    assert_eq!(settled, cursor(scan));
}

#[test]
fn the_scan_bound_starts_after_the_cursor() {
    let window = unbounded();
    let scan = u64::try_from(Window::SCAN).unwrap();
    for id in 1..=scan + 6 {
        window.push(typed(id, "a"));
    }

    let read = window.read(cursor(5), cursor(scan + 6), &types(&[("b", 0)]), UNLIMITED);

    assert_eq!(page(read), (vec![], cursor(scan + 5)));
}

#[test]
fn exactly_scan_entries_examined_is_not_a_cut() {
    let window = unbounded();
    let scan = u64::try_from(Window::SCAN).unwrap();
    for id in 1..=scan {
        window.push(typed(id, "a"));
    }

    let read = window.read(
        Cursor::START,
        cursor(scan + 50),
        &types(&[("b", 0)]),
        UNLIMITED,
    );

    assert_eq!(page(read), (vec![], cursor(scan + 50)));
}

#[test]
fn entries_above_up_to_do_not_count_against_the_scan_bound() {
    let window = unbounded();
    let scan = u64::try_from(Window::SCAN).unwrap();
    for id in 1..=scan + 1 {
        window.push(typed(id, "a"));
    }

    let read = window.read(Cursor::START, cursor(scan), &types(&[("b", 0)]), UNLIMITED);

    assert_eq!(page(read), (vec![], cursor(scan)));
}

#[test]
fn a_poisoned_lock_is_taken_over() {
    let window = Arc::new(Window::new(2, usize::MAX));
    window.push(typed(1, "a"));
    let poisoner = Arc::clone(&window);
    let outcome = std::thread::spawn(move || {
        let _guard = poisoner.inner.write().unwrap();
        panic!("poison the window lock");
    })
    .join();
    assert!(outcome.is_err());
    assert!(window.inner.is_poisoned());

    window.push(typed(2, "a"));
    window.push(typed(3, "a"));

    assert_eq!(window.start(), cursor(1));
    assert_eq!(window.last(), Some(sequence(3)));
    assert_eq!(window.bytes(), 2 * (2 + 1 + 128));
    assert_eq!(
        page(window.read(cursor(1), cursor(3), &types(&[("a", 0)]), UNLIMITED)),
        (vec![2, 3], cursor(3))
    );

    window.restart(cursor(9));

    assert_eq!(window.start(), cursor(9));
    assert_eq!(window.last(), None);
    assert_eq!(window.bytes(), 0);
}
