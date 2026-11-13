//! Unit tests of the parent module.

use super::{Failure, directory, parse};

const LIST: &str = r#"
approval_label = "approved-config-change"
files = ["clippy.toml", "deny.toml"]
directories = ["xtask/tests/", "scripts"]
manifest_tables = [{ file = "Cargo.toml", table = "workspace.lints" }]
json_values = [
    { file = "budgets.json", pointers = ["/a/threshold", "/b/threshold"] },
    { file = "other.json", pointers = ["/c"] },
]
override_names = ["clippy.toml", ".cargo/config.toml"]
override_exempt = ["fixtures/"]
required_directories = ["crates", "xtask"]
"#;

#[test]
fn every_key_of_the_schema_is_read() {
    let list = parse(LIST.as_bytes()).unwrap();

    assert_eq!(list.approval_label, "approved-config-change");
    assert_eq!(list.files, ["clippy.toml", "deny.toml"]);
    assert_eq!(list.directories, ["xtask/tests/", "scripts"]);
    assert_eq!(list.manifest_tables.len(), 1);
    assert_eq!(list.manifest_tables[0].file, "Cargo.toml");
    assert_eq!(list.manifest_tables[0].table, "workspace.lints");
    assert_eq!(list.json_values.len(), 2);
    assert_eq!(list.json_values[0].file, "budgets.json");
    assert_eq!(
        list.json_values[0].pointers,
        ["/a/threshold", "/b/threshold"]
    );
    assert_eq!(list.override_names, ["clippy.toml", ".cargo/config.toml"]);
    assert_eq!(list.override_exempt, ["fixtures/"]);
    assert_eq!(list.required_directories, ["crates", "xtask"]);
}

#[test]
fn only_the_label_is_required() {
    let list = parse(b"approval_label = \"ok\"").unwrap();

    assert_eq!(list.approval_label, "ok");
    assert_eq!(list.items(), Vec::<String>::new());
    assert!(matches!(
        parse(b"files = []").unwrap_err(),
        Failure::InvalidList(_)
    ));
}

#[test]
fn an_unknown_key_is_an_error_at_every_level() {
    for text in [
        "approval_label = \"x\"\nfile = []",
        "approval_label = \"x\"\nmanifest_tables = [{ file = \"a\", table = \"b\", extra = 1 }]",
        "approval_label = \"x\"\njson_values = [{ file = \"a\", pointers = [], extra = 1 }]",
    ] {
        let error = parse(text.as_bytes()).unwrap_err();

        assert!(matches!(error, Failure::InvalidList(_)), "{text}");
        assert!(
            error
                .to_string()
                .starts_with("invalid protected-config.toml: ")
        );
    }
}

#[test]
fn items_are_files_directories_tables_and_values_in_that_order() {
    let list = parse(LIST.as_bytes()).unwrap();

    assert_eq!(
        list.items(),
        [
            "clippy.toml",
            "deny.toml",
            "xtask/tests/",
            "scripts/",
            "Cargo.toml#workspace.lints",
            "budgets.json#/a/threshold",
            "budgets.json#/b/threshold",
            "other.json#/c",
        ]
    );
}

#[test]
fn a_directory_ends_with_exactly_one_slash() {
    assert_eq!(directory("a/b"), "a/b/");
    assert_eq!(directory("a/b/"), "a/b/");
    assert_eq!(directory("a/b//"), "a/b/");
}
