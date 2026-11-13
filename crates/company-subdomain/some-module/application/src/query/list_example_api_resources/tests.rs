use std::sync::Arc;

use shared_domain::{InvalidPageRequest, Query as _, QueryHandler as _, Uuid};
use some_module_domain::{
    ExampleApiResource, ExampleApiResourceDescription, ExampleApiResourceId,
    ExampleApiResourceName, ExampleApiResourceRepository as _,
};
use test_support::repository::InMemoryExampleApiResourceRepository;

use super::{ListExampleApiResources, ListExampleApiResourcesHandler};
use crate::error::ExampleApiResourceError;
use crate::query::ExampleApiResourceView;

fn id(counter: u128) -> Uuid {
    Uuid::from_uuid(uuid::Uuid::from_u128(counter))
}

fn view(counter: u128) -> ExampleApiResourceView {
    ExampleApiResourceView {
        id: id(counter),
        name: format!("n{counter}"),
        description: "Description".to_owned(),
    }
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

fn ask(page: u32, items_per_page: u32) -> ListExampleApiResources {
    ListExampleApiResources {
        page,
        items_per_page,
    }
}

#[test]
fn the_query_is_named_after_its_type() {
    assert_eq!(ListExampleApiResources::NAME, "ListExampleApiResources");
}

#[tokio::test]
async fn pages_hold_the_newest_views_first() {
    let handler = ListExampleApiResourcesHandler::new(Arc::new(seeded_repository().await));

    let first = handler.handle(ask(1, 2)).await.unwrap();
    let second = handler.handle(ask(2, 2)).await.unwrap();
    let beyond = handler.handle(ask(9, 2)).await.unwrap();

    assert_eq!(first, [view(3), view(2)]);
    assert_eq!(second, [view(1)]);
    assert_eq!(beyond, Vec::<ExampleApiResourceView>::new());
}

#[tokio::test]
async fn an_invalid_page_request_is_invalid_page() {
    let handler = ListExampleApiResourcesHandler::new(Arc::new(seeded_repository().await));

    let page = handler.handle(ask(0, 2)).await.unwrap_err();
    let size = handler.handle(ask(1, 101)).await.unwrap_err();

    assert!(matches!(
        page,
        ExampleApiResourceError::InvalidPage(InvalidPageRequest::Page)
    ));
    assert!(matches!(
        size,
        ExampleApiResourceError::InvalidPage(InvalidPageRequest::ItemsPerPage)
    ));
}

#[tokio::test]
async fn a_failing_repository_is_a_storage_error() {
    let handler = ListExampleApiResourcesHandler::new(Arc::new(
        InMemoryExampleApiResourceRepository::failing(),
    ));

    let error = handler.handle(ask(1, 2)).await.unwrap_err();

    assert!(matches!(error, ExampleApiResourceError::Storage(_)));
}
