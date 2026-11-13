use std::sync::Arc;

use shared_domain::{Query as _, QueryHandler as _, Uuid};
use some_module_domain::{
    ExampleApiResource, ExampleApiResourceDescription, ExampleApiResourceId,
    ExampleApiResourceName, ExampleApiResourceRepository as _,
};
use test_support::repository::InMemoryExampleApiResourceRepository;

use super::{GetExampleApiResource, GetExampleApiResourceHandler};
use crate::error::ExampleApiResourceError;
use crate::query::ExampleApiResourceView;

fn id(counter: u128) -> Uuid {
    Uuid::from_uuid(uuid::Uuid::from_u128(counter))
}

/// A working repository that holds three resources.
async fn seeded_repository() -> InMemoryExampleApiResourceRepository {
    let repository = InMemoryExampleApiResourceRepository::new();
    for counter in 1..=3 {
        let resource = ExampleApiResource::restore(
            ExampleApiResourceId::new(id(counter)),
            ExampleApiResourceName::new(format!("n{counter}")).unwrap(),
            ExampleApiResourceDescription::new("Description".to_owned()).unwrap(),
        );
        repository.save(&resource).await.unwrap();
    }
    repository
}

#[test]
fn the_query_is_named_after_its_type() {
    assert_eq!(GetExampleApiResource::NAME, "GetExampleApiResource");
}

#[tokio::test]
async fn a_stored_id_answers_its_view() {
    let handler = GetExampleApiResourceHandler::new(Arc::new(seeded_repository().await));

    let answer = handler
        .handle(GetExampleApiResource { id: id(2) })
        .await
        .unwrap();

    assert_eq!(
        answer,
        Some(ExampleApiResourceView {
            id: id(2),
            name: "n2".to_owned(),
            description: "Description".to_owned(),
        })
    );
}

#[tokio::test]
async fn an_unknown_id_answers_none() {
    let handler = GetExampleApiResourceHandler::new(Arc::new(seeded_repository().await));

    let answer = handler
        .handle(GetExampleApiResource { id: id(4) })
        .await
        .unwrap();

    assert_eq!(answer, None);
}

#[tokio::test]
async fn a_failing_repository_is_a_storage_error() {
    let handler = GetExampleApiResourceHandler::new(Arc::new(
        InMemoryExampleApiResourceRepository::failing(),
    ));

    let error = handler
        .handle(GetExampleApiResource { id: id(1) })
        .await
        .unwrap_err();

    assert!(matches!(error, ExampleApiResourceError::Storage(_)));
}
