//! Unit tests of the parent module, against the in-memory tree.

use std::path::Path;

use super::memory::Memory;
use super::{Failure, evaluate, list, run};

const LIST: &str = "approval_label = \"approved-config-change\"\nfiles = [\"clippy.toml\", \"deny.toml\"]\nrequired_directories = [\"crates\"]\n";

fn tree() -> Memory {
    Memory::default()
        .same("protected-config.toml", LIST)
        .same("clippy.toml", "a")
        .same("deny.toml", "a")
        .directory("crates")
}

fn report(tree: &Memory, approved: bool) -> (bool, String) {
    let outcome = evaluate(tree, approved).unwrap();
    (outcome.clean, outcome.report)
}

#[test]
fn an_unchanged_tree_passes() {
    for approved in [false, true] {
        assert_eq!(
            report(&tree(), approved),
            (
                true,
                "Configuration Validation\n[OK] Configuration validation passed".to_owned()
            )
        );
    }
}

#[test]
fn a_modification_fails_and_names_the_label() {
    let tree = tree().in_work("clippy.toml", "b");

    assert_eq!(
        report(&tree, false),
        (
            false,
            "Configuration Validation\n\
             Modification of locked configuration file is not allowed: clippy.toml\n\
             [FAIL] Configuration validation failed\n\
             Approve with the label approved-config-change"
                .to_owned()
        )
    );
}

#[test]
fn the_label_of_the_list_is_the_one_that_is_printed() {
    let list = LIST.replace("approved-config-change", "another-label");
    let tree = tree()
        .in_work("protected-config.toml", &list)
        .on_base("protected-config.toml", &list)
        .in_work("clippy.toml", "b");

    let (clean, text) = report(&tree, false);

    assert!(!clean);
    assert!(
        text.ends_with("\nApprove with the label another-label"),
        "{text}"
    );
    let (clean, text) = report(&tree, true);
    assert!(clean);
    assert!(
        text.ends_with("\n[OK] Configuration validation passed (approved by label another-label)"),
        "{text}"
    );
}

#[test]
fn the_approval_waives_modifications_and_lists_them_as_approved() {
    let tree = tree().in_work("clippy.toml", "b").in_work("deny.toml", "c");

    assert_eq!(
        report(&tree, true),
        (
            true,
            "Configuration Validation\n\
             [APPROVED] Modification of locked configuration file is not allowed: clippy.toml\n\
             [APPROVED] Modification of locked configuration file is not allowed: deny.toml\n\
             [OK] Configuration validation passed (approved by label approved-config-change)"
                .to_owned()
        )
    );
}

#[test]
fn a_missing_required_directory_fails_with_and_without_the_approval() {
    let without = Memory::default()
        .same("protected-config.toml", LIST)
        .same("clippy.toml", "a")
        .same("deny.toml", "a");

    let alone = "Configuration Validation\n\
                 Required directory is missing: crates\n\
                 [FAIL] Configuration validation failed\n\
                 Approve with the label approved-config-change"
        .to_owned();
    assert_eq!(report(&without, false), (false, alone.clone()));
    assert_eq!(report(&without, true), (false, alone));

    let with_change = without.in_work("clippy.toml", "b");
    let (clean, text) = report(&with_change, true);
    assert!(!clean);
    assert_eq!(
        text,
        "Configuration Validation\n\
         [APPROVED] Modification of locked configuration file is not allowed: clippy.toml\n\
         Required directory is missing: crates\n\
         [FAIL] Configuration validation failed\n\
         Approve with the label approved-config-change"
    );
}

#[test]
fn the_list_of_the_base_ref_decides_so_that_removing_an_entry_hides_nothing() {
    let shorter = "approval_label = \"approved-config-change\"\nfiles = [\"deny.toml\"]\n";
    let tree = Memory::default()
        .on_base("protected-config.toml", LIST)
        .in_work("protected-config.toml", shorter)
        .same("deny.toml", "a")
        .on_base("clippy.toml", "a")
        .directory("crates");

    let (clean, text) = report(&tree, false);

    assert!(!clean);
    assert!(
        text.contains("file is not allowed: clippy.toml\n"),
        "{text}"
    );
}

#[test]
fn without_a_list_on_the_base_ref_the_list_of_the_working_tree_is_used() {
    let tree = Memory::default()
        .in_work("protected-config.toml", LIST)
        .in_work("clippy.toml", "a")
        .in_work("crates/x/clippy.toml", "a");

    assert_eq!(
        report(&tree, false),
        (
            false,
            "Configuration Validation\n\
             Required directory is missing: crates\n\
             [FAIL] Configuration validation failed\n\
             Approve with the label approved-config-change"
                .to_owned()
        )
    );
}

#[test]
fn a_run_without_any_list_or_with_an_invalid_one_could_not_run() {
    let none = evaluate(&Memory::default(), false).unwrap_err();
    assert!(matches!(none, Failure::NoList));
    assert_eq!(
        none.to_string(),
        "protected-config.toml is neither in the base ref nor in the working tree"
    );

    let invalid = Memory::default().on_base("protected-config.toml", "files = 3");
    let error = evaluate(&invalid, false).unwrap_err();
    assert!(matches!(error, Failure::InvalidList(_)));
}

#[test]
fn a_failure_of_a_check_is_passed_on() {
    let tree = tree().on_base("Cargo.toml", "workspace = [");
    let text = format!(
        "{LIST}manifest_tables = [{{ file = \"Cargo.toml\", table = \"workspace.lints\" }}]\n"
    );

    let error = evaluate(&tree.on_base("protected-config.toml", &text), false).unwrap_err();

    assert!(matches!(error, Failure::InvalidBase { .. }));
}

#[test]
fn the_items_of_the_list_of_the_repository_are_printed_one_per_line() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();

    let outcome = list(root).unwrap();

    assert!(outcome.clean);
    let lines: Vec<&str> = outcome.report.lines().collect();
    assert_eq!(lines.first(), Some(&".cargo/config.toml"));
    assert!(lines.contains(&"xtask/tests/"));
    assert!(lines.contains(&"Cargo.toml#workspace.lints"));
    assert!(lines.contains(&"tests/Load/config.json.dist#/endpoints/health/smoke/threshold"));
    assert!(!outcome.report.ends_with('\n'));
}

#[test]
fn a_missing_list_or_base_ref_could_not_run() {
    let nowhere = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    let error = list(&nowhere).unwrap_err();
    assert!(matches!(error, Failure::ReadList(_)));
    assert!(
        error
            .to_string()
            .starts_with("cannot read protected-config.toml: "),
        "{error}"
    );
    let error = run(&std::env::temp_dir(), "no-such-ref", false).unwrap_err();
    assert_eq!(error.to_string(), "base ref no-such-ref is not available");
}
