//! Bounded context `event-stream`, infrastructure layer: the adapters that implement the ports of the
//! domain layer, such as persistence, cache and messaging.
//!
//! This crate depends on the domain layer of its own context and of the shared kernel, never on the
//! application layer. The Kafka client lives here and nowhere else.

pub mod kafka;

pub use kafka::{
    KafkaProbe, KafkaReplay, KafkaSecurity, KafkaSettings, KafkaTail, KafkaTopic, SaslCredentials,
    SaslMechanism,
};
