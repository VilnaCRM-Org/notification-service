use std::time::Duration;

use health_check_domain::HealthProbe as _;
use health_check_infrastructure::SqsProbe;
use shared_infrastructure::messaging::{SqsSettings, sqs_client};
use tokio::time::Instant;

use crate::support;

async fn queues(client: &aws_sdk_sqs::Client) -> Vec<String> {
    let mut urls: Vec<String> = client
        .list_queues()
        .send()
        .await
        .unwrap()
        .queue_urls()
        .to_vec();
    urls.sort();
    urls
}

#[tokio::test]
async fn localstack_answers_and_the_queues_are_unchanged() {
    let client = sqs_client(&support::sqs_settings()).await;
    let before = queues(&client).await;

    let result = SqsProbe::new(client.clone()).check().await;

    assert!(result.is_ok());
    assert_eq!(queues(&client).await, before);
}

#[tokio::test]
async fn an_unreachable_endpoint_fails_within_the_operation_timeout_plus_one_second() {
    let operation_timeout = Duration::from_millis(500);
    let settings = SqsSettings {
        endpoint_url: Some(support::DEAD_SQS_ENDPOINT.to_owned()),
        operation_timeout,
        ..support::sqs_settings()
    };
    let probe = SqsProbe::new(sqs_client(&settings).await);
    let started = Instant::now();

    let result = tokio::time::timeout(Duration::from_secs(10), probe.check())
        .await
        .expect("the probe must give up on its own");

    assert!(result.is_err());
    assert!(started.elapsed() < operation_timeout + Duration::from_secs(1));
}
