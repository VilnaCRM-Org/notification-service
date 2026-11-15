use rstest::rstest;
use uuid::Uuid;

use super::StoredEvent;
use crate::value_object::event_type::EventType;
use crate::value_object::sequence::Sequence;

fn event(type_length: usize, json_length: usize) -> StoredEvent {
    StoredEvent {
        sequence: Sequence::new(7).unwrap(),
        id: Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0001),
        event_type: EventType::new("a".repeat(type_length)).unwrap(),
        json: "x".repeat(json_length),
    }
}

#[test]
fn costs_the_json_and_the_type_plus_128() {
    assert_eq!(event(12, 100).cost(), 240);
}

#[rstest]
#[case(1, 0, 129)]
#[case(1, 1, 130)]
#[case(12, 0, 140)]
#[case(1, 100, 229)]
#[case(128, 4096, 4352)]
fn adds_each_part_once(
    #[case] type_length: usize,
    #[case] json_length: usize,
    #[case] cost: usize,
) {
    assert_eq!(event(type_length, json_length).cost(), cost);
}

#[test]
fn counts_bytes_not_characters() {
    let mut multibyte = event(12, 0);
    multibyte.json = "\u{e9}".repeat(50);

    assert_eq!(multibyte.cost(), 240);
}

#[test]
fn holds_its_members() {
    let held = event(5, 3);

    assert_eq!(held.sequence.value(), 7);
    assert_eq!(held.id.to_string(), "01930000-0000-7000-8000-000000000001");
    assert_eq!(held.event_type.as_str(), "aaaaa");
    assert_eq!(held.json, "xxx");
    assert_eq!(held.clone(), held);
}
