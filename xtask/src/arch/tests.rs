//! Unit tests of the parent module, against the fixture workspace and the fixture models.

use std::path::{Path, PathBuf};

use super::{Failure, Options, run};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/arch");

fn options(config: &str, github: bool) -> Options {
    Options {
        config: Path::new(FIXTURES).join(config),
        manifest: Path::new(FIXTURES).join("workspace/Cargo.toml"),
        github,
    }
}

#[test]
fn a_workspace_with_violations_is_reported_and_not_clean() {
    let outcome = run(&options("valid.toml", false)).unwrap();

    assert!(!outcome.clean);
    assert!(
        outcome.report.contains("error[arch::layer]: `shop-domain`"),
        "{}",
        outcome.report
    );
    assert!(
        outcome
            .report
            .ends_with("architecture: FAILED. 9 violation(s) in 14 crate(s), 5 layer(s) checked"),
        "{}",
        outcome.report
    );
    assert!(!outcome.report.contains("::error"), "{}", outcome.report);
}

#[test]
fn the_github_option_adds_the_annotations() {
    let outcome = run(&options("valid.toml", true)).unwrap();

    assert_eq!(
        outcome.report.matches("\n::error file=").count(),
        9,
        "{}",
        outcome.report
    );
}

#[test]
fn a_model_that_covers_nothing_leaves_every_crate_uncovered() {
    let outcome = run(&options("nothing-covered.toml", false)).unwrap();

    assert!(!outcome.clean);
    assert_eq!(
        outcome.report.matches("error[arch::uncovered]").count(),
        14,
        "{}",
        outcome.report
    );
}

#[test]
fn an_unreadable_model_could_not_run() {
    let failure = run(&options("no-such-model.toml", false)).unwrap_err();

    assert!(matches!(failure, Failure::Read { .. }), "{failure}");
}

#[test]
fn an_unreadable_workspace_could_not_run() {
    let mut options = options("valid.toml", false);
    options.manifest = PathBuf::from(FIXTURES).join("no-such-workspace/Cargo.toml");

    let failure = run(&options).unwrap_err();

    assert!(matches!(failure, Failure::Metadata(_)), "{failure}");
}
