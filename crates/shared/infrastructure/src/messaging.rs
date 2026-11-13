//! SQS client factory.

use std::time::Duration;

use aws_config::retry::RetryConfig;
use aws_config::timeout::TimeoutConfig;
use aws_config::{BehaviorVersion, Region};

/// Settings of the SQS client. Credentials are never part of them: they come from the SDK's default chain.
#[derive(Debug, Clone)]
pub struct SqsSettings {
    /// AWS region.
    pub region: String,
    /// Endpoint of the service; `None` selects the regional AWS endpoint.
    pub endpoint_url: Option<String>,
    /// Timeout of establishing a connection.
    pub connect_timeout: Duration,
    /// Timeout of one attempt and of the whole operation.
    pub operation_timeout: Duration,
    /// Attempts per operation, the first one included.
    pub max_attempts: u32,
}

/// Builds the client; it opens no connection until the first call.
pub async fn sqs_client(settings: &SqsSettings) -> aws_sdk_sqs::Client {
    let timeouts = TimeoutConfig::builder()
        .connect_timeout(settings.connect_timeout)
        .operation_attempt_timeout(settings.operation_timeout)
        .operation_timeout(settings.operation_timeout)
        .build();
    let mut loader = aws_config::defaults(BehaviorVersion::v2026_11_10())
        .region(Region::new(settings.region.clone()))
        .timeout_config(timeouts)
        .retry_config(RetryConfig::standard().with_max_attempts(settings.max_attempts));
    if let Some(url) = &settings.endpoint_url {
        loader = loader.endpoint_url(url);
    }
    aws_sdk_sqs::Client::new(&loader.load().await)
}

#[cfg(test)]
mod tests;
