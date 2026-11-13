//! Shared kernel, domain layer: the aggregate and event abstractions, the command, query and event
//! bus ports, the technical ports (cache, clock, UUID generator) and the value objects that every
//! bounded context uses.
//!
//! This crate is framework-free: it depends on other domain crates and on the few third-party crates
//! that `architecture.toml` lists for the domain layer, and on nothing else.

pub mod aggregate;
pub mod bus;
pub mod error;
pub mod event;
pub mod port;
pub mod value_object;

pub use aggregate::{AggregateRoot, RecordedEvents};
pub use bus::command::{Command, CommandBus, CommandHandler};
pub use bus::error::{BusError, ErasedBusError, ErasedMessage};
pub use bus::event::{EventBus, EventBusError, SubscriberFailure};
pub use bus::query::{Query, QueryBus, QueryHandler};
pub use error::{BoxError, error_chain};
pub use event::{DomainEvent, DomainEventSubscriber};
pub use port::cache::{Cache, CacheError};
pub use port::clock::Clock;
pub use port::uuid_generator::UuidGenerator;
pub use value_object::page_request::{InvalidPageRequest, PageRequest};
pub use value_object::uuid::{InvalidUuid, Uuid};
