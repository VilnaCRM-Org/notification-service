//! Unit tests of the parent module, against temporary directories. The lock-file step is covered by
//! the integration tests; these tests stop before it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    Failure, Fault, Refusal, create, existing, listed, package_name, relative, report, write,
};

/// A temporary directory that is removed when it is dropped.
struct Scratch(PathBuf);

static COUNTER: AtomicUsize = AtomicUsize::new(0);

impl Scratch {
    fn new() -> Self {
        let number = COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "xtask-scaffold-unit-{}-{number}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }

    /// A workspace whose manifest lists one member.
    fn workspace() -> Self {
        let scratch = Self::new();
        scratch.file("Cargo.toml", MANIFEST);
        scratch
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn file(&self, path: &str, text: &str) {
        let target = self.0.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, text).unwrap();
    }

    fn read(&self, path: &str) -> String {
        fs::read_to_string(self.0.join(path)).unwrap()
    }

    fn exists(&self, path: &str) -> bool {
        self.0.join(path).exists()
    }

    /// Every file below the directory, as sorted relative paths.
    fn files(&self) -> Vec<String> {
        let mut files = Vec::new();
        let mut pending = vec![self.0.clone()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(relative(&self.0, &path));
                }
            }
        }
        files.sort();
        files
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap_or_default();
    }
}

const MANIFEST: &str = "[workspace]\nmembers = [\n    \"crates/retail/shop/domain\",\n]\n";

const MEMBERS: [&str; 3] = [
    "crates/billing/invoice-ledger/domain",
    "crates/billing/invoice-ledger/application",
    "crates/billing/invoice-ledger/infrastructure",
];

#[test]
fn the_three_crates_are_written_and_registered() {
    let scratch = Scratch::workspace();

    let members = create(scratch.path(), "billing", "invoice-ledger").unwrap();

    assert_eq!(members, MEMBERS);
    assert_eq!(
        scratch.files(),
        [
            "Cargo.toml",
            "crates/billing/invoice-ledger/application/Cargo.toml",
            "crates/billing/invoice-ledger/application/src/lib.rs",
            "crates/billing/invoice-ledger/domain/Cargo.toml",
            "crates/billing/invoice-ledger/domain/src/lib.rs",
            "crates/billing/invoice-ledger/infrastructure/Cargo.toml",
            "crates/billing/invoice-ledger/infrastructure/src/lib.rs",
        ]
    );
    assert_eq!(
        scratch.read("Cargo.toml"),
        "[workspace]\nmembers = [\n    \"crates/billing/invoice-ledger/application\",\n    \"crates/billing/invoice-ledger/domain\",\n    \"crates/billing/invoice-ledger/infrastructure\",\n    \"crates/retail/shop/domain\",\n]\n"
    );
    let manifest = scratch.read("crates/billing/invoice-ledger/domain/Cargo.toml");
    assert!(
        manifest.starts_with("[package]\nname = \"invoice-ledger-domain\"\n"),
        "{manifest}"
    );
    let lib = scratch.read("crates/billing/invoice-ledger/application/src/lib.rs");
    assert!(lib.starts_with("//! Bounded context `invoice-ledger`, application layer"));
}

#[test]
fn an_invalid_name_is_refused_before_anything_is_read_or_written() {
    let scratch = Scratch::new();

    let failure = create(scratch.path(), "Billing", "ledger").unwrap_err();

    assert!(
        matches!(
            failure,
            Failure::Refused(Refusal::InvalidName { kind: "group", .. })
        ),
        "{failure}"
    );
    assert_eq!(scratch.files(), Vec::<String>::new());
}

#[test]
fn a_reserved_name_is_refused_and_changes_nothing() {
    let scratch = Scratch::workspace();

    let failure = create(scratch.path(), "billing", "shared").unwrap_err();

    assert!(
        matches!(failure, Failure::Refused(Refusal::Reserved(_))),
        "{failure}"
    );
    assert_eq!(scratch.files(), ["Cargo.toml"]);
    assert_eq!(scratch.read("Cargo.toml"), MANIFEST);
}

#[test]
fn an_existing_context_is_refused_in_its_own_group_and_in_another_one() {
    for group in ["retail", "billing"] {
        let scratch = Scratch::workspace();
        scratch.file("crates/retail/shop/domain/Cargo.toml", "x");
        let before = scratch.files();

        let failure = create(scratch.path(), group, "shop").unwrap_err();

        assert_eq!(
            failure.to_string(),
            "the bounded context `shop` already exists at crates/retail/shop",
            "{group}"
        );
        assert!(matches!(failure, Failure::Refused(Refusal::Exists { .. })));
        assert_eq!(scratch.files(), before, "{group}");
        assert_eq!(scratch.read("Cargo.toml"), MANIFEST);
    }
}

#[test]
fn an_unreadable_manifest_could_not_run_and_changes_nothing() {
    let scratch = Scratch::new();

    let failure = create(scratch.path(), "billing", "ledger").unwrap_err();

    assert!(
        matches!(failure, Failure::CouldNotRun(Fault::Read { .. })),
        "{failure}"
    );
    assert!(
        failure.to_string().starts_with("cannot read Cargo.toml: "),
        "{failure}"
    );
    assert_eq!(scratch.files(), Vec::<String>::new());
}

#[test]
fn a_manifest_that_does_not_parse_or_has_no_members_could_not_run_and_changes_nothing() {
    for (text, parse) in [("[workspace\n", true), ("[package]\nname = \"x\"\n", false)] {
        let scratch = Scratch::new();
        scratch.file("Cargo.toml", text);

        let failure = create(scratch.path(), "billing", "ledger").unwrap_err();

        let matched = if parse {
            matches!(failure, Failure::CouldNotRun(Fault::Parse(_)))
        } else {
            matches!(failure, Failure::CouldNotRun(Fault::NoMembers))
        };
        assert!(matched, "{failure}");
        assert_eq!(scratch.files(), ["Cargo.toml"]);
        assert_eq!(scratch.read("Cargo.toml"), text);
    }
}

#[test]
fn a_manifest_error_is_reported_before_an_existing_context() {
    let scratch = Scratch::new();
    scratch.file("Cargo.toml", "[workspace\n");
    scratch.file("crates/billing/ledger/domain/Cargo.toml", "x");

    let failure = create(scratch.path(), "billing", "ledger").unwrap_err();

    assert!(
        matches!(failure, Failure::CouldNotRun(Fault::Parse(_))),
        "{failure}"
    );
}

#[test]
fn a_path_that_cannot_be_written_could_not_run() {
    let scratch = Scratch::workspace();
    scratch.file("crates/billing", "a file where the group directory belongs");

    let failure = create(scratch.path(), "billing", "ledger").unwrap_err();

    assert!(
        matches!(failure, Failure::CouldNotRun(Fault::Write { .. })),
        "{failure}"
    );
    assert!(
        failure.to_string().starts_with("cannot write "),
        "{failure}"
    );
    assert_eq!(scratch.read("Cargo.toml"), MANIFEST);
}

#[test]
fn the_context_of_any_group_is_found() {
    let scratch = Scratch::new();
    scratch.file("crates/one/alpha/domain/Cargo.toml", "x");
    scratch.file("crates/two/beta/domain/Cargo.toml", "x");

    assert_eq!(
        existing(scratch.path(), "beta"),
        Some(scratch.path().join("crates/two/beta"))
    );
    assert_eq!(
        existing(scratch.path(), "alpha"),
        Some(scratch.path().join("crates/one/alpha"))
    );
    assert_eq!(existing(scratch.path(), "gamma"), None);
}

#[test]
fn a_file_with_the_name_of_the_context_is_not_a_context() {
    let scratch = Scratch::new();
    scratch.file("crates/one/alpha", "a file");

    assert_eq!(existing(scratch.path(), "alpha"), None);
}

#[test]
fn a_workspace_without_crates_has_no_context() {
    let scratch = Scratch::new();

    assert_eq!(existing(scratch.path(), "alpha"), None);
    scratch.file("crates", "a file instead of a directory");
    assert_eq!(existing(scratch.path(), "alpha"), None);
}

#[test]
fn a_path_is_shown_relative_to_the_root_when_it_is_below_it() {
    assert_eq!(
        relative(Path::new("/w"), Path::new("/w/crates/a/b")),
        "crates/a/b"
    );
    assert_eq!(relative(Path::new("/w"), Path::new("/other/a")), "/other/a");
}

#[test]
fn write_creates_the_directories_above_the_file() {
    let scratch = Scratch::new();

    write(&scratch.path().join("a/b/c.txt"), "text").unwrap();
    write(&scratch.path().join("d.txt"), "more").unwrap();

    assert_eq!(scratch.read("a/b/c.txt"), "text");
    assert_eq!(scratch.read("d.txt"), "more");
}

#[test]
fn write_to_a_path_below_a_file_is_an_error_that_names_the_path() {
    let scratch = Scratch::new();
    scratch.file("blocker", "x");

    let error = write(&scratch.path().join("blocker/inner.txt"), "text").unwrap_err();

    assert!(matches!(error, Fault::Write { .. }), "{error}");
    assert!(error.to_string().contains("blocker/inner.txt"), "{error}");
    assert!(!scratch.exists("blocker/inner.txt"));
}

#[test]
fn the_report_lists_the_crates_and_the_remaining_steps() {
    let members: Vec<String> = MEMBERS.iter().map(|member| (*member).to_owned()).collect();

    assert_eq!(
        report("invoice-ledger", &members),
        "Created the bounded context `invoice-ledger`:\n\
         \x20 crates/billing/invoice-ledger/domain\n\
         \x20 crates/billing/invoice-ledger/application\n\
         \x20 crates/billing/invoice-ledger/infrastructure\n\
         Registered the three crates in [workspace].members and refreshed Cargo.lock.\n\
         \n\
         Next steps:\n\
         \x20 1. Add [workspace.dependencies] path entries for the new crates when the first crate depends on them.\n\
         \x20 2. Add the code layer by layer and put the fakes of its ports in test-support.\n\
         \x20 3. Wire the context in the composition root (`service`): adapters in `Adapters`, handlers in `bootstrap::router`,\n\
         \x20    a state field in `AppState`, its router in `openapi::api_router` and its GraphQL roots in `graphql::schema`.\n\
         \x20 4. Add its three components to workspace.dsl; until then the architecture model check fails."
    );
}

#[test]
fn a_context_directly_below_crates_is_found() {
    let scratch = Scratch::new();
    scratch.file("crates/alpha/domain/Cargo.toml", "x");

    assert_eq!(
        existing(scratch.path(), "alpha"),
        Some(scratch.path().join("crates/alpha"))
    );
    assert_eq!(existing(scratch.path(), "gamma"), None);
}

#[test]
fn a_context_without_a_group_is_refused_and_changes_nothing() {
    let scratch = Scratch::new();
    let manifest = "[workspace]\nmembers = [\"crates/billing/domain\"]\n";
    scratch.file("Cargo.toml", manifest);
    scratch.file("crates/billing/domain/Cargo.toml", "x");
    let before = scratch.files();

    let failure = create(scratch.path(), "sales", "billing").unwrap_err();

    assert_eq!(
        failure.to_string(),
        "the bounded context `billing` already exists at crates/billing"
    );
    assert_eq!(scratch.files(), before);
    assert_eq!(scratch.read("Cargo.toml"), manifest);
}

#[test]
fn a_member_that_names_or_places_the_context_is_listed_wherever_it_lies() {
    let scratch = Scratch::new();
    scratch.file(
        "libs/core/Cargo.toml",
        "[package]\nname = \"billing-infrastructure\"\n",
    );
    scratch.file("libs/other/Cargo.toml", "[package]\nname = \"other\"\n");
    let members =
        |paths: &[&str]| -> Vec<String> { paths.iter().map(|path| (*path).to_owned()).collect() };

    assert_eq!(
        listed(
            scratch.path(),
            "billing",
            &members(&["libs/other", "libs/core"])
        ),
        Some(scratch.path().join("libs/core"))
    );
    assert_eq!(
        listed(
            scratch.path(),
            "billing",
            &members(&["libs/other", "x/billing/domain"])
        ),
        Some(scratch.path().join("x/billing"))
    );
    assert_eq!(
        listed(
            scratch.path(),
            "billing",
            &members(&["libs/other", "tests/billing", "xtask"])
        ),
        None
    );
    assert_eq!(listed(scratch.path(), "billing", &[]), None);
}

#[test]
fn the_package_name_needs_a_readable_manifest_with_a_name() {
    let scratch = Scratch::new();
    scratch.file("named/Cargo.toml", "[package]\nname = \"named\"\n");
    scratch.file("unnamed/Cargo.toml", "[package]\nversion = \"1\"\n");
    scratch.file("broken/Cargo.toml", "[package\n");

    assert_eq!(
        package_name(&scratch.path().join("named")),
        Some("named".to_owned())
    );
    assert_eq!(package_name(&scratch.path().join("unnamed")), None);
    assert_eq!(package_name(&scratch.path().join("broken")), None);
    assert_eq!(package_name(&scratch.path().join("missing")), None);
}
