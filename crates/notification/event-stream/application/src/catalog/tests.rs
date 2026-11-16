use std::collections::BTreeSet;

use event_stream_domain::EventType;
use rstest::rstest;
use shared_domain::PageRequest;

use super::EventCatalog;

fn name(value: &str) -> EventType {
    EventType::new(value.to_owned()).unwrap()
}

fn names(values: &[&str]) -> Vec<EventType> {
    values.iter().copied().map(name).collect()
}

fn catalog(values: &[&str]) -> EventCatalog {
    EventCatalog::new(values.iter().copied().map(name).collect::<BTreeSet<_>>())
}

fn page(page: u32, items_per_page: u32) -> PageRequest {
    PageRequest::new(page, items_per_page).unwrap()
}

#[test]
fn keeps_distinct_names_in_byte_order() {
    let catalog = catalog(&[
        "user.created",
        "order.paid",
        "order-paid",
        "order_paid",
        "order.paid",
    ]);

    assert_eq!(
        catalog.page(page(1, 30)),
        names(&["order-paid", "order.paid", "order_paid", "user.created"])
    );
}

#[test]
fn the_default_catalog_is_empty() {
    assert_eq!(
        EventCatalog::default().page(page(1, 30)),
        Vec::<EventType>::new()
    );
    assert_eq!(EventCatalog::default(), EventCatalog::new(BTreeSet::new()));
}

#[rstest]
#[case::first_of_one(1, 1, &["order.paid"])]
#[case::second_of_one(2, 1, &["user.created"])]
#[case::third_of_one(3, 1, &["user.deleted"])]
#[case::beyond_the_last(4, 1, &[])]
#[case::first_of_two(1, 2, &["order.paid", "user.created"])]
#[case::second_of_two(2, 2, &["user.deleted"])]
#[case::all(1, 100, &["order.paid", "user.created", "user.deleted"])]
#[case::far_beyond(u32::MAX, 100, &[])]
fn pages_the_names(#[case] number: u32, #[case] size: u32, #[case] expected: &[&str]) {
    let catalog = catalog(&["user.deleted", "order.paid", "user.created"]);

    assert_eq!(catalog.page(page(number, size)), names(expected));
}
