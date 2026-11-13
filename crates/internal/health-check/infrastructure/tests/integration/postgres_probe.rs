use std::time::Duration;

use health_check_domain::{Dependency, HealthProbe as _};
use health_check_infrastructure::PostgresProbe;
use shared_infrastructure::persistence::{PostgresSettings, connect_lazy};
use tokio::time::Instant;

use crate::support;

#[tokio::test]
async fn a_healthy_database_passes() {
    let pool = connect_lazy(&support::postgres_settings()).unwrap();
    let probe = PostgresProbe::new(pool);

    let result = probe.check().await;

    assert!(result.is_ok());
    assert_eq!(probe.dependency(), Dependency::Database);
}

#[tokio::test]
async fn an_unreachable_database_fails_within_the_acquire_timeout_plus_one_second() {
    let acquire_timeout = Duration::from_millis(500);
    let settings = PostgresSettings {
        url: support::DEAD_DATABASE_URL.to_owned(),
        acquire_timeout,
        ..support::postgres_settings()
    };
    let probe = PostgresProbe::new(connect_lazy(&settings).unwrap());
    let started = Instant::now();

    let result = tokio::time::timeout(Duration::from_secs(10), probe.check())
        .await
        .expect("the probe must give up on its own");

    assert!(result.is_err());
    assert!(started.elapsed() < acquire_timeout + Duration::from_secs(1));
}
