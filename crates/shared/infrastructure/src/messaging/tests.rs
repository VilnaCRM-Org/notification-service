use super::*;

fn settings(endpoint_url: Option<&str>) -> SqsSettings {
    SqsSettings {
        region: "eu-west-1".to_owned(),
        endpoint_url: endpoint_url.map(str::to_owned),
        connect_timeout: Duration::from_millis(300),
        operation_timeout: Duration::from_secs(1),
        max_attempts: 2,
    }
}

#[tokio::test]
async fn the_client_carries_region_endpoint_timeouts_and_attempts() {
    let client = sqs_client(&settings(Some("http://127.0.0.1:1"))).await;

    let config = client.config();
    assert_eq!(config.region().map(Region::as_ref), Some("eu-west-1"));
    let timeouts = config.timeout_config().unwrap();
    assert_eq!(timeouts.connect_timeout(), Some(Duration::from_millis(300)));
    assert_eq!(
        timeouts.operation_attempt_timeout(),
        Some(Duration::from_secs(1))
    );
    assert_eq!(timeouts.operation_timeout(), Some(Duration::from_secs(1)));
    assert_eq!(config.retry_config().unwrap().max_attempts(), 2);
}

#[tokio::test]
async fn a_client_without_an_endpoint_url_is_built_for_the_region() {
    let client = sqs_client(&settings(None)).await;

    assert_eq!(
        client.config().region().map(Region::as_ref),
        Some("eu-west-1")
    );
}
