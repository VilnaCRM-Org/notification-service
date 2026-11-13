use std::error::Error as _;
use std::time::Duration;

use super::{HealthReport, ProbeFailure, ProbeFailureCause};
use crate::value_object::dependency::Dependency;

#[derive(Debug, thiserror::Error)]
#[error("connection refused")]
struct Refused;

fn failure(dependency: Dependency) -> ProbeFailure {
    ProbeFailure {
        dependency,
        cause: ProbeFailureCause::TimedOut(Duration::from_secs(3)),
    }
}

#[test]
fn a_report_without_failures_is_healthy() {
    assert!(HealthReport::new(Vec::new()).is_healthy());
    assert!(HealthReport::default().is_healthy());
}

#[test]
fn a_report_with_a_failure_is_not_healthy() {
    assert!(!HealthReport::new(vec![failure(Dependency::Cache)]).is_healthy());
}

#[test]
fn a_report_keeps_the_failures_in_the_order_given() {
    let report = HealthReport::new(vec![
        failure(Dependency::Database),
        failure(Dependency::MessageBroker),
    ]);

    let dependencies: Vec<_> = report
        .failures()
        .iter()
        .map(|failure| failure.dependency)
        .collect();
    assert_eq!(
        dependencies,
        vec![Dependency::Database, Dependency::MessageBroker]
    );
}

#[test]
fn a_failed_cause_names_the_error_and_keeps_it_as_its_source() {
    let cause = ProbeFailureCause::Failed(Box::new(Refused));

    assert_eq!(cause.to_string(), "probe failed");
    assert_eq!(cause.source().unwrap().to_string(), "connection refused");
}

#[test]
fn a_timed_out_cause_names_the_deadline() {
    let cause = ProbeFailureCause::TimedOut(Duration::from_secs(3));

    assert_eq!(cause.to_string(), "probe timed out after 3s");
    assert!(cause.source().is_none());
}
