use aws_sdk_sqs::Client;
use aws_sdk_sqs::operation::list_queues::{ListQueuesError, ListQueuesOutput};
use aws_sdk_sqs::types::error::InvalidAddress;
use aws_smithy_mocks::{RuleMode, mock, mock_client};
use health_check_domain::{Dependency, HealthProbe as _};

use super::SqsProbe;

/// A `mock!` builder that answers only a `ListQueues` call asking for one result.
macro_rules! list_one_queue {
    () => {
        mock!(Client::list_queues).match_requests(|input| input.max_results() == Some(1))
    };
}

#[tokio::test]
async fn an_answered_list_queues_call_passes() {
    let rule = list_one_queue!().then_output(|| ListQueuesOutput::builder().build());
    let client = mock_client!(aws_sdk_sqs, RuleMode::MatchAny, [&rule]);

    let result = SqsProbe::new(client).check().await;

    assert!(result.is_ok());
    assert_eq!(rule.num_calls(), 1);
}

#[tokio::test]
async fn a_service_error_fails() {
    let rule = list_one_queue!()
        .then_error(|| ListQueuesError::InvalidAddress(InvalidAddress::builder().build()));
    let client = mock_client!(aws_sdk_sqs, RuleMode::MatchAny, [&rule]);

    let result = SqsProbe::new(client).check().await;

    assert!(result.is_err());
    assert_eq!(rule.num_calls(), 1);
}

#[test]
fn the_probe_stands_for_the_message_broker() {
    let client = mock_client!(aws_sdk_sqs, RuleMode::MatchAny, []);

    assert_eq!(
        SqsProbe::new(client).dependency(),
        Dependency::MessageBroker
    );
}
