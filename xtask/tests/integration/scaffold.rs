//! The binary in copies of the fixture workspace `tests/fixtures/scaffold/workspace/`: the crates are
//! written, the lock file is refreshed, the architecture gate passes and each new package checks alone.
//! The real workspace is never scaffolded; every copy lives in the system temporary directory and is
//! removed when its test ends.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::support::{Run, collect, forward, workspace_root, xtask_in};

/// Variables that `cargo` and `git` need to be found and to run; nothing else reaches them.
const STACK: [&str; 5] = [
    "PATH",
    "HOME",
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
];

static COPIES: AtomicUsize = AtomicUsize::new(0);

/// A copy of the fixture workspace in its own temporary git repository, committed as it is.
struct Copy {
    directory: PathBuf,
}

impl Copy {
    fn new() -> Self {
        let number = COPIES.fetch_add(1, Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("xtask-scaffold-it-{}-{number}", std::process::id()));
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scaffold/workspace");
        copy_tree(&source, &directory);
        let copy = Self { directory };
        copy.git(&["init", "--quiet", "--initial-branch=main"]);
        copy.git(&["add", "--all"]);
        copy.git(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--message",
            "fixture",
        ]);
        copy
    }

    /// Runs `xtask` with the arguments in the copy.
    fn xtask(&self, arguments: &[&str]) -> Run {
        self.xtask_with(arguments, &[])
    }

    fn xtask_with(&self, arguments: &[&str], variables: &[(&str, &str)]) -> Run {
        collect(&mut xtask_in(&self.directory, arguments, variables))
    }

    /// Runs `cargo` in the copy with a cleared environment, so no build setting of the test run
    /// (target directory, coverage flags) reaches it.
    fn cargo(&self, arguments: &[&str]) -> Run {
        let mut command = Command::new("cargo");
        command
            .args(arguments)
            .env_clear()
            .current_dir(&self.directory);
        forward(&mut command, &STACK);
        collect(&mut command)
    }

    fn git(&self, arguments: &[&str]) -> String {
        let mut command = Command::new("git");
        command
            .args(arguments)
            .env_clear()
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .current_dir(&self.directory);
        forward(&mut command, &STACK);
        let run = collect(&mut command);
        assert_eq!(run.code, Some(0), "git {arguments:?}: {}", run.stderr);
        run.stdout
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.directory.join(path)).unwrap()
    }

    fn write(&self, path: &str, text: &str) {
        let target = self.directory.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, text).unwrap();
    }

    /// Every file of the copy except the build output and git's own data, with its bytes.
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut files = BTreeMap::new();
        let mut pending = vec![self.directory.clone()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(&directory).unwrap() {
                let path = entry.unwrap().path();
                let name = path.strip_prefix(&self.directory).unwrap().to_owned();
                if name.starts_with(".git") || name.starts_with("target") {
                    continue;
                }
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.insert(name.display().to_string(), fs::read(path).unwrap());
                }
            }
        }
        files
    }
}

impl Drop for Copy {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).unwrap_or_default();
    }
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

const LAYERS: [&str; 3] = ["domain", "application", "infrastructure"];

fn created(copy: &Copy) -> Run {
    let run = copy.xtask(&["new-context", "billing", "invoice-ledger"]);
    assert_eq!(run.code, Some(0), "{}{}", run.stdout, run.stderr);
    run
}

#[test]
fn the_command_writes_the_three_crates_registers_them_and_prints_the_next_steps() {
    let copy = Copy::new();

    let run = created(&copy);

    assert_eq!(run.stderr, "");
    for expected in [
        "Created the bounded context `invoice-ledger`:\n",
        "  crates/billing/invoice-ledger/domain\n",
        "  crates/billing/invoice-ledger/application\n",
        "  crates/billing/invoice-ledger/infrastructure\n",
        "Registered the three crates in [workspace].members and refreshed Cargo.lock.\n",
        "Next steps:\n",
        "[workspace.dependencies] path entries",
        "composition root (`service`)",
        "workspace.dsl",
    ] {
        assert!(run.stdout.contains(expected), "{expected}: {}", run.stdout);
    }
    for layer in LAYERS {
        let root = format!("crates/billing/invoice-ledger/{layer}");
        let manifest = copy.read(&format!("{root}/Cargo.toml"));
        assert!(
            manifest.starts_with(&format!("[package]\nname = \"invoice-ledger-{layer}\"\n")),
            "{manifest}"
        );
        for field in [
            "version",
            "edition",
            "rust-version",
            "license",
            "publish",
            "repository",
        ] {
            assert!(
                manifest.contains(&format!("\n{field}.workspace = true\n")),
                "{manifest}"
            );
        }
        assert!(
            manifest.ends_with("\n[lints]\nworkspace = true\n"),
            "{manifest}"
        );
        assert!(!manifest.contains("[dependencies]"), "{manifest}");
        let lib = copy.read(&format!("{root}/src/lib.rs"));
        assert!(
            lib.lines()
                .all(|line| line.starts_with("//! ") || line == "//!"),
            "{lib}"
        );
        assert!(lib.contains(&format!("{layer} layer")), "{lib}");
    }
    let original = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/scaffold/workspace/Cargo.toml"),
    )
    .unwrap();
    assert_eq!(
        copy.read("Cargo.toml"),
        original.replace(
            "    # The one",
            "    \"crates/billing/invoice-ledger/application\",\n    \"crates/billing/invoice-ledger/domain\",\n    \"crates/billing/invoice-ledger/infrastructure\",\n    # The one"
        )
    );
}

#[test]
fn the_lock_file_is_refreshed_by_the_command_so_that_locked_metadata_succeeds() {
    let copy = Copy::new();
    created(&copy);

    let metadata = copy.cargo(&["metadata", "--locked", "--offline", "--format-version", "1"]);

    assert_eq!(metadata.code, Some(0), "{}", metadata.stderr);
    let lock = copy.read("Cargo.lock");
    for layer in LAYERS {
        assert!(
            lock.contains(&format!("name = \"invoice-ledger-{layer}\"\n")),
            "{lock}"
        );
    }
    assert!(lock.contains("name = \"shop-domain\"\n"), "{lock}");
}

#[test]
fn the_architecture_gate_passes_and_each_new_package_checks_alone() {
    let copy = Copy::new();
    created(&copy);

    let arch = copy.xtask(&["arch"]);

    assert_eq!(arch.code, Some(0), "{}{}", arch.stdout, arch.stderr);
    assert_eq!(
        arch.stdout,
        "architecture: ok. All 4 crate(s) are assigned to exactly one layer (3 layer(s)); 0 violation(s)\n"
    );
    for layer in LAYERS {
        let package = format!("invoice-ledger-{layer}");
        let check = copy.cargo(&["check", "--locked", "--offline", "--package", &package]);

        assert_eq!(check.code, Some(0), "{package}: {}", check.stderr);
    }
}

/// The paths that `git status` reports, one per file.
fn changed(copy: &Copy) -> Vec<String> {
    copy.git(&["status", "--porcelain", "--untracked-files=all"])
        .lines()
        .map(|line| line.get(3..).unwrap().to_owned())
        .collect()
}

#[test]
fn no_protected_path_is_touched() {
    let copy = Copy::new();
    created(&copy);

    let list = copy.xtask(&["config-guard", "--list"]);
    let changed = changed(&copy);

    assert_eq!(list.code, Some(0), "{}", list.stderr);
    let items: Vec<&str> = list.stdout.lines().collect();
    assert!(items.contains(&"architecture.toml"), "{items:?}");
    assert!(changed.contains(&"Cargo.toml".to_owned()), "{changed:?}");
    assert!(changed.contains(&"Cargo.lock".to_owned()), "{changed:?}");
    assert_eq!(changed.len(), 2 + 6, "{changed:?}");
    for path in &changed {
        for item in &items {
            assert!(
                path != item && !(item.ends_with('/') && path.starts_with(item)),
                "{path} is protected by {item}"
            );
        }
    }
    let guard = copy.xtask(&["config-guard", "--base", "HEAD"]);
    assert_eq!(guard.code, Some(0), "{}{}", guard.stdout, guard.stderr);
    assert_eq!(
        guard.stdout,
        "Configuration Validation\n[OK] Configuration validation passed\n"
    );
}

#[test]
fn a_refused_request_exits_with_status_one_names_the_reason_and_changes_no_file() {
    for (arguments, reason) in [
        (["Billing", "ledger"], "the group name `Billing` is invalid"),
        (["billing", "1x"], "the context name `1x` is invalid"),
        (["billing", "a--b"], "the context name `a--b` is invalid"),
        (
            ["billing", "Ledger"],
            "the context name `Ledger` is invalid",
        ),
        (["billing", "shared"], "`shared` is reserved"),
        (["billing", "service"], "`service` is reserved"),
        (["billing", "xtask"], "`xtask` is reserved"),
        (["billing", "test-support"], "`test-support` is reserved"),
        (["billing", "e2e-tests"], "`e2e-tests` is reserved"),
        (
            ["retail", "shop"],
            "the bounded context `shop` already exists at crates/retail/shop",
        ),
        (
            ["billing", "shop"],
            "the bounded context `shop` already exists at crates/retail/shop",
        ),
    ] {
        let copy = Copy::new();
        let before = copy.snapshot();

        let run = copy.xtask(&["new-context", arguments[0], arguments[1]]);

        assert_eq!(run.code, Some(1), "{arguments:?}: {}", run.stderr);
        assert_eq!(run.stdout, "", "{arguments:?}");
        assert!(
            run.stderr.starts_with("new-context refused: ") && run.stderr.contains(reason),
            "{arguments:?}: {}",
            run.stderr
        );
        assert_eq!(copy.snapshot(), before, "{arguments:?}");
        assert_eq!(copy.git(&["status", "--porcelain"]), "", "{arguments:?}");
    }
}

#[test]
fn a_second_run_for_the_same_context_is_refused() {
    let copy = Copy::new();
    created(&copy);
    let after_first = copy.snapshot();

    let again = copy.xtask(&["new-context", "billing", "invoice-ledger"]);
    let elsewhere = copy.xtask(&["new-context", "accounting", "invoice-ledger"]);

    for run in [&again, &elsewhere] {
        assert_eq!(run.code, Some(1), "{}", run.stderr);
        assert!(
            run.stderr.contains("the bounded context `invoice-ledger` already exists at crates/billing/invoice-ledger"),
            "{}",
            run.stderr
        );
    }
    assert_eq!(copy.snapshot(), after_first);
}

#[test]
fn a_root_manifest_that_cannot_be_parsed_exits_with_status_two_and_changes_no_file() {
    for manifest in [
        "[workspace\n",
        "[package]\nname = \"x\"\n",
        "[workspace]\nmembers = \"crates\"\n",
    ] {
        let copy = Copy::new();
        copy.write("Cargo.toml", manifest);
        let before = copy.snapshot();

        let run = copy.xtask(&["new-context", "billing", "invoice-ledger"]);

        assert_eq!(run.code, Some(2), "{manifest}: {}", run.stdout);
        assert_eq!(run.stdout, "", "{manifest}");
        assert!(
            run.stderr.starts_with("new-context could not run: "),
            "{manifest}: {}",
            run.stderr
        );
        assert_eq!(copy.snapshot(), before, "{manifest}");
    }
}

#[test]
fn a_directory_without_a_root_manifest_exits_with_status_two() {
    let copy = Copy::new();
    fs::remove_file(copy.directory.join("Cargo.toml")).unwrap();

    let run = copy.xtask(&["new-context", "billing", "invoice-ledger"]);

    assert_eq!(run.code, Some(2));
    assert!(
        run.stderr
            .starts_with("new-context could not run: cannot read Cargo.toml: "),
        "{}",
        run.stderr
    );
    assert!(!copy.directory.join("crates/billing").exists());
}

#[test]
fn a_failing_lock_step_exits_with_status_two() {
    let copy = Copy::new();
    copy.write("Cargo.lock", "this is not a lock file\n");

    let run = copy.xtask(&["new-context", "billing", "invoice-ledger"]);

    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert_eq!(run.stdout, "");
    assert!(
        run.stderr.starts_with(
            "new-context could not run: the crates were created, but `cargo update --workspace --offline` failed, so Cargo.lock is stale: "
        ),
        "{}",
        run.stderr
    );
    assert!(run.stderr.contains("Cargo.lock"), "{}", run.stderr);
}

#[test]
fn a_missing_cargo_exits_with_status_two() {
    let copy = Copy::new();

    let run = copy.xtask_with(
        &["new-context", "billing", "invoice-ledger"],
        &[("PATH", "/nonexistent")],
    );

    assert_eq!(run.code, Some(2), "{}", run.stdout);
    assert!(
        run.stderr.starts_with(
            "new-context could not run: cannot run `cargo update --workspace --offline`: "
        ),
        "{}",
        run.stderr
    );
}

#[test]
fn the_make_target_without_both_names_exits_with_status_two_and_prints_the_usage() {
    for arguments in [&[][..], &["group=billing"], &["context=ledger"]] {
        let mut command = Command::new("make");
        command
            .arg("--no-print-directory")
            .arg("-C")
            .arg(workspace_root())
            .arg("new-context")
            .args(arguments)
            .env_clear();
        forward(&mut command, &STACK);

        let run = collect(&mut command);

        assert_eq!(run.code, Some(2), "{arguments:?}: {}", run.stderr);
        assert!(
            run.stderr
                .contains("Usage: make new-context group=<group> context=<context>\n"),
            "{arguments:?}: {}",
            run.stderr
        );
        assert_eq!(run.stdout, "", "{arguments:?}");
    }
}

#[test]
fn a_context_without_a_group_directory_is_refused_before_anything_is_written() {
    let copy = Copy::new();
    let manifest = copy.read("Cargo.toml").replace(
        "    \"crates/retail/shop/domain\",\n",
        "    \"crates/ledger/domain\",\n    \"crates/retail/shop/domain\",\n",
    );
    copy.write("Cargo.toml", &manifest);
    copy.write(
        "crates/ledger/domain/Cargo.toml",
        "[package]\nname = \"ledger-domain\"\nversion = \"0.0.0\"\n",
    );
    let before = copy.snapshot();

    let run = copy.xtask(&["new-context", "billing", "ledger"]);

    assert_eq!(run.code, Some(1), "{}{}", run.stdout, run.stderr);
    assert_eq!(run.stdout, "");
    assert_eq!(
        run.stderr,
        "new-context refused: the bounded context `ledger` already exists at crates/ledger\n"
    );
    assert_eq!(copy.snapshot(), before);
}

#[test]
fn a_context_that_a_member_names_or_places_is_refused_wherever_the_crates_are() {
    for (member, package, reason) in [
        (
            "libs/core",
            "ledger-application",
            "already exists at libs/core",
        ),
        (
            "libs/ledger/domain",
            "other",
            "already exists at libs/ledger",
        ),
    ] {
        let copy = Copy::new();
        let manifest = copy.read("Cargo.toml").replace(
            "    \"crates/retail/shop/domain\",\n",
            &format!("    \"crates/retail/shop/domain\",\n    \"{member}\",\n"),
        );
        copy.write("Cargo.toml", &manifest);
        copy.write(
            &format!("{member}/Cargo.toml"),
            &format!("[package]\nname = \"{package}\"\nversion = \"0.0.0\"\n"),
        );
        let before = copy.snapshot();

        let run = copy.xtask(&["new-context", "billing", "ledger"]);

        assert_eq!(run.code, Some(1), "{member}: {}{}", run.stdout, run.stderr);
        assert!(run.stderr.contains(reason), "{member}: {}", run.stderr);
        assert_eq!(copy.snapshot(), before, "{member}");
    }
}
