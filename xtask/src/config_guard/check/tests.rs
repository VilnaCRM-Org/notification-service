//! Unit tests of the parent module, against the in-memory tree.

use super::super::Failure;
use super::super::base::Mode;
use super::super::list::{self, List};
use super::super::memory::Memory;
use super::{Findings, check};

const CARGO: &str = "[workspace]\nmembers = []\n\n[workspace.lints.clippy]\nall = \"warn\"\npedantic = \"warn\"\n\n[dependencies]\nserde = \"1\"\n";
const BUDGETS: &str =
    r#"{"endpoints":{"health":{"smoke":{"threshold":50},"stress":{"threshold":80}}},"extra":1}"#;

fn list(text: &str) -> List {
    list::parse(format!("approval_label = \"label\"\n{text}").as_bytes()).unwrap()
}

fn lines(tree: &Memory, text: &str) -> Vec<String> {
    let findings = check(&list(text), tree).unwrap();
    assert_eq!(findings.missing, Vec::<String>::new(), "{text}");
    findings.modifications
}

fn file_line(path: &str) -> String {
    format!("Modification of locked configuration file is not allowed: {path}")
}

fn override_line(path: &str) -> String {
    format!("Overriding configuration file is not allowed: {path}")
}

fn table_line(file: &str, table: &str) -> String {
    format!("Modification of locked configuration table is not allowed: {file} [{table}]")
}

fn value_line(file: &str, pointer: &str) -> String {
    format!("Modification of locked configuration value is not allowed: {file} {pointer}")
}

const CARGO_TABLE: &str =
    "manifest_tables = [{ file = \"Cargo.toml\", table = \"workspace.lints\" }]";
const BUDGET_VALUES: &str = "json_values = [{ file = \"budgets.json\", pointers = [\"/endpoints/health/smoke/threshold\", \"/endpoints/health/stress/threshold\"] }]";

#[test]
fn an_unchanged_tree_has_no_findings() {
    let tree = Memory::default()
        .same("clippy.toml", "a = 1")
        .same("xtask/tests/one.rs", "x")
        .same("Cargo.toml", CARGO)
        .same("budgets.json", BUDGETS)
        .directory("crates");
    let text = format!(
        "files = [\"clippy.toml\"]\ndirectories = [\"xtask/tests/\"]\n{CARGO_TABLE}\n{BUDGET_VALUES}\nrequired_directories = [\"crates\"]\noverride_names = [\"clippy.toml\"]"
    );

    assert_eq!(
        check(&list(&text), &tree).unwrap(),
        Findings {
            modifications: Vec::new(),
            missing: Vec::new()
        }
    );
}

#[test]
fn a_listed_file_that_changed_or_is_gone_is_a_modification_and_a_new_file_is_allowed() {
    let tree = Memory::default()
        .same("same.toml", "a")
        .on_base("changed.toml", "a")
        .in_work("changed.toml", "b")
        .on_base("deleted.toml", "a")
        .in_work("new.toml", "a");
    let text = "files = [\"same.toml\", \"changed.toml\", \"deleted.toml\", \"new.toml\", \"absent.toml\"]";

    assert_eq!(
        lines(&tree, text),
        [file_line("changed.toml"), file_line("deleted.toml")]
    );
}

#[test]
fn a_file_of_a_listed_directory_is_protected_as_the_base_ref_has_it() {
    let tree = Memory::default()
        .same("xtask/tests/same.rs", "a")
        .on_base("xtask/tests/changed.rs", "a")
        .in_work("xtask/tests/changed.rs", "b")
        .on_base("xtask/tests/deep/deleted.rs", "a")
        .same("xtask/tests-extra/other.rs", "a")
        .on_base("xtask/tests-extra/changed.rs", "a")
        .in_work("xtask/tests/new.rs", "a")
        .in_work("xtask/tests-extra/changed.rs", "b");

    for directory in ["xtask/tests/", "xtask/tests"] {
        let text = format!("directories = [\"{directory}\"]");

        assert_eq!(
            lines(&tree, &text),
            [
                file_line("xtask/tests/changed.rs"),
                file_line("xtask/tests/deep/deleted.rs")
            ],
            "{directory}"
        );
    }
}

#[test]
fn a_file_that_is_listed_and_below_a_listed_directory_is_reported_once() {
    let tree = Memory::default().on_base("xtask/tests/one.rs", "a");
    let text = "files = [\"xtask/tests/one.rs\"]\ndirectories = [\"xtask/tests/\"]";

    assert_eq!(lines(&tree, text), [file_line("xtask/tests/one.rs")]);
}

#[test]
fn a_nested_file_named_like_a_protected_one_is_an_override_once_a_protected_item_exists() {
    let tree = Memory::default()
        .same("clippy.toml", "a")
        .same("crates/old/clippy.toml", "a")
        .in_work("crates/x/clippy.toml", "a")
        .in_work("crates/x/.cargo/config.toml", "a")
        .in_work("deep/er/clippy.toml", "a")
        .in_work("rust-toolchain.toml", "a")
        .in_work("crates/x/myclippy.toml", "a")
        .in_work("crates/x/not.cargo/config.toml", "a")
        .in_work("crates/x/readme.md", "a");
    let text = "files = [\"clippy.toml\"]\noverride_names = [\"clippy.toml\", \".cargo/config.toml\", \"rust-toolchain.toml\"]";

    assert_eq!(
        lines(&tree, text),
        [
            override_line("crates/x/.cargo/config.toml"),
            override_line("crates/x/clippy.toml"),
            override_line("deep/er/clippy.toml"),
            override_line("rust-toolchain.toml"),
        ]
    );
}

#[test]
fn an_exempt_path_and_a_listed_file_are_not_overrides() {
    let tree = Memory::default()
        .same("clippy.toml", "a")
        .in_work("xtask/tests/fixtures/a/clippy.toml", "a")
        .in_work("tests/fixtures-not/clippy.toml", "a")
        .in_work("rustfmt.toml", "a");
    let text = "files = [\"clippy.toml\", \"rustfmt.toml\"]\noverride_names = [\"clippy.toml\", \"rustfmt.toml\"]\noverride_exempt = [\"xtask/tests/fixtures\", \"tests/fixtures/\"]";

    assert_eq!(
        lines(&tree, text),
        [override_line("tests/fixtures-not/clippy.toml")]
    );
}

#[test]
fn nothing_is_an_override_while_the_base_ref_holds_no_protected_item() {
    let tree = Memory::default()
        .in_work("clippy.toml", "a")
        .in_work("crates/x/clippy.toml", "a")
        .in_work("Cargo.toml", CARGO)
        .in_work("budgets.json", BUDGETS);
    let text = format!(
        "files = [\"clippy.toml\"]\ndirectories = [\"xtask/tests/\"]\n{CARGO_TABLE}\n{BUDGET_VALUES}\noverride_names = [\"clippy.toml\"]"
    );

    assert_eq!(lines(&tree, &text), Vec::<String>::new());
}

#[test]
fn each_kind_of_protected_item_on_the_base_ref_switches_the_override_check_on() {
    let nested = |tree: Memory| tree.in_work("crates/x/clippy.toml", "a");
    let expected = [override_line("crates/x/clippy.toml")];
    let names = "override_names = [\"clippy.toml\"]";

    let by_file = nested(Memory::default().same("deny.toml", "a"));
    let by_directory = nested(Memory::default().same("xtask/tests/one.rs", "a"));
    let by_table = nested(Memory::default().same("Cargo.toml", CARGO));
    let by_value = nested(Memory::default().same("budgets.json", BUDGETS));

    assert_eq!(
        lines(&by_file, &format!("files = [\"deny.toml\"]\n{names}")),
        expected
    );
    assert_eq!(
        lines(
            &by_directory,
            &format!("directories = [\"xtask/tests/\"]\n{names}")
        ),
        expected
    );
    assert_eq!(
        lines(&by_table, &format!("{CARGO_TABLE}\n{names}")),
        expected
    );
    assert_eq!(
        lines(&by_value, &format!("{BUDGET_VALUES}\n{names}")),
        expected
    );
}

#[test]
fn a_table_is_compared_as_a_value_whatever_the_order_and_the_comments() {
    let reordered = "# comment\n[dependencies]\nserde = \"2\"\nanyhow = \"1\"\n\n[workspace.lints.clippy]\n# another\npedantic = \"warn\"\nall = \"warn\"\n";
    let tree = Memory::default()
        .on_base("Cargo.toml", CARGO)
        .in_work("Cargo.toml", reordered);

    assert_eq!(lines(&tree, CARGO_TABLE), Vec::<String>::new());
}

#[test]
fn a_changed_or_missing_table_or_file_is_a_modification() {
    let expected = [table_line("Cargo.toml", "workspace.lints")];
    let level = CARGO.replace("pedantic = \"warn\"", "pedantic = \"allow\"");
    let without = "[dependencies]\nserde = \"1\"\n";
    let added = CARGO.replace("all = \"warn\"", "all = \"warn\"\nnursery = \"warn\"");

    for (name, work) in [
        ("level", Some(level)),
        ("table deleted", Some(without.to_owned())),
        ("key added", Some(added)),
        ("not toml", Some("workspace = [".to_owned())),
        ("file deleted", None),
    ] {
        let mut tree = Memory::default().on_base("Cargo.toml", CARGO);
        if let Some(text) = work {
            tree = tree.in_work("Cargo.toml", &text);
        }

        assert_eq!(lines(&tree, CARGO_TABLE), expected, "{name}");
    }
}

#[test]
fn a_table_that_the_base_ref_lacks_is_new_and_a_dotted_path_walks_nested_tables() {
    let tree = Memory::default()
        .on_base("Cargo.toml", "[package]\nname = \"a\"\nversion = 1\n")
        .in_work("Cargo.toml", CARGO);
    let text = "manifest_tables = [{ file = \"Cargo.toml\", table = \"workspace.lints\" }, { file = \"Cargo.toml\", table = \"package.name.deeper\" }, { file = \"absent.toml\", table = \"x\" }]";

    assert_eq!(lines(&tree, text), Vec::<String>::new());

    let nested = Memory::default()
        .on_base("Cargo.toml", CARGO)
        .in_work("Cargo.toml", &CARGO.replace("\"warn\"", "\"deny\""));
    let narrow =
        "manifest_tables = [{ file = \"Cargo.toml\", table = \"workspace.lints.clippy\" }]";

    assert_eq!(
        lines(&nested, narrow),
        [table_line("Cargo.toml", "workspace.lints.clippy")]
    );
}

#[test]
fn a_manifest_that_cannot_be_parsed_at_the_base_ref_stops_the_guard() {
    let tree = Memory::default().on_base("Cargo.toml", "workspace = [");

    let error = check(&list(CARGO_TABLE), &tree).unwrap_err();

    assert!(matches!(&error, Failure::InvalidBase { file, .. } if file == "Cargo.toml"));
    assert!(
        error
            .to_string()
            .starts_with("cannot parse Cargo.toml at the base ref: "),
        "{error}"
    );
}

#[test]
fn a_json_value_is_compared_at_its_pointer_and_the_rest_of_the_file_is_free() {
    let changed = BUDGETS.replace("\"extra\":1", "\"extra\":2,\"more\":[1]");
    let tree = Memory::default()
        .on_base("budgets.json", BUDGETS)
        .in_work("budgets.json", &changed);

    assert_eq!(lines(&tree, BUDGET_VALUES), Vec::<String>::new());
}

#[test]
fn a_changed_or_missing_json_value_or_file_is_a_modification() {
    let pointer = "/endpoints/health/smoke/threshold";
    let changed = BUDGETS.replace("50", "500");
    let gone = r#"{"endpoints":{"health":{"stress":{"threshold":80}}}}"#;
    let both = r#"{"endpoints":{"health":{"smoke":{"threshold":50}}}}"#;

    for (name, work, expected) in [
        (
            "changed",
            Some(changed),
            vec![value_line("budgets.json", pointer)],
        ),
        (
            "pointer missing",
            Some(gone.to_owned()),
            vec![value_line("budgets.json", pointer)],
        ),
        (
            "other pointer missing",
            Some(both.to_owned()),
            vec![value_line(
                "budgets.json",
                "/endpoints/health/stress/threshold",
            )],
        ),
        (
            "not json",
            Some("{".to_owned()),
            vec![
                value_line("budgets.json", pointer),
                value_line("budgets.json", "/endpoints/health/stress/threshold"),
            ],
        ),
        (
            "file deleted",
            None,
            vec![
                value_line("budgets.json", pointer),
                value_line("budgets.json", "/endpoints/health/stress/threshold"),
            ],
        ),
    ] {
        let mut tree = Memory::default().on_base("budgets.json", BUDGETS);
        if let Some(text) = work {
            tree = tree.in_work("budgets.json", &text);
        }

        assert_eq!(lines(&tree, BUDGET_VALUES), expected, "{name}");
    }
}

#[test]
fn a_json_value_or_file_that_the_base_ref_lacks_is_new() {
    let tree = Memory::default()
        .on_base("budgets.json", r#"{"endpoints":{}}"#)
        .in_work("budgets.json", BUDGETS)
        .in_work("other.json", "{}");
    let text = "json_values = [{ file = \"budgets.json\", pointers = [\"/endpoints/health/smoke/threshold\"] }, { file = \"other.json\", pointers = [\"/a\"] }, { file = \"absent.json\", pointers = [\"/a\"] }]";

    assert_eq!(lines(&tree, text), Vec::<String>::new());
}

#[test]
fn a_json_file_that_cannot_be_parsed_at_the_base_ref_stops_the_guard() {
    let tree = Memory::default().on_base("budgets.json", "{");

    let error = check(&list(BUDGET_VALUES), &tree).unwrap_err();

    assert!(matches!(&error, Failure::InvalidBase { file, .. } if file == "budgets.json"));
}

#[test]
fn a_required_directory_that_the_working_tree_lacks_is_missing() {
    let tree = Memory::default().directory("crates");
    let text = "required_directories = [\"crates\", \"xtask\", \"tests/Load\"]";

    let findings = check(&list(text), &tree).unwrap();

    assert_eq!(
        findings.missing,
        [
            "Required directory is missing: xtask",
            "Required directory is missing: tests/Load"
        ]
    );
    assert_eq!(findings.modifications, Vec::<String>::new());
}

#[test]
fn findings_come_in_the_order_files_overrides_tables_values() {
    let tree = Memory::default()
        .on_base("clippy.toml", "a")
        .on_base("Cargo.toml", CARGO)
        .in_work("Cargo.toml", "[workspace]\n")
        .on_base("budgets.json", BUDGETS)
        .in_work("crates/x/clippy.toml", "a");
    let text = format!(
        "files = [\"clippy.toml\"]\n{CARGO_TABLE}\n{BUDGET_VALUES}\noverride_names = [\"clippy.toml\"]"
    );

    let findings = check(&list(&text), &tree).unwrap();

    assert_eq!(
        findings.modifications,
        [
            file_line("clippy.toml"),
            override_line("crates/x/clippy.toml"),
            table_line("Cargo.toml", "workspace.lints"),
            value_line("budgets.json", "/endpoints/health/smoke/threshold"),
            value_line("budgets.json", "/endpoints/health/stress/threshold"),
        ]
    );
}

#[test]
fn a_changed_executable_bit_is_a_modification_even_with_equal_bytes() {
    let tree = Memory::default()
        .same("kept", "a")
        .same("lost", "a")
        .base_as("lost", Mode::Executable)
        .same("gained", "a")
        .work_as("gained", Mode::Executable)
        .same("still", "a")
        .base_as("still", Mode::Executable)
        .work_as("still", Mode::Executable);
    let text = "files = [\"kept\", \"lost\", \"gained\", \"still\"]";

    assert_eq!(lines(&tree, text), [file_line("gained"), file_line("lost")]);
}

#[test]
fn a_link_in_place_of_a_file_is_a_modification_and_an_unchanged_link_is_not() {
    let tree = Memory::default()
        .same("file", "a")
        .work_as("file", Mode::Link)
        .same("link", "target")
        .base_as("link", Mode::Link)
        .work_as("link", Mode::Link)
        .on_base("retargeted", "one")
        .base_as("retargeted", Mode::Link)
        .in_work("retargeted", "two")
        .work_as("retargeted", Mode::Link);
    let text = "files = [\"file\", \"link\", \"retargeted\"]";

    assert_eq!(
        lines(&tree, text),
        [file_line("file"), file_line("retargeted")]
    );
}

#[test]
fn a_protected_directory_that_is_a_link_or_a_file_is_a_modification_of_the_directory() {
    let text = "directories = [\"guarded/\"]";
    for mode in [Mode::Link, Mode::Plain, Mode::Executable] {
        let tree = Memory::default()
            .on_base("guarded/one.txt", "1")
            .in_work("guarded", "x")
            .work_as("guarded", mode);

        assert_eq!(
            lines(&tree, text),
            [file_line("guarded"), file_line("guarded/one.txt")],
            "{mode:?}"
        );
    }
}

#[test]
fn a_protected_directory_that_is_a_directory_or_is_gone_adds_no_line_of_its_own() {
    let text = "directories = [\"guarded/\"]";
    let kept = Memory::default()
        .same("guarded/one.txt", "1")
        .directory("guarded");
    let gone = Memory::default().on_base("guarded/one.txt", "1");

    assert_eq!(lines(&kept, text), Vec::<String>::new());
    assert_eq!(lines(&gone, text), [file_line("guarded/one.txt")]);
}

#[test]
fn a_protected_directory_that_the_base_ref_holds_as_a_link_is_protected_as_that_link() {
    let text = "directories = [\"guarded/\"]";
    let same = Memory::default()
        .same("guarded", "elsewhere")
        .base_as("guarded", Mode::Link)
        .work_as("guarded", Mode::Link);
    let retargeted = Memory::default()
        .on_base("guarded", "elsewhere")
        .base_as("guarded", Mode::Link)
        .in_work("guarded", "somewhere")
        .work_as("guarded", Mode::Link);
    let other = Memory::default().same("guarded-two/file", "x");

    assert_eq!(lines(&same, text), Vec::<String>::new());
    assert_eq!(lines(&retargeted, text), [file_line("guarded")]);
    assert_eq!(lines(&other, text), Vec::<String>::new());
}
