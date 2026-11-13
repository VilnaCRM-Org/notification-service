//! The example migration: the table, its named constraints and the one reversible migration.
//!
//! These tests belong to the example context. The migration-agnostic tests of the embedded
//! migrator stay in the integration tests of `shared-infrastructure`.

use shared_infrastructure::persistence::MIGRATOR;
use sqlx::{PgPool, Row as _};

const COLUMNS: &str = "SELECT table_name::text, column_name::text, data_type::text, is_nullable::text, \
    ordinal_position::text FROM information_schema.columns \
    WHERE table_schema = current_schema() AND table_name <> '_sqlx_migrations' \
    ORDER BY table_name, ordinal_position";

const CONSTRAINTS: &str = "SELECT conrelid::regclass::text, conname::text, contype::text, \
    pg_get_constraintdef(oid) FROM pg_constraint \
    WHERE connamespace = current_schema()::regnamespace AND conrelid <> '_sqlx_migrations'::regclass \
    ORDER BY conname";

type Catalog = (Vec<Vec<String>>, Vec<Vec<String>>);

async fn rows(pool: &PgPool, sql: &'static str) -> Vec<Vec<String>> {
    let rows = sqlx::query(sql).fetch_all(pool).await.unwrap();
    rows.iter()
        .map(|row| {
            (0..row.len())
                .map(|index| row.get::<String, _>(index))
                .collect()
        })
        .collect()
}

async fn catalog(pool: &PgPool) -> Catalog {
    (rows(pool, COLUMNS).await, rows(pool, CONSTRAINTS).await)
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[sqlx::test(migrations = false)]
async fn the_migration_creates_the_table_with_named_constraints(pool: PgPool) {
    MIGRATOR.run(&pool).await.unwrap();

    let (columns, constraints) = catalog(&pool).await;

    assert_eq!(
        columns,
        vec![
            strings(&["example_api_resources", "id", "uuid", "NO", "1"]),
            strings(&["example_api_resources", "name", "text", "NO", "2"]),
            strings(&["example_api_resources", "description", "text", "NO", "3"]),
        ]
    );
    let names: Vec<&str> = constraints.iter().map(|row| row[1].as_str()).collect();
    assert_eq!(
        names,
        vec![
            "example_api_resources_description_check",
            "example_api_resources_name_check",
            "example_api_resources_pkey",
        ]
    );
    assert_eq!(constraints[2][2], "p");
}

#[sqlx::test(migrations = false)]
async fn the_embedded_migrator_holds_one_reversible_migration(pool: PgPool) {
    MIGRATOR.run(&pool).await.unwrap();

    let applied: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(applied, 1);
    assert_eq!(
        MIGRATOR
            .iter()
            .filter(|migration| migration.migration_type.is_down_migration())
            .count(),
        1
    );
}

#[sqlx::test(migrations = false)]
async fn the_constraints_reject_an_empty_name_and_an_oversized_description(pool: PgPool) {
    MIGRATOR.run(&pool).await.unwrap();
    let insert = "INSERT INTO example_api_resources (id, name, description) VALUES (gen_random_uuid(), $1, $2)";

    let empty_name = sqlx::query(insert).bind("").bind("d").execute(&pool).await;
    let long_description = sqlx::query(insert)
        .bind("n")
        .bind("d".repeat(1025))
        .execute(&pool)
        .await;
    let valid = sqlx::query(insert)
        .bind("n")
        .bind("d".repeat(1024))
        .execute(&pool)
        .await;

    assert!(empty_name.is_err());
    assert!(long_description.is_err());
    assert!(valid.is_ok());
}
