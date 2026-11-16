use event_stream_domain::EventType;

use super::{event_type, stored_event};
use crate::seed::TestRng;

#[test]
fn generated_event_types_satisfy_the_grammar() {
    let mut rng = TestRng::from_seed(20_261_114);

    for _round in 0..500 {
        let name = event_type(&mut rng);

        assert!(follows_the_grammar(name.as_str()), "{name}");
        assert!(name.as_str().chars().count() <= 45);
    }
}

/// An oracle independent of `EventType::new`: a lower-case ASCII letter first, and splitting on
/// `.`, `_` and `-` gives only non-empty runs of `[a-z0-9]`.
fn follows_the_grammar(name: &str) -> bool {
    name.bytes()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
        && name.split(['.', '_', '-']).all(|run| {
            !run.is_empty()
                && run
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

#[test]
fn the_grammar_oracle_rejects_names_outside_the_grammar() {
    assert!(follows_the_grammar("order.paid-2_x"));
    for name in [
        "",
        "1order",
        "Order",
        "order.",
        "order..paid",
        "-order",
        "order paid",
        "ordér",
    ] {
        assert!(!follows_the_grammar(name), "{name}");
    }
}

#[test]
fn the_same_seed_gives_equal_event_types() {
    assert_eq!(
        event_type(&mut TestRng::from_seed(3)),
        event_type(&mut TestRng::from_seed(3))
    );
}

#[test]
fn event_types_vary_with_the_seed() {
    let mut rng = TestRng::from_seed(5);
    let names: std::collections::BTreeSet<EventType> =
        (0..50).map(|_| event_type(&mut rng)).collect();

    assert!(names.len() > 40);
}

#[test]
fn stored_events_carry_their_arguments_and_distinct_ids() {
    let first = stored_event(7, "order.paid", "{}");
    let second = stored_event(8, "order.paid", "{}");

    assert_eq!(first.sequence.value(), 7);
    assert_eq!(first.event_type.as_str(), "order.paid");
    assert_eq!(first.json, "{}");
    assert_eq!(first.id, uuid::Uuid::from_u128(7));
    assert_ne!(first.id, second.id);
}
