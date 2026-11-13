use std::time::Duration;

use shared_domain::PageRequest;
use shared_infrastructure::persistence::{PostgresSettings, connect_lazy};
use some_module_domain::{
    ExampleApiResource, ExampleApiResourceId, ExampleApiResourceRepository as _, RepositoryError,
};
use some_module_infrastructure::PgExampleApiResourceRepository;
use sqlx::PgPool;
use test_support::fixture::example_api_resource;
use test_support::seed::TestRng;
use tokio::time::Instant;

/// URL that nothing listens on: the "unreachable database" case. No container is stopped.
const DEAD_DATABASE_URL: &str = "postgres://app:unused@127.0.0.1:1/app";

/// Draws `count` resources from the generator of the test. Each test binds its generator as its first
/// statement and keeps it until it ends, so a failing assertion prints `TEST_SEED=<seed>`.
fn resources(rng: &mut TestRng, count: usize) -> Vec<ExampleApiResource> {
    (0..count).map(|_| example_api_resource(rng)).collect()
}

fn same(left: &ExampleApiResource, right: &ExampleApiResource) -> bool {
    left.id() == right.id()
        && left.name() == right.name()
        && left.description() == right.description()
}

#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]
async fn a_saved_resource_is_found_by_its_id(pool: PgPool) {
    let mut rng = TestRng::from_env();
    let repository = PgExampleApiResourceRepository::new(pool);
    let resource = resources(&mut rng, 1).remove(0);

    repository.save(&resource).await.unwrap();
    let found = repository.find_by_id(resource.id()).await.unwrap();

    assert!(same(&found.unwrap(), &resource));
}

#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]
async fn an_unknown_id_is_not_found(pool: PgPool) {
    let mut rng = TestRng::from_env();
    let repository = PgExampleApiResourceRepository::new(pool);
    let mut stored = resources(&mut rng, 2);
    let unknown = stored.remove(1).id();
    repository.save(&stored[0]).await.unwrap();

    let found = repository.find_by_id(unknown).await.unwrap();

    assert!(found.is_none());
}

#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]
async fn saving_the_same_id_twice_is_already_exists(pool: PgPool) {
    let mut rng = TestRng::from_env();
    let repository = PgExampleApiResourceRepository::new(pool);
    let resource = resources(&mut rng, 1).remove(0);
    repository.save(&resource).await.unwrap();

    let error = repository.save(&resource).await.unwrap_err();

    assert!(matches!(error, RepositoryError::AlreadyExists));
}

#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]
async fn pages_are_ordered_by_id_descending_with_limit_and_offset(pool: PgPool) {
    let mut rng = TestRng::from_env();
    let repository = PgExampleApiResourceRepository::new(pool);
    let mut stored = resources(&mut rng, 5);
    for resource in &stored {
        repository.save(resource).await.unwrap();
    }
    stored.sort_by_key(|resource| std::cmp::Reverse(resource.id().value().as_uuid()));
    let ids: Vec<ExampleApiResourceId> = stored.iter().map(ExampleApiResource::id).collect();

    let first = repository
        .find_page(PageRequest::new(1, 2).unwrap())
        .await
        .unwrap();
    let second = repository
        .find_page(PageRequest::new(2, 2).unwrap())
        .await
        .unwrap();
    let third = repository
        .find_page(PageRequest::new(3, 2).unwrap())
        .await
        .unwrap();
    let beyond = repository
        .find_page(PageRequest::new(4, 2).unwrap())
        .await
        .unwrap();

    let page_ids = |page: &[ExampleApiResource]| -> Vec<ExampleApiResourceId> {
        page.iter().map(ExampleApiResource::id).collect()
    };
    assert_eq!(page_ids(&first), ids[0..2]);
    assert_eq!(page_ids(&second), ids[2..4]);
    assert_eq!(page_ids(&third), ids[4..5]);
    assert!(beyond.is_empty());
}

#[sqlx::test(migrator = "shared_infrastructure::persistence::MIGRATOR")]
async fn a_stored_row_that_breaks_a_domain_invariant_is_unavailable(pool: PgPool) {
    let mut rng = TestRng::from_env();
    sqlx::query(
        "ALTER TABLE example_api_resources DROP CONSTRAINT example_api_resources_name_check",
    )
    .execute(&pool)
    .await
    .unwrap();
    let id = resources(&mut rng, 1).remove(0).id();
    sqlx::query(
        "INSERT INTO example_api_resources (id, name, description) VALUES ($1, '', 'text')",
    )
    .bind(id.value().as_uuid())
    .execute(&pool)
    .await
    .unwrap();
    let repository = PgExampleApiResourceRepository::new(pool);

    let by_id = repository.find_by_id(id).await.unwrap_err();
    let page = repository
        .find_page(PageRequest::new(1, 10).unwrap())
        .await
        .unwrap_err();

    assert!(matches!(by_id, RepositoryError::Unavailable(_)));
    assert!(matches!(page, RepositoryError::Unavailable(_)));
}

#[tokio::test]
async fn an_unreachable_database_is_unavailable_within_the_acquire_timeout() {
    let mut rng = TestRng::from_env();
    let acquire_timeout = Duration::from_millis(500);
    let pool = connect_lazy(&PostgresSettings {
        url: DEAD_DATABASE_URL.to_owned(),
        max_connections: 1,
        acquire_timeout,
        statement_timeout: Duration::from_secs(1),
    })
    .unwrap();
    let repository = PgExampleApiResourceRepository::new(pool);
    let resource = resources(&mut rng, 1).remove(0);
    let bound = acquire_timeout + Duration::from_secs(1);

    let started = Instant::now();
    let saved = repository.save(&resource).await.unwrap_err();
    assert!(started.elapsed() < bound);
    let started = Instant::now();
    let found = repository.find_by_id(resource.id()).await.unwrap_err();
    assert!(started.elapsed() < bound);
    let started = Instant::now();
    let paged = repository
        .find_page(PageRequest::new(1, 1).unwrap())
        .await
        .unwrap_err();
    assert!(started.elapsed() < bound);

    for error in [saved, found, paged] {
        assert!(matches!(error, RepositoryError::Unavailable(_)));
        let text = format!("{error} {error:?}").to_lowercase();
        assert_eq!(error.to_string(), "the repository is unavailable");
        assert!(!text.contains("insert"));
        assert!(!text.contains("select"));
    }
}
