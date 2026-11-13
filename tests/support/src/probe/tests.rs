use std::panic::{AssertUnwindSafe, catch_unwind};
use std::task::{Context, Poll, Waker};

use health_check_domain::{Dependency, HealthProbe as _};

use super::StubProbe;
use crate::block_on;

#[test]
fn a_probe_reports_its_dependency() {
    assert_eq!(
        StubProbe::healthy(Dependency::Cache).dependency(),
        Dependency::Cache
    );
    assert_eq!(
        StubProbe::failing(Dependency::Database).dependency(),
        Dependency::Database
    );
}

#[test]
fn the_healthy_probe_succeeds() {
    let probe = StubProbe::healthy(Dependency::Database);

    assert!(block_on(probe.check()).is_ok());
}

#[test]
fn the_failing_probe_returns_an_error() {
    let probe = StubProbe::failing(Dependency::Database);

    assert_eq!(
        block_on(probe.check()).unwrap_err().to_string(),
        "stub probe failure"
    );
}

#[test]
fn the_hanging_probe_never_completes() {
    let probe = StubProbe::hanging(Dependency::MessageBroker);
    let mut future = std::pin::pin!(probe.check());
    let mut context = Context::from_waker(Waker::noop());

    for _ in 0..3 {
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
    }
}

#[test]
fn the_panicking_probe_panics() {
    let probe = StubProbe::panicking(Dependency::Cache);

    let outcome = catch_unwind(AssertUnwindSafe(|| block_on(probe.check())));

    assert!(outcome.is_err());
}
