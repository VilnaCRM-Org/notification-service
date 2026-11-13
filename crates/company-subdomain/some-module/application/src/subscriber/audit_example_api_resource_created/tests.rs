use std::time::{Duration, SystemTime};

use shared_domain::{DomainEventSubscriber as _, Uuid};
use some_module_domain::ExampleApiResourceCreated;
use test_support::log::LogRecorder;

use super::AuditExampleApiResourceCreated;

#[tokio::test]
async fn an_event_is_one_audit_entry_at_info() {
    let (recorder, _guard) = LogRecorder::install();
    let event = ExampleApiResourceCreated {
        event_id: Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-000000000001").unwrap(),
        aggregate_id: Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90").unwrap(),
        occurred_at: SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_secs(1_700_000_000))
            .unwrap(),
    };

    AuditExampleApiResourceCreated.handle(&event).await.unwrap();

    let lines = recorder.lines();
    assert_eq!(lines.len(), 1);
    let line = lines.first().unwrap();
    assert_eq!(line.level, tracing::Level::INFO);
    assert_eq!(line.target, "audit");
    assert_eq!(line.message, "domain event");
    assert_eq!(
        line.field("event_name"),
        Some("example_api_resource.created")
    );
    assert_eq!(
        line.field("event_id"),
        Some("0192f3a0-7b1c-7d2e-8f3a-000000000001")
    );
    assert_eq!(
        line.field("aggregate_id"),
        Some("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90")
    );
    assert_eq!(
        line.field("occurred_at"),
        Some("2023-11-14T22:13:20.000000Z")
    );
}
