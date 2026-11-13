//! Log capture for tests: a minimal `tracing` subscriber that records the events of one thread.

use std::sync::{Arc, Mutex, PoisonError};

use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::DefaultGuard;
use tracing::{Event, Level, Metadata, Subscriber};

/// One recorded event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// Level of the event.
    pub level: Level,
    /// Target of the event.
    pub target: String,
    /// The message of the event, empty when it has none.
    pub message: String,
    /// The other fields as `(name, value)` pairs in declaration order.
    pub fields: Vec<(String, String)>,
}

impl LogLine {
    /// The value of the field `name`, if the event has it.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(field, _value)| field == name)
            .map(|(_field, value)| value.as_str())
    }
}

/// Records every event of the current thread while the guard of [`LogRecorder::install`] lives.
#[derive(Debug, Clone, Default)]
pub struct LogRecorder(Arc<Mutex<Vec<LogLine>>>);

#[derive(Default)]
struct Collected {
    message: String,
    fields: Vec<(String, String)>,
}

impl Collected {
    fn push(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = value;
        } else {
            self.fields.push((field.name().to_owned(), value));
        }
    }
}

impl Visit for Collected {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.push(field, value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.push(field, format!("{value:?}"));
    }
}

impl LogRecorder {
    /// Installs a recorder as the default subscriber of this thread; events of other threads are
    /// not recorded.
    #[must_use]
    pub fn install() -> (Self, DefaultGuard) {
        let recorder = Self::default();
        let guard = tracing::subscriber::set_default(recorder.clone());
        (recorder, guard)
    }

    /// The recorded events, in order.
    #[must_use]
    pub fn lines(&self) -> Vec<LogLine> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Subscriber for LogRecorder {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}

    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut collected = Collected::default();
        event.record(&mut collected);
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(LogLine {
                level: *event.metadata().level(),
                target: event.metadata().target().to_owned(),
                message: collected.message,
                fields: collected.fields,
            });
    }

    fn enter(&self, _span: &Id) {}

    fn exit(&self, _span: &Id) {}
}

#[cfg(test)]
mod tests;
