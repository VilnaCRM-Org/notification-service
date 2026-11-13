use rstest::rstest;

use super::Dependency;

#[rstest]
#[case::database(Dependency::Database, "database", "Database is not available")]
#[case::cache(Dependency::Cache, "cache", "Cache is not working")]
#[case::message_broker(
    Dependency::MessageBroker,
    "message_broker",
    "Message broker is not available"
)]
fn a_dependency_has_an_identifier_and_a_failure_message(
    #[case] dependency: Dependency,
    #[case] identifier: &str,
    #[case] message: &str,
) {
    assert_eq!(dependency.identifier(), identifier);
    assert_eq!(dependency.failure_message(), message);
}
