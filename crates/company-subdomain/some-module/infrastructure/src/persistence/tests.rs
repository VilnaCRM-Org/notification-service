use std::error::Error as _;

use shared_domain::AggregateRoot as _;

use super::*;

fn row(name: &str, description: &str) -> Row {
    Row {
        id: uuid::Uuid::from_u128(7),
        name: name.to_owned(),
        description: description.to_owned(),
    }
}

#[test]
fn a_valid_row_is_restored_without_events() {
    let mut resource = restore(row("name", "description")).unwrap();

    assert_eq!(resource.id().value().as_uuid(), uuid::Uuid::from_u128(7));
    assert_eq!(resource.name().as_str(), "name");
    assert_eq!(resource.description().as_str(), "description");
    assert_eq!(resource.pull_domain_events().len(), 0);
}

#[test]
fn a_row_with_an_invalid_name_is_unavailable() {
    let error = restore(row("", "description")).unwrap_err();

    assert!(matches!(error, RepositoryError::Unavailable(_)));
    assert!(error.source().is_some());
}

#[test]
fn a_row_with_an_invalid_description_is_unavailable() {
    let error = restore(row("name", "")).unwrap_err();

    assert!(matches!(error, RepositoryError::Unavailable(_)));
}

#[test]
fn an_error_that_is_not_a_database_error_is_unavailable() {
    let error = translate(sqlx::Error::PoolTimedOut);

    assert!(matches!(error, RepositoryError::Unavailable(_)));
    assert_eq!(error.to_string(), "the repository is unavailable");
}
