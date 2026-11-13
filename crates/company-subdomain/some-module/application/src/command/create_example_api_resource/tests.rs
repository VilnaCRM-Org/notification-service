use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use shared_domain::{
    Command as _, CommandHandler as _, DomainEvent, EventBus, EventBusError, SubscriberFailure,
    Uuid, UuidGenerator as _,
};
use some_module_domain::{ExampleApiResourceId, ExampleApiResourceRepository};
use test_support::bus::{PublishedEvent, RecordingEventBus};
use test_support::clock::FixedClock;
use test_support::id::SequenceUuidGenerator;
use test_support::log::LogRecorder;
use test_support::repository::InMemoryExampleApiResourceRepository;

use super::{CreateExampleApiResource, CreateExampleApiResourceHandler};
use crate::error::ExampleApiResourceError;

struct Fixture {
    handler: CreateExampleApiResourceHandler,
    repository: Arc<InMemoryExampleApiResourceRepository>,
    bus: Arc<RecordingEventBus>,
}

fn time() -> SystemTime {
    SystemTime::UNIX_EPOCH
        .checked_add(Duration::from_secs(1_700_000_000))
        .unwrap()
}

fn fixture(repository: InMemoryExampleApiResourceRepository, bus: RecordingEventBus) -> Fixture {
    let repository = Arc::new(repository);
    let bus = Arc::new(bus);
    let repository_port: Arc<dyn ExampleApiResourceRepository> =
        Arc::<InMemoryExampleApiResourceRepository>::clone(&repository);
    let bus_port: Arc<dyn EventBus> = Arc::<RecordingEventBus>::clone(&bus);
    let handler = CreateExampleApiResourceHandler::new(
        repository_port,
        bus_port,
        Arc::new(FixedClock::at(time())),
        Arc::new(SequenceUuidGenerator::starting_at(100)),
    );
    Fixture {
        handler,
        repository,
        bus,
    }
}

fn working() -> Fixture {
    fixture(
        InMemoryExampleApiResourceRepository::new(),
        RecordingEventBus::new(),
    )
}

fn id() -> Uuid {
    Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90").unwrap()
}

fn command(name: &str, description: &str) -> CreateExampleApiResource {
    CreateExampleApiResource {
        id: id(),
        name: name.to_owned(),
        description: description.to_owned(),
    }
}

async fn stored(fixture: &Fixture) -> Option<(String, String)> {
    let found = fixture
        .repository
        .find_by_id(ExampleApiResourceId::new(id()))
        .await
        .unwrap();
    found.map(|resource| {
        (
            resource.name().as_str().to_owned(),
            resource.description().as_str().to_owned(),
        )
    })
}

#[test]
fn the_command_is_named_after_its_type() {
    assert_eq!(CreateExampleApiResource::NAME, "CreateExampleApiResource");
}

#[tokio::test]
async fn a_valid_command_stores_the_resource_and_publishes_one_event_after_the_save() {
    let fixture = working();

    fixture
        .handler
        .handle(command("Name", "Description"))
        .await
        .unwrap();

    assert_eq!(
        stored(&fixture).await,
        Some(("Name".to_owned(), "Description".to_owned()))
    );
    assert_eq!(
        fixture.bus.published(),
        [PublishedEvent {
            event_name: "example_api_resource.created",
            event_id: SequenceUuidGenerator::starting_at(100).generate(),
            aggregate_id: id(),
        }]
    );
}

/// An event bus that keeps the occurrence time of the events it receives.
#[derive(Debug, Default)]
struct TimeBus(Mutex<Vec<(Uuid, SystemTime)>>);

#[async_trait::async_trait]
impl EventBus for TimeBus {
    async fn publish(&self, events: Vec<Box<dyn DomainEvent>>) -> Result<(), EventBusError> {
        let seen: Vec<_> = events
            .iter()
            .map(|event| (event.event_id(), event.occurred_at()))
            .collect();
        self.0.lock().unwrap().extend(seen);
        Ok(())
    }
}

#[tokio::test]
async fn the_event_carries_the_generated_id_and_the_clock_time() {
    let bus = Arc::new(TimeBus::default());
    let bus_port: Arc<dyn EventBus> = Arc::<TimeBus>::clone(&bus);
    let handler = CreateExampleApiResourceHandler::new(
        Arc::new(InMemoryExampleApiResourceRepository::new()),
        bus_port,
        Arc::new(FixedClock::at(time())),
        Arc::new(SequenceUuidGenerator::starting_at(100)),
    );

    handler
        .handle(command("Name", "Description"))
        .await
        .unwrap();

    assert_eq!(
        *bus.0.lock().unwrap(),
        [(SequenceUuidGenerator::starting_at(100).generate(), time())]
    );
}

#[tokio::test]
async fn a_failing_repository_is_a_storage_error_and_publishes_nothing() {
    let fixture = fixture(
        InMemoryExampleApiResourceRepository::failing(),
        RecordingEventBus::new(),
    );

    let error = fixture
        .handler
        .handle(command("Name", "Description"))
        .await
        .unwrap_err();

    assert!(matches!(error, ExampleApiResourceError::Storage(_)));
    assert_eq!(fixture.bus.published(), Vec::<PublishedEvent>::new());
}

#[tokio::test]
async fn an_invalid_name_or_description_is_invalid_and_stores_nothing() {
    let fixture = working();

    let name = fixture.handler.handle(command("", "Description")).await;
    let description = fixture.handler.handle(command("Name", "")).await;

    assert!(matches!(name, Err(ExampleApiResourceError::Invalid(_))));
    assert!(matches!(
        description,
        Err(ExampleApiResourceError::Invalid(_))
    ));
    assert_eq!(stored(&fixture).await, None);
    assert_eq!(fixture.bus.published(), Vec::<PublishedEvent>::new());
}

#[tokio::test]
async fn a_duplicate_id_is_already_exists_and_publishes_once() {
    let fixture = working();
    fixture
        .handler
        .handle(command("Name", "Description"))
        .await
        .unwrap();

    let error = fixture
        .handler
        .handle(command("Other", "Other"))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ExampleApiResourceError::AlreadyExists(duplicate) if duplicate.value() == id()
    ));
    assert_eq!(
        stored(&fixture).await,
        Some(("Name".to_owned(), "Description".to_owned()))
    );
    assert_eq!(fixture.bus.published().len(), 1);
}

#[tokio::test]
async fn a_failed_publication_is_logged_and_the_command_still_succeeds() {
    let (recorder, _guard) = LogRecorder::install();
    let fixture = fixture(
        InMemoryExampleApiResourceRepository::new(),
        RecordingEventBus::failing(),
    );

    fixture
        .handler
        .handle(command("Name", "Description"))
        .await
        .unwrap();

    assert_eq!(
        stored(&fixture).await,
        Some(("Name".to_owned(), "Description".to_owned()))
    );
    let published = fixture.bus.published();
    let lines = recorder.lines();
    assert_eq!(lines.len(), 1);
    let line = lines.first().unwrap();
    assert_eq!(line.level, tracing::Level::ERROR);
    assert_eq!(line.message, "event publication failed");
    assert_eq!(
        line.field("event_name"),
        Some("example_api_resource.created")
    );
    assert_eq!(
        line.field("event_id"),
        Some(published.first().unwrap().event_id.to_string().as_str())
    );
    assert_eq!(line.field("error"), Some("recording bus failure"));
}

/// An error whose text is generic and whose reason sits in its source, like an SDK error.
#[derive(Debug)]
struct Layered(&'static str, Option<Box<Self>>);

impl std::fmt::Display for Layered {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for Layered {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.1
            .as_deref()
            .map(|source| -> &(dyn std::error::Error + 'static) { source })
    }
}

/// An event bus whose subscriber fails with a layered error.
#[derive(Debug)]
struct LayeredFailureBus;

#[async_trait::async_trait]
impl EventBus for LayeredFailureBus {
    async fn publish(&self, events: Vec<Box<dyn DomainEvent>>) -> Result<(), EventBusError> {
        let failures = events
            .iter()
            .map(|event| SubscriberFailure {
                event_name: event.event_name(),
                event_id: event.event_id(),
                source: Box::new(Layered(
                    "dispatch failure",
                    Some(Box::new(Layered("connection refused", None))),
                )),
            })
            .collect();
        Err(EventBusError { failures })
    }
}

#[tokio::test]
async fn a_failed_publication_logs_the_whole_error_chain_of_the_subscriber() {
    let (recorder, _guard) = LogRecorder::install();
    let bus: Arc<dyn EventBus> = Arc::new(LayeredFailureBus);
    let handler = CreateExampleApiResourceHandler::new(
        Arc::new(InMemoryExampleApiResourceRepository::new()),
        bus,
        Arc::new(FixedClock::at(time())),
        Arc::new(SequenceUuidGenerator::starting_at(100)),
    );

    handler
        .handle(command("Name", "Description"))
        .await
        .unwrap();

    let lines = recorder.lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines.first().unwrap().field("error"),
        Some("dispatch failure: connection refused")
    );
}
