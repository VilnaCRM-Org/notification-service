use std::sync::Arc;
use std::time::Duration;

use health_check_domain::{Dependency, HealthProbe, HealthReport, ProbeFailureCause};
use rstest::rstest;
use shared_domain::BoxError;
use test_support::log::LogRecorder;
use test_support::probe::StubProbe;
use tokio::time::Instant;
use tracing::Level;

use super::HealthService;

const TIMEOUT: Duration = Duration::from_secs(3);

const ALL: [Dependency; 3] = [
    Dependency::Database,
    Dependency::Cache,
    Dependency::MessageBroker,
];

fn service(probes: Vec<StubProbe>) -> HealthService {
    let probes = probes
        .into_iter()
        .map(|probe| -> Arc<dyn HealthProbe> { Arc::new(probe) })
        .collect();
    HealthService::new(probes, TIMEOUT)
}

fn probes_failing(failing: &[Dependency]) -> Vec<StubProbe> {
    ALL.into_iter()
        .map(|dependency| {
            if failing.contains(&dependency) {
                StubProbe::failing(dependency)
            } else {
                StubProbe::healthy(dependency)
            }
        })
        .collect()
}

fn failed(report: &HealthReport) -> Vec<Dependency> {
    report
        .failures()
        .iter()
        .map(|failure| failure.dependency)
        .collect()
}

#[tokio::test]
async fn all_probes_healthy_gives_a_healthy_report() {
    let report = service(probes_failing(&[])).check().await;

    assert!(report.is_healthy());
    assert_eq!(failed(&report), Vec::<Dependency>::new());
}

#[tokio::test]
async fn a_service_without_probes_is_healthy() {
    assert!(service(Vec::new()).check().await.is_healthy());
}

#[rstest]
#[case::database(&[Dependency::Database])]
#[case::cache(&[Dependency::Cache])]
#[case::message_broker(&[Dependency::MessageBroker])]
#[case::database_and_cache(&[Dependency::Database, Dependency::Cache])]
#[case::database_and_message_broker(&[Dependency::Database, Dependency::MessageBroker])]
#[case::cache_and_message_broker(&[Dependency::Cache, Dependency::MessageBroker])]
#[case::all(&[Dependency::Database, Dependency::Cache, Dependency::MessageBroker])]
#[tokio::test]
async fn the_report_holds_exactly_the_failing_dependencies_in_registration_order(
    #[case] failing: &[Dependency],
) {
    let report = service(probes_failing(failing)).check().await;

    assert!(!report.is_healthy());
    assert_eq!(failed(&report), failing);
}

#[tokio::test]
async fn failures_follow_the_registration_order_not_the_dependency_order() {
    let report = service(vec![
        StubProbe::failing(Dependency::MessageBroker),
        StubProbe::healthy(Dependency::Cache),
        StubProbe::failing(Dependency::Database),
    ])
    .check()
    .await;

    assert_eq!(
        failed(&report),
        vec![Dependency::MessageBroker, Dependency::Database]
    );
}

#[tokio::test]
async fn a_failed_probe_keeps_its_error_as_the_cause() {
    let report = service(probes_failing(&[Dependency::Cache])).check().await;

    let cause = &report.failures()[0].cause;
    assert!(matches!(cause, ProbeFailureCause::Failed(_)));
    assert_eq!(cause.to_string(), "probe failed");
}

#[tokio::test(start_paused = true)]
async fn a_hanging_probe_times_out_and_counts_as_failed() {
    let started = Instant::now();

    let report = service(vec![
        StubProbe::healthy(Dependency::Database),
        StubProbe::hanging(Dependency::Cache),
        StubProbe::healthy(Dependency::MessageBroker),
    ])
    .check()
    .await;

    assert_eq!(started.elapsed(), TIMEOUT);
    assert_eq!(failed(&report), vec![Dependency::Cache]);
    assert!(matches!(
        report.failures()[0].cause,
        ProbeFailureCause::TimedOut(timeout) if timeout == TIMEOUT
    ));
}

#[tokio::test(start_paused = true)]
async fn probes_run_concurrently() {
    let started = Instant::now();

    let report = service(vec![
        StubProbe::hanging(Dependency::Database),
        StubProbe::hanging(Dependency::Cache),
        StubProbe::hanging(Dependency::MessageBroker),
    ])
    .check()
    .await;

    assert_eq!(started.elapsed(), TIMEOUT);
    assert_eq!(failed(&report), ALL);
}

#[tokio::test(start_paused = true)]
async fn each_failure_is_logged_once_at_warn_with_its_dependency_and_cause() {
    let (recorder, _guard) = LogRecorder::install();

    let _report = service(vec![
        StubProbe::failing(Dependency::Database),
        StubProbe::healthy(Dependency::Cache),
        StubProbe::hanging(Dependency::MessageBroker),
    ])
    .check()
    .await;

    let lines = recorder.lines();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].level, Level::WARN);
    assert_eq!(lines[0].target, "health_check_application::service");
    assert_eq!(lines[0].message, "health probe failed");
    assert_eq!(lines[0].field("dependency"), Some("database"));
    assert_eq!(
        lines[0].field("cause"),
        Some("probe failed: stub probe failure")
    );
    assert_eq!(lines[1].level, Level::WARN);
    assert_eq!(lines[1].message, "health probe failed");
    assert_eq!(lines[1].field("dependency"), Some("message_broker"));
    assert_eq!(lines[1].field("cause"), Some("probe timed out after 3s"));
}

#[tokio::test]
async fn a_healthy_check_logs_nothing() {
    let (recorder, _guard) = LogRecorder::install();

    let _report = service(probes_failing(&[])).check().await;

    assert_eq!(recorder.lines(), Vec::new());
}

/// An error whose text is generic and whose reason sits in its source, like an SDK error.
#[derive(Debug)]
struct Layered(&'static str, Option<Box<Self>>);

impl std::fmt::Display for Layered {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl std::error::Error for Layered {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.1
            .as_deref()
            .map(|source| -> &(dyn std::error::Error + 'static) { source })
    }
}

#[derive(Debug)]
struct LayeredProbe;

#[async_trait::async_trait]
impl HealthProbe for LayeredProbe {
    fn dependency(&self) -> Dependency {
        Dependency::MessageBroker
    }

    async fn check(&self) -> Result<(), BoxError> {
        Err(Box::new(Layered(
            "dispatch failure",
            Some(Box::new(Layered("connection refused", None))),
        )))
    }
}

#[tokio::test]
async fn the_log_holds_the_whole_error_chain_of_a_failed_probe() {
    let (recorder, _guard) = LogRecorder::install();
    let probe: Arc<dyn HealthProbe> = Arc::new(LayeredProbe);

    let _report = HealthService::new(vec![probe], TIMEOUT).check().await;

    let lines = recorder.lines();
    assert_eq!(lines.len(), 1);
    assert_eq!(
        lines[0].field("cause"),
        Some("probe failed: dispatch failure: connection refused")
    );
}
