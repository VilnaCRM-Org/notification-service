//! The settings of the Kafka adapter: plain data that the composition root parses from the
//! environment. The adapter itself is added with the Kafka client.

use std::fmt;
use std::time::Duration;

/// The SASL mechanism of `KAFKA_SASL_MECHANISM`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaslMechanism {
    /// `PLAIN`.
    Plain,
    /// `SCRAM-SHA-256`.
    ScramSha256,
    /// `SCRAM-SHA-512`.
    ScramSha512,
}

/// The SASL credentials of a connection. The `Debug` output prints the password as
/// `<redacted>`, so a logged or printed value never holds it.
#[derive(Clone, PartialEq, Eq)]
pub struct SaslCredentials {
    /// The mechanism.
    pub mechanism: SaslMechanism,
    /// The user name (`KAFKA_SASL_USERNAME`).
    pub username: String,
    /// The password (`KAFKA_SASL_PASSWORD`); never logged.
    pub password: String,
}

impl fmt::Debug for SaslCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SaslCredentials")
            .field("mechanism", &self.mechanism)
            .field("username", &self.username)
            .field("password", &format_args!("<redacted>"))
            .finish()
    }
}

/// The security protocol of `KAFKA_SECURITY_PROTOCOL`, with the credentials of the SASL ones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KafkaSecurity {
    /// `plaintext`.
    Plaintext,
    /// `ssl`.
    Ssl,
    /// `sasl_plaintext`.
    SaslPlaintext(SaslCredentials),
    /// `sasl_ssl`.
    SaslSsl(SaslCredentials),
}

/// The connection settings of every Kafka client of the node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KafkaSettings {
    /// The validated `host:port` entries of `KAFKA_BOOTSTRAP_SERVERS`, joined with `,`.
    pub bootstrap_servers: String,
    /// The topic (`KAFKA_TOPIC`).
    pub topic: String,
    /// The client id of every connection (`KAFKA_CLIENT_ID`).
    pub client_id: String,
    /// The security protocol and its credentials.
    pub security: KafkaSecurity,
    /// The request time-out of the client (`KAFKA_REQUEST_TIMEOUT`).
    pub request_timeout: Duration,
    /// The connect time-out of the client (`KAFKA_CONNECT_TIMEOUT`), at most the request time-out.
    pub connect_timeout: Duration,
    /// The long-poll wait of the tail (`KAFKA_FETCH_MAX_WAIT`).
    pub fetch_wait: Duration,
}

#[cfg(test)]
mod tests;
