//! Settings of the SQS emulator of the test stack, read from the environment the Make targets export.

use std::time::Duration;

use shared_infrastructure::messaging::SqsSettings;

/// Endpoint that nothing listens on: the "unreachable broker" case.
pub(crate) const DEAD_SQS_ENDPOINT: &str = "http://127.0.0.1:1";

#[expect(
    clippy::disallowed_methods,
    reason = "the test stack is described by the environment"
)]
fn env(name: &str) -> String {
    std::env::var(name)
        .unwrap_or_else(|_| panic!("{name} is not set; run the tests through `make`"))
}

/// Settings of the SQS emulator of the test stack, with generous timeouts: a call that must succeed
/// does not depend on how fast the machine opens a connection.
pub(crate) fn sqs_settings() -> SqsSettings {
    SqsSettings {
        region: env("AWS_REGION"),
        endpoint_url: Some(env("AWS_ENDPOINT_URL_SQS")),
        connect_timeout: Duration::from_secs(10),
        operation_timeout: Duration::from_secs(30),
        max_attempts: 1,
    }
}
