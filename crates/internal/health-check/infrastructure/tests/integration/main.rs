//! Integration tests of the probes against the test stack (PostgreSQL and the SQS emulator).

#[cfg(test)]
mod postgres_probe;
#[cfg(test)]
mod sqs_probe;
#[cfg(test)]
mod support;
