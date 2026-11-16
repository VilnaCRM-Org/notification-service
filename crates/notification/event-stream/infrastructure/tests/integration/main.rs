//! Integration tests of the Kafka adapter against the broker of the test stack. Every test uses a
//! topic of its own, so the tests run in parallel against one broker.

#[cfg(test)]
mod real_broker;
#[cfg(test)]
mod support;
