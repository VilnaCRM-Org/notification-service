use shared_domain::Uuid;

use super::ExampleApiResourceId;

fn uuid() -> Uuid {
    Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90").unwrap()
}

#[test]
fn returns_the_wrapped_uuid() {
    assert_eq!(ExampleApiResourceId::new(uuid()).value(), uuid());
}

#[test]
fn displays_as_the_hyphenated_uuid() {
    assert_eq!(
        ExampleApiResourceId::new(uuid()).to_string(),
        "0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90"
    );
}

#[test]
fn ids_with_the_same_uuid_are_equal() {
    assert_eq!(
        ExampleApiResourceId::new(uuid()),
        ExampleApiResourceId::new(uuid())
    );
    assert_ne!(
        ExampleApiResourceId::new(uuid()),
        ExampleApiResourceId::new(Uuid::parse("00000000-0000-0000-0000-000000000000").unwrap())
    );
}
