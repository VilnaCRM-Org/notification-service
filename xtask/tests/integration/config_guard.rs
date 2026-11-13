//! The binary against temporary git repositories: the base ref is a commit, the working tree is
//! what the test changes afterwards.

use std::fs;

use super::support::{Repo, run, workspace_root};

const LIST: &str = r#"approval_label = "approved-config-change"
files = ["clippy.toml", "deny.toml"]
directories = ["guarded/"]
manifest_tables = [{ file = "Cargo.toml", table = "workspace.lints" }]
json_values = [{ file = "tests/Load/config.json.dist", pointers = ["/endpoints/health/smoke/threshold", "/endpoints/health/stress/threshold"] }]
override_names = ["clippy.toml", ".cargo/config.toml"]
override_exempt = ["fixtures/"]
required_directories = ["crates", "tests/Load"]
"#;

const CARGO: &str = "[workspace]\nmembers = []\n\n[workspace.lints.clippy]\nall = \"warn\"\npedantic = \"warn\"\n\n[dependencies]\nserde = \"1\"\n";

const BUDGETS: &str = r#"{"endpoints":{"health":{"smoke":{"threshold":50},"stress":{"threshold":80}},"other":{"smoke":{"threshold":1}}}}"#;

const OK: &str = "Configuration Validation\n[OK] Configuration validation passed\n";

const FILE: &str = "Modification of locked configuration file is not allowed:";
const TABLE: &str = "Modification of locked configuration table is not allowed:";
const VALUE: &str = "Modification of locked configuration value is not allowed:";
const OVERRIDE: &str = "Overriding configuration file is not allowed:";

const FAILED: &str =
    "[FAIL] Configuration validation failed\nApprove with the label approved-config-change\n";

/// A repository with one commit that holds the list and every protected item.
fn repository() -> Repo {
    let repo = Repo::new();
    repo.write("protected-config.toml", LIST);
    repo.write("clippy.toml", "too-many-lines-threshold = 50\n");
    repo.write("deny.toml", "[bans]\n");
    repo.write("Cargo.toml", CARGO);
    repo.write("tests/Load/config.json.dist", BUDGETS);
    repo.write("crates/keep.txt", "x");
    repo.write("guarded/one.txt", "1");
    repo.write("README.md", "# readme\n");
    repo.commit("base");
    repo
}

/// The working tree of the scenario of the first acceptance criterion.
fn change_everything(repo: &Repo) {
    repo.write("clippy.toml", "too-many-lines-threshold = 500\n");
    repo.remove("deny.toml");
    repo.write(
        "Cargo.toml",
        &CARGO.replace("pedantic = \"warn\"", "pedantic = \"allow\""),
    );
    repo.write(
        "tests/Load/config.json.dist",
        &BUDGETS.replace("\"threshold\":50", "\"threshold\":5000"),
    );
    repo.write("crates/x/clippy.toml", "a = 1\n");
}

#[test]
fn an_unchanged_working_tree_passes() {
    let repo = repository();

    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, OK);
    assert_eq!(run.stderr, "");
}

#[test]
fn every_kind_of_modification_is_reported_and_the_label_is_named() {
    let repo = repository();
    change_everything(&repo);

    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert_eq!(run.stderr, "");
    assert_eq!(
        run.stdout,
        format!(
            "Configuration Validation\n\
             {FILE} clippy.toml\n\
             {FILE} deny.toml\n\
             {OVERRIDE} crates/x/clippy.toml\n\
             {TABLE} Cargo.toml [workspace.lints]\n\
             {VALUE} tests/Load/config.json.dist /endpoints/health/smoke/threshold\n\
             {FAILED}"
        )
    );
}

#[test]
fn the_approval_variable_lists_every_modification_as_approved() {
    let repo = repository();
    change_everything(&repo);

    let run = repo.run(
        &["config-guard", "--base", "HEAD"],
        &[("CONFIG_CHANGE_APPROVED", "true")],
    );

    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stderr, "");
    assert_eq!(
        run.stdout,
        format!(
            "Configuration Validation\n\
             [APPROVED] {FILE} clippy.toml\n\
             [APPROVED] {FILE} deny.toml\n\
             [APPROVED] {OVERRIDE} crates/x/clippy.toml\n\
             [APPROVED] {TABLE} Cargo.toml [workspace.lints]\n\
             [APPROVED] {VALUE} tests/Load/config.json.dist /endpoints/health/smoke/threshold\n\
             [OK] Configuration validation passed (approved by label approved-config-change)\n"
        )
    );
}

#[test]
fn an_approval_that_is_not_true_approves_nothing() {
    let repo = repository();
    repo.write("clippy.toml", "changed = 1\n");

    let off = repo.run(
        &["config-guard", "--base", "HEAD"],
        &[("CONFIG_CHANGE_APPROVED", "false")],
    );
    assert_eq!(off.code, Some(1), "{}{}", off.stdout, off.stderr);
    assert!(off.stdout.ends_with(FAILED), "{}", off.stdout);
    let empty = repo.run(
        &["config-guard", "--base", "HEAD"],
        &[("CONFIG_CHANGE_APPROVED", "")],
    );
    assert_eq!(empty.code, Some(1), "{}{}", empty.stdout, empty.stderr);
    assert!(empty.stdout.ends_with(FAILED), "{}", empty.stdout);
    for value in ["0", "maybe"] {
        let run = repo.run(
            &["config-guard", "--base", "HEAD"],
            &[("CONFIG_CHANGE_APPROVED", value)],
        );

        assert_eq!(run.code, Some(2), "{value:?}: {}{}", run.stdout, run.stderr);
        assert_eq!(run.stdout, "");
    }
    let flag = repo.run(
        &["config-guard", "--base", "HEAD", "--config-change-approved"],
        &[],
    );
    assert_eq!(flag.code, Some(0), "{}{}", flag.stdout, flag.stderr);
}

#[test]
fn a_missing_required_directory_fails_with_and_without_the_approval() {
    let repo = repository();
    repo.remove("crates");
    repo.write("clippy.toml", "changed = 1\n");

    let plain = repo.run(&["config-guard", "--base", "HEAD"], &[]);
    let approved = repo.run(
        &["config-guard", "--base", "HEAD"],
        &[("CONFIG_CHANGE_APPROVED", "true")],
    );

    assert_eq!(plain.code, Some(1), "{}", plain.stderr);
    assert_eq!(
        plain.stdout,
        format!(
            "Configuration Validation\n{FILE} clippy.toml\n\
             Required directory is missing: crates\n{FAILED}"
        )
    );
    assert_eq!(approved.code, Some(1), "{}", approved.stderr);
    assert_eq!(
        approved.stdout,
        format!(
            "Configuration Validation\n[APPROVED] {FILE} clippy.toml\n\
             Required directory is missing: crates\n{FAILED}"
        )
    );
}

#[test]
fn a_deleted_file_a_deleted_table_a_missing_pointer_and_a_deleted_json_file_are_modifications() {
    let repo = repository();
    repo.remove("deny.toml");
    repo.write("Cargo.toml", "[dependencies]\nserde = \"1\"\n");
    repo.write(
        "tests/Load/config.json.dist",
        r#"{"endpoints":{"health":{"smoke":{"threshold":50}}}}"#,
    );

    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert_eq!(
        run.stdout,
        format!(
            "Configuration Validation\n\
             {FILE} deny.toml\n\
             {TABLE} Cargo.toml [workspace.lints]\n\
             {VALUE} tests/Load/config.json.dist /endpoints/health/stress/threshold\n\
             {FAILED}"
        )
    );

    repo.remove("tests/Load/config.json.dist");
    repo.write("tests/Load/readme.txt", "x");
    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);
    assert_eq!(run.stdout.matches(VALUE).count(), 2, "{}", run.stdout);
}

#[test]
fn a_file_below_a_protected_directory_is_protected_and_a_new_one_is_not() {
    let repo = repository();
    repo.write("guarded/two.txt", "new");
    repo.write("guarded/deep/three.txt", "new");

    let new = repo.run(&["config-guard", "--base", "HEAD"], &[]);
    repo.write("guarded/one.txt", "changed");
    let changed = repo.run(&["config-guard", "--base", "HEAD"], &[]);
    repo.remove("guarded");
    let deleted = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(new.stdout, OK);
    assert_eq!(
        changed.stdout,
        format!("Configuration Validation\n{FILE} guarded/one.txt\n{FAILED}")
    );
    assert_eq!(
        deleted.stdout,
        format!("Configuration Validation\n{FILE} guarded/one.txt\n{FAILED}")
    );
}

#[test]
fn changes_that_the_list_does_not_protect_pass() {
    let repo = repository();
    repo.write("Cargo.toml", &format!("{CARGO}anyhow = \"1\"\n"));
    repo.write(
        "tests/Load/config.json.dist",
        &BUDGETS.replace("\"threshold\":1}", "\"threshold\":99}"),
    );
    repo.write("src/new.rs", "fn main() {}\n");
    repo.write("fixtures/case/clippy.toml", "a = 1\n");

    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, OK);
}

#[test]
fn an_override_is_found_whether_it_is_untracked_or_staged() {
    let repo = repository();
    repo.write("crates/a/.cargo/config.toml", "a = 1\n");
    repo.write("crates/b/clippy.toml", "a = 1\n");
    repo.stage("crates/b/clippy.toml");

    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.code, Some(1), "{}", run.stderr);
    assert_eq!(
        run.stdout,
        format!(
            "Configuration Validation\n\
             {OVERRIDE} crates/a/.cargo/config.toml\n\
             {OVERRIDE} crates/b/clippy.toml\n\
             {FAILED}"
        )
    );
}

#[test]
fn an_ignored_file_is_not_an_override() {
    let repo = repository();
    repo.write(".gitignore", "ignored/\n");
    repo.commit("ignore");
    repo.write("ignored/clippy.toml", "a = 1\n");

    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.stdout, OK);
}

#[test]
fn the_base_ref_decides_not_the_last_commit() {
    let repo = repository();
    repo.write("clippy.toml", "changed = 1\n");
    repo.write("crates/x/clippy.toml", "a = 1\n");
    repo.commit("weaken");

    let head = repo.run(&["config-guard", "--base", "HEAD"], &[]);
    let parent = repo.run(&["config-guard", "--base", "HEAD~1"], &[]);

    assert_eq!(head.stdout, OK);
    assert_eq!(
        parent.stdout,
        format!(
            "Configuration Validation\n{FILE} clippy.toml\n\
             {OVERRIDE} crates/x/clippy.toml\n{FAILED}"
        )
    );
}

#[test]
fn a_list_that_loses_an_entry_still_reports_what_the_base_ref_protected() {
    let repo = repository();
    repo.write(
        "protected-config.toml",
        &LIST.replace("files = [\"clippy.toml\", \"deny.toml\"]", "files = []"),
    );
    repo.write("clippy.toml", "changed = 1\n");

    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.code, Some(1), "{}", run.stdout);
    assert!(
        run.stdout.contains(&format!("{FILE} clippy.toml\n")),
        "{}",
        run.stdout
    );
}

#[test]
fn a_base_ref_without_any_protected_item_is_the_bootstrap_case() {
    let repo = Repo::new();
    repo.write("README.md", "# readme\n");
    repo.commit("first");
    repo.write("protected-config.toml", LIST);
    repo.write("clippy.toml", "a = 1\n");
    repo.write("Cargo.toml", CARGO);
    repo.write("crates/x/clippy.toml", "a = 1\n");
    repo.write("tests/Load/config.json.dist", BUDGETS);

    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, OK);
}

#[test]
fn a_base_ref_that_does_not_resolve_exits_with_status_two() {
    let repo = repository();

    for arguments in [
        vec!["config-guard", "--base", "no-such-ref"],
        vec!["config-guard", "--base", "--no-such-option"],
        vec!["config-guard"],
    ] {
        let run = repo.run(&arguments, &[]);

        assert_eq!(run.code, Some(2), "{arguments:?}: {}", run.stdout);
        assert_eq!(run.stdout, "", "{arguments:?}");
    }
    let run = repo.run(&["config-guard", "--base", "no-such-ref"], &[]);
    assert_eq!(
        run.stderr,
        "configuration guard could not run: base ref no-such-ref is not available\n"
    );
    let default = repo.run(&["config-guard"], &[]);
    assert_eq!(
        default.stderr,
        "configuration guard could not run: base ref origin/main is not available\n"
    );
}

#[test]
fn a_directory_that_is_not_a_repository_has_no_base_ref() {
    let run = run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert_eq!(
        run.stderr,
        "configuration guard could not run: base ref HEAD is not available\n"
    );
}

#[test]
fn a_missing_git_executable_is_a_failure_to_run_and_never_a_pass() {
    let repo = repository();

    let run = repo.run(
        &["config-guard", "--base", "HEAD"],
        &[("PATH", "/nonexistent")],
    );

    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("configuration guard could not run: cannot run git: "),
        "{}",
        run.stderr
    );
}

#[test]
fn a_list_that_cannot_be_read_at_the_base_ref_or_in_the_working_tree_is_a_failure_to_run() {
    let repo = Repo::new();
    repo.write("README.md", "# readme\n");
    repo.commit("first");

    let run = repo.run(&["config-guard", "--base", "HEAD"], &[]);

    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert_eq!(
        run.stderr,
        "configuration guard could not run: protected-config.toml is neither in the base ref nor in the working tree\n"
    );
}

#[test]
fn listing_prints_one_protected_item_per_line() {
    let repo = repository();

    let run = repo.run(&["config-guard", "--list"], &[]);

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(run.stderr, "");
    assert_eq!(
        run.stdout,
        "clippy.toml\ndeny.toml\nguarded/\nCargo.toml#workspace.lints\n\
         tests/Load/config.json.dist#/endpoints/health/smoke/threshold\n\
         tests/Load/config.json.dist#/endpoints/health/stress/threshold\n"
    );
}

#[test]
fn listing_the_real_repository_prints_every_entry_of_its_list() {
    let root = workspace_root();
    let text = fs::read_to_string(root.join("protected-config.toml")).unwrap();
    let list: toml::Table = text.parse().unwrap();
    let count = |key: &str| list[key].as_array().unwrap().len();
    let pointers: usize = list["json_values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["pointers"].as_array().unwrap().len())
        .sum();

    let run = super::support::xtask_in(&root, &["config-guard", "--list"], &[])
        .output()
        .unwrap();

    assert_eq!(run.status.code(), Some(0));
    assert_eq!(run.stderr, b"");
    let stdout = String::from_utf8(run.stdout).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(
        lines.len(),
        count("files") + count("directories") + count("manifest_tables") + pointers
    );
    for file in list["files"].as_array().unwrap() {
        assert!(lines.contains(&file.as_str().unwrap()), "{file}");
    }
    for directory in list["directories"].as_array().unwrap() {
        let directory = directory.as_str().unwrap();
        assert!(lines.contains(&directory), "{directory}");
        assert!(directory.ends_with('/'), "{directory}");
    }
    assert!(lines.contains(&"Cargo.toml#workspace.lints"));
    assert!(lines.contains(&"tests/Load/config.json.dist#/endpoints/health/spike/threshold"));
    assert!(lines.iter().all(|line| !line.is_empty()));
}

#[test]
fn listing_without_a_list_is_a_failure_to_run() {
    let run = run(&["config-guard", "--list"], &[]);

    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr
            .starts_with("configuration guard could not run: cannot read protected-config.toml: "),
        "{}",
        run.stderr
    );
}

/// A repository with the list and the required directories, and nothing else committed yet.
fn bare_repository() -> Repo {
    let repo = Repo::new();
    repo.write("protected-config.toml", LIST);
    repo.write("crates/keep.txt", "x");
    repo.write("tests/Load/keep.txt", "x");
    repo
}

/// What the guard prints when only the given lines are reported.
fn failure_with(lines: &[String]) -> String {
    format!("Configuration Validation\n{}\n{FAILED}", lines.join("\n"))
}

fn guard(repo: &Repo) -> super::support::Run {
    repo.run(&["config-guard", "--base", "HEAD"], &[])
}

#[test]
fn a_protected_directory_replaced_by_a_link_to_a_copy_is_a_modification() {
    let repo = repository();
    repo.rename("guarded", "moved");
    repo.link("guarded", "moved");

    let run = guard(&repo);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(
        run.stdout,
        failure_with(&[format!("{FILE} guarded"), format!("{FILE} guarded/one.txt")])
    );
}

#[test]
fn a_link_below_a_protected_directory_is_a_modification() {
    let repo = bare_repository();
    repo.write("guarded/inner/two.txt", "2");
    repo.commit("base");
    repo.rename("guarded/inner", "elsewhere");
    repo.link("guarded/inner", "../elsewhere");

    let run = guard(&repo);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(
        run.stdout,
        failure_with(&[format!("{FILE} guarded/inner/two.txt")])
    );
}

#[test]
fn a_protected_file_replaced_by_a_link_with_the_same_bytes_is_a_modification() {
    let repo = repository();
    repo.write("copy.toml", "too-many-lines-threshold = 50\n");
    repo.link("clippy.toml", "copy.toml");

    let run = guard(&repo);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, failure_with(&[format!("{FILE} clippy.toml")]));
}

#[test]
fn a_protected_file_replaced_by_a_directory_is_a_modification() {
    let repo = repository();
    repo.remove("deny.toml");
    repo.write("deny.toml/inner.txt", "x");

    let run = guard(&repo);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, failure_with(&[format!("{FILE} deny.toml")]));
}

#[test]
fn a_link_that_the_base_ref_already_holds_is_unchanged_until_it_is_retargeted() {
    let repo = bare_repository();
    repo.write("clippy.toml", "a = 1\n");
    repo.write("other.toml", "b = 2\n");
    repo.link("deny.toml", "clippy.toml");
    repo.link("guarded", "elsewhere");
    repo.commit("base");

    let run = guard(&repo);
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, OK);

    repo.link("deny.toml", "other.toml");
    let run = guard(&repo);
    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, failure_with(&[format!("{FILE} deny.toml")]));
}

#[test]
fn a_directory_that_the_base_ref_holds_as_a_link_is_protected_as_that_link() {
    let repo = bare_repository();
    repo.write("elsewhere/one.txt", "1");
    repo.link("guarded", "elsewhere");
    repo.commit("base");
    repo.link("guarded", "somewhere-else");

    let run = guard(&repo);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, failure_with(&[format!("{FILE} guarded")]));
}

#[test]
fn losing_or_gaining_the_executable_bit_of_a_protected_file_is_a_modification() {
    let repo = bare_repository();
    repo.write("clippy.toml", "a = 1\n");
    repo.write("deny.toml", "b = 2\n");
    repo.write("guarded/hook.sh", "#!/bin/sh\n");
    repo.executable("deny.toml", true);
    repo.executable("guarded/hook.sh", true);
    repo.commit("base");
    assert_eq!(guard(&repo).code, Some(0));

    repo.executable("deny.toml", false);
    repo.executable("guarded/hook.sh", false);
    repo.executable("clippy.toml", true);
    let run = guard(&repo);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(
        run.stdout,
        failure_with(&[
            format!("{FILE} clippy.toml"),
            format!("{FILE} deny.toml"),
            format!("{FILE} guarded/hook.sh"),
        ])
    );
    let approved = repo.run(
        &["config-guard", "--base", "HEAD"],
        &[("CONFIG_CHANGE_APPROVED", "true")],
    );
    assert_eq!(approved.code, Some(0), "{}", approved.stdout);
}

/// A repository that holds the list of the real repository and every directory the list requires.
fn real_list_repository() -> Repo {
    let real = fs::read_to_string(workspace_root().join("protected-config.toml")).unwrap();
    let list: toml::Table = real.parse().unwrap();
    let repo = Repo::new();
    repo.write("protected-config.toml", &real);
    for directory in list["required_directories"].as_array().unwrap() {
        repo.write(&format!("{}/keep.txt", directory.as_str().unwrap()), "x");
    }
    repo
}

#[test]
fn the_real_list_reports_the_configuration_names_of_the_pinned_lint_tools() {
    let repo = real_list_repository();
    repo.write("clippy.toml", "a = 1\n");
    repo.commit("base");
    let names = [
        "tests/Load/scripts/eslint.config.mjs",
        "tests/Load/scripts/eslint.config.cts",
        "tests/Load/.prettierrc.json5",
        "tests/Load/.prettierrc.toml",
        "tests/Load/prettier.config.mjs",
        "tests/Load/prettier.config.ts",
        "docs/.markdownlint.cjs",
        "docs/.markdownlint-cli2.mjs",
    ];
    for name in names {
        repo.write(name, "x");
    }

    let run = guard(&repo);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    for name in names {
        assert!(
            run.stdout.contains(&format!("{OVERRIDE} {name}\n")),
            "{name}: {}",
            run.stdout
        );
    }
}

#[test]
fn the_real_list_protects_the_entry_point_of_the_guard_and_the_load_thresholds() {
    let repo = real_list_repository();
    let items = [
        "tests/Load/utils/thresholdsBuilder.js",
        "xtask/src/cli.rs",
        "xtask/src/main.rs",
    ];
    for item in items {
        repo.write(item, "base\n");
    }
    repo.commit("base");
    assert_eq!(guard(&repo).code, Some(0));
    for item in items {
        repo.write(item, "changed\n");
    }

    let run = guard(&repo);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    let expected: Vec<String> = items.iter().map(|item| format!("{FILE} {item}")).collect();
    assert_eq!(run.stdout, failure_with(&expected));
}
