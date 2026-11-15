//! The test installs the global subscriber, so it relies on nextest giving every test its own
//! process.

use std::io;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use super::init;

/// An in-memory log sink that the test reads back.
#[derive(Debug, Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .map_err(|_poisoned| io::ErrorKind::Other)?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn the_kafka_client_writes_errors_only_whatever_the_level() {
    let output = Captured::default();
    let sink = output.clone();
    init("debug", move || sink.clone()).unwrap();

    tracing::warn!(target: "krafka::metadata", "client warning");
    tracing::info!(target: "krafka", "client information");
    tracing::error!(target: "krafka::consumer", "client error");
    tracing::debug!(target: "service::background", "own debug entry");

    let captured = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    let entries: Vec<(String, String)> = captured
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .map(|line| {
            (
                line["target"].as_str().unwrap().to_owned(),
                line["message"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        entries,
        [
            ("krafka::consumer".to_owned(), "client error".to_owned()),
            (
                "service::background".to_owned(),
                "own debug entry".to_owned()
            ),
        ],
        "{captured}"
    );
}
