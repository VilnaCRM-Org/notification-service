use std::time::Duration;

use shared_infrastructure::messaging::{SqsSettings, sqs_client};
use tokio::time::Instant;

use crate::support;

#[tokio::test]
async fn list_queues_succeeds_against_localstack() {
    let client = sqs_client(&support::sqs_settings()).await;

    let result = client.list_queues().max_results(1).send().await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn list_queues_fails_within_the_operation_timeout_against_a_dead_endpoint() {
    let settings = SqsSettings {
        endpoint_url: Some(support::DEAD_SQS_ENDPOINT.to_owned()),
        operation_timeout: Duration::from_millis(500),
        max_attempts: 2,
        ..support::sqs_settings()
    };
    let client = sqs_client(&settings).await;
    let started = Instant::now();

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        client.list_queues().max_results(1).send(),
    )
    .await
    .expect("the call must give up on its own");

    assert!(result.is_err());
    assert!(started.elapsed() < Duration::from_millis(1500));
}
