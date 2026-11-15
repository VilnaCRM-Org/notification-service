//! The composition root: the only place where adapters meet use cases (AD-04).
//!
//! [`Adapters`] holds every driven port; [`router`] registers the handlers and builds the
//! application from it, so a test replaces any port by assigning a field or calling
//! [`Adapters::with_probe`].

use std::sync::Arc;

use axum::Router;
use event_stream_application::http::EventStreamState;
use event_stream_application::{
    EventCatalog, Hub, HubSettings, ListEventTypesHandler, ReplaySettings, StatusSource,
    StreamSettings, Subscriptions, TailState, Window,
};
use event_stream_domain::{TopicProbe, TopicReplay, TopicTail};
use event_stream_infrastructure::KafkaTopic;
use health_check_application::HealthCheckState;
use health_check_application::HealthService;
use health_check_domain::HealthProbe;
use health_check_infrastructure::{CacheProbe, PostgresProbe, SqsProbe};
use shared_domain::{Clock, CommandBus, EventBus, QueryBus, UuidGenerator};
use shared_infrastructure::bus::{
    DuplicateHandler, InMemoryCommandBus, InMemoryEventBus, InMemoryQueryBus,
};
use shared_infrastructure::cache::MokaCache;
use shared_infrastructure::clock::SystemClock;
use shared_infrastructure::id::UuidV7Generator;
use shared_infrastructure::messaging::sqs_client;
use shared_infrastructure::persistence::connect_lazy;
use some_module_application::http::SomeModuleState;
use some_module_application::{
    AuditExampleApiResourceCreated, CreateExampleApiResourceHandler, GetExampleApiResourceHandler,
    ListExampleApiResourcesHandler,
};
use some_module_domain::ExampleApiResourceRepository;
use some_module_infrastructure::PgExampleApiResourceRepository;

use tokio::sync::watch;

use crate::config::{Config, StreamConfig};
use crate::graphql::schema;
use crate::http::AppState;
use crate::http::router::assemble;

/// Every driven port of the application.
#[derive(Debug)]
pub struct Adapters {
    /// Repository of the example context.
    pub repository: Arc<dyn ExampleApiResourceRepository>,
    /// Health probes in the order database, cache, message broker.
    pub probes: Vec<Arc<dyn HealthProbe>>,
    /// Bus the command handlers publish domain events to.
    pub event_bus: Arc<dyn EventBus>,
    /// Source of the current time.
    pub clock: Arc<dyn Clock>,
    /// Source of identifiers.
    pub uuid_generator: Arc<dyn UuidGenerator>,
    /// Reader of the metadata and the watermarks of the event topic.
    pub topic_probe: Arc<dyn TopicProbe>,
    /// Reader of the tail of the event topic.
    pub topic_tail: Box<dyn TopicTail>,
    /// Reader of pages of older records of the event topic, shared by the replays.
    pub topic_replay: Arc<dyn TopicReplay>,
}

/// The application could not be wired.
#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    /// The database settings are invalid.
    #[error("invalid database settings")]
    Database(#[from] sqlx::Error),
    /// A message has two handlers.
    #[error(transparent)]
    DuplicateHandler(#[from] DuplicateHandler),
}

impl Adapters {
    /// The production wiring: a lazy connection pool, an in-process cache behind the cache
    /// probe, the SQS client, the in-process event bus and the three readers of the event topic.
    ///
    /// Nothing here touches the network; the first connection is made by the first request that
    /// needs it, and each reader of the topic connects when it is first called.
    ///
    /// # Errors
    ///
    /// Returns [`BootstrapError::Database`] when the database URL cannot be parsed.
    pub async fn connect(config: &Config) -> Result<Self, BootstrapError> {
        let pool = connect_lazy(&config.database)?;
        let sqs = sqs_client(&config.sqs).await;
        let cache = Arc::new(MokaCache::new());
        let topic = KafkaTopic::new(config.kafka.clone());
        Ok(Self {
            repository: Arc::new(PgExampleApiResourceRepository::new(pool.clone())),
            probes: vec![
                Arc::new(PostgresProbe::new(pool)),
                Arc::new(CacheProbe::new(cache)),
                Arc::new(SqsProbe::new(sqs)),
            ],
            event_bus: event_bus(),
            clock: Arc::new(SystemClock),
            uuid_generator: Arc::new(UuidV7Generator),
            topic_probe: Arc::new(topic.probe()),
            topic_tail: Box::new(topic.tail()),
            topic_replay: Arc::new(topic.replay()),
        })
    }

    /// Replaces the probe of the same dependency; the order and the other probes are kept.
    #[must_use]
    pub fn with_probe(mut self, probe: Arc<dyn HealthProbe>) -> Self {
        let dependency = probe.dependency();
        let position = self
            .probes
            .iter()
            .position(|slot| slot.dependency() == dependency);
        if let Some(slot) = position.and_then(|index| self.probes.get_mut(index)) {
            *slot = probe;
        }
        self
    }
}

/// The in-process event bus with every subscriber registered.
#[must_use]
pub fn event_bus() -> Arc<dyn EventBus> {
    Arc::new(InMemoryEventBus::new().subscribe(AuditExampleApiResourceCreated))
}

/// Registers the command handlers of the example context.
fn register_commands(
    bus: InMemoryCommandBus,
    create: CreateExampleApiResourceHandler,
) -> Result<InMemoryCommandBus, BootstrapError> {
    Ok(bus.register(create)?)
}

/// Registers the query handlers of the example context and of the event stream.
fn register_queries(
    bus: InMemoryQueryBus,
    repository: &Arc<dyn ExampleApiResourceRepository>,
    catalog: &EventCatalog,
) -> Result<InMemoryQueryBus, BootstrapError> {
    let get = GetExampleApiResourceHandler::new(Arc::clone(repository));
    let list = ListExampleApiResourcesHandler::new(Arc::clone(repository));
    let event_types = ListEventTypesHandler::new(Arc::new(catalog.clone()));
    Ok(bus.register(get)?.register(list)?.register(event_types)?)
}

/// The state of the event stream routes; the subscriptions replay through `replay`.
///
/// Until the tail is wired (story 3.8) the status and the subscriptions read a hub that is never
/// started and a tail state that stays `TailState::default()`, so the status reports `starting`
/// with `catching-up`, the streaming routes answer the 503 problem `the node is starting` and no
/// broker is contacted.
fn event_stream_state(
    query_bus: Arc<dyn QueryBus>,
    streams: &StreamConfig,
    replay: Arc<dyn TopicReplay>,
) -> EventStreamState {
    let window = Arc::new(Window::new(
        streams.window_max_events,
        streams.window_max_bytes,
    ));
    let hub = Hub::new(HubSettings {
        max_subscribers: streams.max_subscribers,
        buffer: streams.buffer,
    });
    let (_tail_sender, tail_state) = watch::channel(TailState::default());
    let subscriptions = Arc::new(Subscriptions::new(
        Arc::<Hub>::clone(&hub),
        Arc::<Window>::clone(&window),
        replay,
        tail_state.clone(),
        ReplaySettings {
            max_events: streams.replay_max_events,
            max_data_bytes: streams.max_data_bytes,
            permits: streams.replay_permits,
        },
    ));
    let status = Arc::new(StatusSource::new(
        tail_state,
        window,
        Arc::<Hub>::clone(&hub),
    ));
    EventStreamState {
        query_bus,
        subscriptions,
        status,
        hub,
        settings: StreamSettings {
            keep_alive: streams.keep_alive,
        },
    }
}

/// Registers every handler, builds the command and query buses, the state, the `GraphQL` schema
/// and the router with its middleware.
///
/// # Errors
///
/// Returns [`BootstrapError::DuplicateHandler`] when a message has two handlers.
pub fn router(adapters: Adapters, config: &Config) -> Result<Router, BootstrapError> {
    let create = CreateExampleApiResourceHandler::new(
        Arc::clone(&adapters.repository),
        adapters.event_bus,
        adapters.clock,
        Arc::clone(&adapters.uuid_generator),
    );
    let command_bus = register_commands(InMemoryCommandBus::new(), create)?;
    let catalog = &config.streams.catalog;
    let query_bus = register_queries(InMemoryQueryBus::new(), &adapters.repository, catalog)?;
    let commands: Arc<dyn CommandBus> = Arc::new(command_bus);
    let queries: Arc<dyn QueryBus> = Arc::new(query_bus);
    let event_stream =
        event_stream_state(Arc::clone(&queries), &config.streams, adapters.topic_replay);
    let some_module = SomeModuleState {
        command_bus: commands,
        query_bus: queries,
        uuid_generator: adapters.uuid_generator,
    };
    let service = HealthService::new(adapters.probes, config.health_probe_timeout);
    let health_check = HealthCheckState {
        service: Arc::new(service),
    };
    let schema = schema::builder()
        .data(some_module.clone())
        .data(event_stream.clone())
        .finish();
    let state = AppState {
        health_check,
        some_module,
        event_stream,
    };
    Ok(assemble(state, schema, config))
}

#[cfg(test)]
mod tests;
