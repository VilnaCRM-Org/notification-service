use shared_domain::{PageRequest, Uuid};
use some_module_domain::{
    ExampleApiResource, ExampleApiResourceDescription, ExampleApiResourceId,
    ExampleApiResourceName, ExampleApiResourceRepository, RepositoryError,
};

use super::InMemoryExampleApiResourceRepository;
use crate::block_on;

fn id(counter: u128) -> ExampleApiResourceId {
    ExampleApiResourceId::new(Uuid::from_uuid(uuid::Uuid::from_u128(counter)))
}

fn resource(counter: u128, name: &str) -> ExampleApiResource {
    ExampleApiResource::restore(
        id(counter),
        ExampleApiResourceName::new(name.to_owned()).unwrap(),
        ExampleApiResourceDescription::new("description".to_owned()).unwrap(),
    )
}

fn page(page: u32, items_per_page: u32) -> PageRequest {
    PageRequest::new(page, items_per_page).unwrap()
}

fn names(resources: &[ExampleApiResource]) -> Vec<&str> {
    resources
        .iter()
        .map(|resource| resource.name().as_str())
        .collect()
}

fn filled(count: u128) -> InMemoryExampleApiResourceRepository {
    let repository = InMemoryExampleApiResourceRepository::new();
    let port: &dyn ExampleApiResourceRepository = &repository;
    for counter in 1..=count {
        block_on(port.save(&resource(counter, &format!("n{counter}")))).unwrap();
    }
    repository
}

#[test]
fn saves_and_finds_a_resource() {
    let repository = InMemoryExampleApiResourceRepository::default();
    let port: &dyn ExampleApiResourceRepository = &repository;

    block_on(port.save(&resource(1, "one"))).unwrap();

    let found = block_on(port.find_by_id(id(1))).unwrap().unwrap();
    assert_eq!(found.id(), id(1));
    assert_eq!(found.name().as_str(), "one");
    assert_eq!(found.description().as_str(), "description");
    assert!(block_on(port.find_by_id(id(2))).unwrap().is_none());
}

#[test]
fn saving_an_existing_id_is_already_exists_and_keeps_the_first() {
    let repository = filled(1);
    let port: &dyn ExampleApiResourceRepository = &repository;

    let error = block_on(port.save(&resource(1, "other"))).unwrap_err();

    assert!(matches!(error, RepositoryError::AlreadyExists));
    let found = block_on(port.find_by_id(id(1))).unwrap().unwrap();
    assert_eq!(found.name().as_str(), "n1");
}

#[test]
fn a_page_is_ordered_by_id_descending() {
    let repository = filled(5);
    let port: &dyn ExampleApiResourceRepository = &repository;

    let all = block_on(port.find_page(page(1, 100))).unwrap();

    assert_eq!(names(&all), ["n5", "n4", "n3", "n2", "n1"]);
}

#[test]
fn a_page_respects_its_bounds() {
    let repository = filled(5);
    let port: &dyn ExampleApiResourceRepository = &repository;

    let first = block_on(port.find_page(page(1, 2))).unwrap();
    let second = block_on(port.find_page(page(2, 2))).unwrap();
    let third = block_on(port.find_page(page(3, 2))).unwrap();
    let beyond = block_on(port.find_page(page(4, 2))).unwrap();

    assert_eq!(names(&first), ["n5", "n4"]);
    assert_eq!(names(&second), ["n3", "n2"]);
    assert_eq!(names(&third), ["n1"]);
    assert!(beyond.is_empty());
}

#[test]
fn an_empty_repository_has_an_empty_page() {
    let repository = InMemoryExampleApiResourceRepository::new();
    let port: &dyn ExampleApiResourceRepository = &repository;

    assert!(block_on(port.find_page(page(1, 30))).unwrap().is_empty());
}

#[test]
fn the_failing_repository_is_unavailable_from_every_method() {
    let repository = InMemoryExampleApiResourceRepository::failing();
    let port: &dyn ExampleApiResourceRepository = &repository;

    assert!(matches!(
        block_on(port.save(&resource(1, "one"))),
        Err(RepositoryError::Unavailable(_))
    ));
    assert!(matches!(
        block_on(port.find_by_id(id(1))),
        Err(RepositoryError::Unavailable(_))
    ));
    assert!(matches!(
        block_on(port.find_page(page(1, 30))),
        Err(RepositoryError::Unavailable(_))
    ));
}
