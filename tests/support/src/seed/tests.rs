use rand::RngExt as _;

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};

use super::{PanicReport, TestRng, parse_seed};

fn draw_ten<W: Write>(rng: &mut TestRng<W>) -> Vec<u64> {
    (0..10).map(|_index| rng.rng().random::<u64>()).collect()
}

#[test]
fn two_generators_with_the_same_seed_draw_the_same_values() {
    let mut first = TestRng::from_seed(42);
    let mut second = TestRng::from_seed(42);

    assert_eq!(draw_ten(&mut first), draw_ten(&mut second));
}

#[test]
fn generators_with_different_seeds_draw_different_values() {
    let mut first = TestRng::from_seed(42);
    let mut second = TestRng::from_seed(43);

    assert_ne!(draw_ten(&mut first), draw_ten(&mut second));
}

#[test]
fn from_env_is_reproducible_and_reports_its_seed() {
    let mut first = TestRng::from_env();
    let mut second = TestRng::from_env();

    assert_eq!(first.seed(), second.seed());
    assert_eq!(draw_ten(&mut first), draw_ten(&mut second));
}

#[test]
fn a_usable_value_of_the_variable_is_the_seed() {
    assert_eq!(TestRng::from_raw_seed(Some(" 42\n")).seed(), 42);
}

#[test]
fn an_unusable_value_of_the_variable_panics_with_the_message_that_names_it() {
    for (raw, expected) in [
        (
            None,
            "TEST_SEED is not set; run through cargo or set TEST_SEED=<unsigned integer>",
        ),
        (Some("abc"), "TEST_SEED must be an unsigned 64-bit integer"),
    ] {
        let payload = std::panic::catch_unwind(|| TestRng::from_raw_seed(raw)).unwrap_err();

        assert_eq!(
            payload.downcast_ref::<String>().map(String::as_str),
            Some(expected)
        );
    }
}

#[test]
fn parses_an_unsigned_integer() {
    assert_eq!(parse_seed(Some("42")), Ok(42));
    assert_eq!(parse_seed(Some(" 7\n")), Ok(7));
    assert_eq!(parse_seed(Some("18446744073709551615")), Ok(u64::MAX));
}

#[test]
fn a_missing_seed_names_the_variable() {
    let message = parse_seed(None).unwrap_err();

    assert!(message.contains("TEST_SEED"));
}

#[test]
fn an_invalid_seed_names_the_variable() {
    for raw in ["", "abc", "-1", "1.5", "18446744073709551616"] {
        let message = parse_seed(Some(raw)).unwrap_err();

        assert!(message.contains("TEST_SEED"), "{raw}");
    }
}

/// A writer that keeps what is written, so that a test can read it after the report was dropped.
#[derive(Debug, Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Shared {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn the_report_is_the_seed_line() {
    let out = Shared::default();
    let mut report = PanicReport::new(7, out.clone());

    report.report().unwrap();

    assert_eq!(out.text(), "TEST_SEED=7\n");
}

#[test]
fn a_panic_in_the_scope_of_the_report_writes_the_seed_line() {
    let out = Shared::default();
    let sink = out.clone();

    let result = std::panic::catch_unwind(move || {
        let _report = PanicReport::new(1234, sink);
        std::panic::panic_any("a test failure");
    });

    assert!(result.is_err());
    assert_eq!(out.text(), "TEST_SEED=1234\n");
}

#[test]
fn a_scope_that_ends_without_a_panic_writes_nothing() {
    let out = Shared::default();
    let sink = out.clone();

    let result = std::panic::catch_unwind(move || {
        let _report = PanicReport::new(1234, sink);
    });

    assert!(result.is_ok());
    assert_eq!(out.text(), "");
}

#[test]
fn the_generator_reports_its_own_seed() {
    let rng = TestRng::from_seed(7);

    assert_eq!(rng.seed(), 7);
}

#[test]
fn dropping_while_panicking_does_not_abort() {
    let result = std::panic::catch_unwind(|| {
        let _rng = TestRng::from_seed(7);
        std::panic::panic_any("a test failure");
    });

    assert!(result.is_err());
}

#[test]
fn a_generator_alive_at_the_panic_writes_its_seed_line() {
    let out = Shared::default();
    let sink = out.clone();

    let result = std::panic::catch_unwind(move || {
        let mut rng = TestRng::with_report(4321, sink);
        let drawn = draw_ten(&mut rng);
        assert!(drawn.is_empty(), "a test failure after the data was drawn");
    });

    assert!(result.is_err());
    assert_eq!(out.text(), "TEST_SEED=4321\n");
}
