//! The binary against the real workspace (exit 0) and against the fixture workspace (exit 1 and 2).

use std::fs::OpenOptions;
use std::process::Stdio;

use super::support::{fixture, run, workspace_root, xtask};

/// The arguments that check the fixture workspace against `model`.
fn fixture_arguments<'a>(model: &'a str, workspace: &'a str) -> Vec<&'a str> {
    vec!["arch", "--config", model, "--manifest-path", workspace]
}

#[test]
fn the_real_workspace_satisfies_its_architecture_model() {
    let root = workspace_root().display().to_string();
    let (model, manifest) = (
        format!("{root}/architecture.toml"),
        format!("{root}/Cargo.toml"),
    );

    let run = run(&fixture_arguments(&model, &manifest), &[]);

    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    // The count is not pinned: a service that runs `make new-context` adds crates, and this file
    // is protected, so it cannot be edited to follow. The template itself has 13.
    let count = run
        .stdout
        .strip_prefix("architecture: ok. All ")
        .and_then(|rest| {
            rest.strip_suffix(
                " crate(s) are assigned to exactly one layer (7 layer(s)); 0 violation(s)\n",
            )
        })
        .unwrap_or_else(|| panic!("{}", run.stdout));
    assert!(count.parse::<usize>().unwrap() >= 13, "{}", run.stdout);
    assert_eq!(run.stderr, "");
}

#[test]
fn the_fixture_workspace_breaks_one_rule_per_planted_violation() {
    let (model, workspace) = (fixture("valid.toml"), fixture("workspace/Cargo.toml"));

    let run = run(&fixture_arguments(&model, &workspace), &[]);

    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert_eq!(run.stderr, "");
    let stdout = &run.stdout;
    for expected in [
        "error[arch::uncovered]: `stray` is not assigned to any layer\n  --> crates/stray/Cargo.toml\n",
        "error[arch::ambiguous]: `overlap-domain` matches more than one layer: domain, overlap\n  --> crates/overlap/domain/Cargo.toml\n",
        "error[arch::lints]: `nolints-domain` does not inherit the workspace lint policy\n  --> crates/nolints/domain/Cargo.toml\n",
        "error[arch::layer]: `shop-domain` (layer `domain`, context `shop`) must not depend on `shop-infrastructure` (layer `infrastructure`, context `shop`)\n  --> crates/shop/domain/Cargo.toml\n   = help: layer `domain` may depend on: [domain]; ",
        "error[arch::context]: `audit-domain` (layer `domain`, context `audit`) must not depend on `shop-domain` (layer `domain`, context `shop`)\n  --> crates/audit/domain/Cargo.toml\n",
        "error[arch::external]: `ledger-domain` (layer `domain`, context `ledger`) must not depend on the third-party crate `serde`\n  --> crates/ledger/domain/Cargo.toml\n",
        "error[arch::transitive]: `reports-application` (layer `application`, context `reports`) reaches `sqlx` through non-dev dependencies: `reports-application` -> `axum` -> `sqlx`\n  --> crates/reports/application/Cargo.toml\n",
        "error[arch::path]: `misplaced-domain` (layer `domain`, context `misplaced`) is not at `crates/[<group>/]misplaced/domain` but at `crates/misplaced/core`\n  --> crates/misplaced/core/Cargo.toml\n",
    ] {
        assert!(
            stdout.contains(expected),
            "missing {expected:?} in\n{stdout}"
        );
    }
    assert_eq!(stdout.matches("error[arch::").count(), 9, "{stdout}");
    assert!(
        stdout
            .ends_with("architecture: FAILED. 9 violation(s) in 14 crate(s), 5 layer(s) checked\n"),
        "{stdout}"
    );
}

#[test]
fn a_dev_dependency_on_a_dev_layer_is_not_reported_but_the_same_normal_dependency_is() {
    let (model, workspace) = (fixture("valid.toml"), fixture("workspace/Cargo.toml"));

    let run = run(&fixture_arguments(&model, &workspace), &[]);

    assert!(
        !run.stdout.contains("`billing-application`"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains(
            "error[arch::layer]: `shop-application` (layer `application`, context `shop`) must not depend on `test-support` (layer `test-support`)"
        ),
        "{}",
        run.stdout
    );
}

#[test]
fn github_actions_adds_one_annotation_per_violation() {
    let (model, workspace) = (fixture("valid.toml"), fixture("workspace/Cargo.toml"));
    let arguments = fixture_arguments(&model, &workspace);

    let annotated = run(&arguments, &[("GITHUB_ACTIONS", "true")]);
    let plain = run(&arguments, &[]);
    let disabled = run(&arguments, &[("GITHUB_ACTIONS", "false")]);

    assert_eq!(annotated.code, Some(1));
    let annotations: Vec<&str> = annotated
        .stdout
        .lines()
        .filter(|line| line.starts_with("::error "))
        .collect();
    assert_eq!(annotations.len(), 9, "{}", annotated.stdout);
    assert!(
        annotations.contains(&"::error file=crates/stray/Cargo.toml,title=Architecture (uncovered)::`stray` is not assigned to any layer"),
        "{annotations:?}"
    );
    // Without the variable, or with another value, the same report is printed and no annotation is added.
    for (name, run) in [("plain", &plain), ("disabled", &disabled)] {
        assert_eq!(run.code, Some(1), "{name}: {}", run.stdout);
        assert!(
            run.stdout.contains(
                "error[arch::uncovered]: `stray` is not assigned to any layer\n  --> crates/stray/Cargo.toml\n"
            ),
            "{name}: {}",
            run.stdout
        );
        assert!(!run.stdout.contains("::error"), "{name}: {}", run.stdout);
    }
}

#[test]
fn a_model_with_an_unknown_key_could_not_run() {
    let (model, workspace) = (fixture("invalid.toml"), fixture("workspace/Cargo.toml"));

    let run = run(&fixture_arguments(&model, &workspace), &[]);

    assert_eq!(run.code, Some(2));
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("architecture check could not run: invalid ")
            && run
                .stderr
                .ends_with("invalid.toml: unknown key `typo` in [[layer]]\n"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_workspace_that_does_not_exist_could_not_run() {
    let (model, workspace) = (
        fixture("valid.toml"),
        fixture("no-such-workspace/Cargo.toml"),
    );

    let run = run(&fixture_arguments(&model, &workspace), &[]);

    assert_eq!(run.code, Some(2));
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("architecture check could not run: cannot read the workspace: "),
        "{}",
        run.stderr
    );
}

#[test]
fn a_model_that_does_not_exist_could_not_run() {
    let workspace = fixture("workspace/Cargo.toml");

    let run = run(&fixture_arguments("no-such-model.toml", &workspace), &[]);

    assert_eq!(run.code, Some(2));
    assert!(
        run.stderr
            .starts_with("architecture check could not run: cannot read no-such-model.toml: "),
        "{}",
        run.stderr
    );
}

#[test]
fn a_model_that_covers_nothing_reports_every_crate_as_uncovered() {
    let (model, workspace) = (
        fixture("nothing-covered.toml"),
        fixture("workspace/Cargo.toml"),
    );

    let run = run(&fixture_arguments(&model, &workspace), &[]);

    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert_eq!(
        run.stdout.matches("error[arch::uncovered]").count(),
        14,
        "{}",
        run.stdout
    );
    assert_eq!(
        run.stdout.matches("error[arch::lints]").count(),
        1,
        "{}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("`billing-domain` is not assigned to any layer"),
        "{}",
        run.stdout
    );
}

#[test]
fn a_report_that_cannot_be_written_is_never_a_pass() {
    let (model, workspace) = (fixture("valid.toml"), fixture("workspace/Cargo.toml"));
    let full = OpenOptions::new().write(true).open("/dev/full").unwrap();

    let status = xtask(&fixture_arguments(&model, &workspace), &[])
        .stdout(Stdio::from(full))
        .stderr(Stdio::null())
        .status()
        .unwrap();

    assert_eq!(status.code(), Some(2));
}

#[test]
fn a_missing_command_prints_the_usage_and_could_not_run() {
    let run = run(&[], &[]);

    assert_eq!(run.code, Some(2));
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr.contains("Usage: xtask <COMMAND>"),
        "{}",
        run.stderr
    );
}

#[test]
fn a_shared_context_and_a_dev_dependency_that_reaches_a_forbidden_crate_are_not_reported() {
    let (model, workspace) = (fixture("valid.toml"), fixture("workspace/Cargo.toml"));

    let run = run(&fixture_arguments(&model, &workspace), &[]);

    // `billing-domain` depends on the crate of the shared context, `billing-application` has a
    // dev-dependency on a stand-in that depends on `sqlx`.
    assert!(!run.stdout.contains("`billing-domain`"), "{}", run.stdout);
    assert!(
        !run.stdout.contains("`billing-application`"),
        "{}",
        run.stdout
    );
}

#[test]
fn every_dependency_rule_names_both_ends_of_the_edge() {
    let (model, workspace) = (fixture("valid.toml"), fixture("workspace/Cargo.toml"));

    let run = run(&fixture_arguments(&model, &workspace), &[]);

    for (rule, ends) in [
        ("context", ["`audit-domain`", "`shop-domain`"]),
        ("external", ["`ledger-domain`", "`serde`"]),
        ("transitive", ["`reports-application`", "`sqlx`"]),
    ] {
        let block = run
            .stdout
            .split("\n\n")
            .find(|block| block.starts_with(&format!("error[arch::{rule}]")))
            .unwrap();
        for end in ends {
            assert!(block.contains(end), "{rule}: {end} missing in {block}");
        }
    }
}

#[test]
fn the_vendor_stand_ins_and_the_fixture_workspace_are_not_members_of_the_real_workspace() {
    let manifest = workspace_root().join("Cargo.toml");

    let output = std::process::Command::new("cargo")
        .args(["metadata", "--locked", "--format-version", "1", "--no-deps"])
        .arg("--manifest-path")
        .arg(&manifest)
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let members: Vec<&str> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| package["name"].as_str().unwrap())
        .collect();
    // At least the 13 crates of the template: `make new-context` adds more downstream.
    assert!(members.len() >= 13, "{members:?}");
    for name in [
        "ledger-domain",
        "reports-application",
        "serde",
        "sqlx",
        "axum",
        "rstest",
    ] {
        assert!(!members.contains(&name), "{name} is a workspace member");
    }
    let manifests: Vec<&str> = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| package["manifest_path"].as_str().unwrap())
        .collect();
    assert!(
        manifests
            .iter()
            .all(|path| !path.contains("tests/fixtures")),
        "{manifests:?}"
    );
}

#[test]
fn a_local_crate_outside_the_workspace_members_is_not_assigned_to_a_layer() {
    let (model, workspace) = (fixture("outside.toml"), fixture("outside/Cargo.toml"));

    let run = run(&fixture_arguments(&model, &workspace), &[]);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stderr, "");
    let block = |name: &str| {
        format!(
            "error[arch::uncovered]: `{name}` is not assigned to any layer: it is a local crate that is not a member of the workspace\n  \
             --> crates/{name}/Cargo.toml\n   \
             = help: add the crate to `members` of the workspace and name it `<context>-<layer>`, or add its name to the `crates` of a [[layer]]\n\n"
        )
    };
    assert_eq!(
        run.stdout,
        format!(
            "{}{}architecture: FAILED. 2 violation(s) in 1 crate(s), 1 layer(s) checked\n",
            block("audit-core"),
            block("billing-core")
        )
    );
}
