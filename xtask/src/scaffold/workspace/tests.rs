//! Unit tests of the parent module.

use super::super::Fault;
use super::{add_members, members};

fn add(manifest: &str, paths: &[&str]) -> String {
    let paths: Vec<String> = paths.iter().map(|path| (*path).to_owned()).collect();
    add_members(manifest, &paths).unwrap()
}

const BLOCK: &str = "[workspace]\nresolver = \"3\" # keep\nmembers = [\n    # the shared kernel\n    \"crates/shared/domain\",\n    \"crates/shop/domain\", # the shop\n    # the tools\n    \"xtask\",\n]\n\n[workspace.package]\nversion = \"0.1.0\"\n";

#[test]
fn entries_go_to_their_sorted_position_and_every_comment_stays_with_its_entry() {
    let text = add(
        BLOCK,
        &["crates/billing/x/domain", "crates/shop/x/domain", "zz"],
    );

    assert_eq!(
        text,
        "[workspace]\nresolver = \"3\" # keep\nmembers = [\n    \"crates/billing/x/domain\",\n    # the shared kernel\n    \"crates/shared/domain\",\n    \"crates/shop/domain\", # the shop\n    \"crates/shop/x/domain\",\n    # the tools\n    \"xtask\",\n    \"zz\",\n]\n\n[workspace.package]\nversion = \"0.1.0\"\n"
    );
}

#[test]
fn the_rest_of_the_file_is_not_touched() {
    let text = add(BLOCK, &["crates/shop/a"]);

    assert!(text.starts_with("[workspace]\nresolver = \"3\" # keep\nmembers = [\n"));
    assert!(text.ends_with("]\n\n[workspace.package]\nversion = \"0.1.0\"\n"));
}

#[test]
fn a_single_line_array_stays_on_one_line() {
    assert_eq!(
        add("[workspace]\nmembers = [\"b\", \"d\"]\n", &["a", "c", "e"]),
        "[workspace]\nmembers = [\"a\", \"b\", \"c\", \"d\", \"e\"]\n"
    );
}

#[test]
fn a_single_line_array_without_spaces_after_the_bracket_keeps_its_first_prefix() {
    assert_eq!(
        add("[workspace]\nmembers = [ \"b\" ]\n", &["a"]),
        "[workspace]\nmembers = [ \"a\", \"b\" ]\n"
    );
}

#[test]
fn a_single_entry_is_followed_by_a_space() {
    assert_eq!(
        add("[workspace]\nmembers = [\"a\"]\n", &["b"]),
        "[workspace]\nmembers = [\"a\", \"b\"]\n"
    );
}

#[test]
fn an_empty_array_is_filled() {
    assert_eq!(
        add("[workspace]\nmembers = []\n", &["a", "b"]),
        "[workspace]\nmembers = [\"a\", \"b\"]\n"
    );
}

#[test]
fn a_four_space_indentation_is_kept_and_a_two_space_one_too() {
    let two = add("[workspace]\nmembers = [\n  \"b\",\n]\n", &["a", "c"]);
    let four = add("[workspace]\nmembers = [\n    \"b\",\n]\n", &["a", "c"]);

    assert_eq!(
        two,
        "[workspace]\nmembers = [\n  \"a\",\n  \"b\",\n  \"c\",\n]\n"
    );
    assert_eq!(
        four,
        "[workspace]\nmembers = [\n    \"a\",\n    \"b\",\n    \"c\",\n]\n"
    );
}

#[test]
fn entries_that_are_not_strings_are_ignored_when_sorting() {
    assert_eq!(
        add("[workspace]\nmembers = [1, \"b\"]\n", &["a"]),
        "[workspace]\nmembers = [1, \"a\", \"b\"]\n"
    );
}

#[test]
fn a_manifest_that_does_not_parse_is_an_error() {
    let error = add_members("[workspace\n", &[]).unwrap_err();

    assert!(matches!(error, Fault::Parse(_)), "{error}");
    assert!(
        error.to_string().starts_with("cannot parse Cargo.toml: "),
        "{error}"
    );
}

#[test]
fn a_manifest_without_members_is_an_error() {
    for text in [
        "[package]\nname = \"x\"\n",
        "[workspace]\nresolver = \"3\"\n",
        "[workspace]\nmembers = \"crates\"\n",
        "workspace = 1\n",
    ] {
        let error = add_members(text, &[]).unwrap_err();

        assert!(matches!(error, Fault::NoMembers), "{text}: {error}");
        assert_eq!(
            error.to_string(),
            "Cargo.toml has no [workspace].members array"
        );
    }
}

#[test]
fn a_comment_after_the_last_comma_stays_with_the_last_entry() {
    assert_eq!(
        add(
            "[workspace]\nmembers = [\n    \"a\",\n    \"xtask\", # tools\n]\n",
            &["z"]
        ),
        "[workspace]\nmembers = [\n    \"a\",\n    \"xtask\", # tools\n    \"z\",\n]\n"
    );
}

#[test]
fn an_array_without_a_trailing_comma_keeps_it_that_way() {
    assert_eq!(
        add(
            "[workspace]\nmembers = [\n    \"a\",\n    \"c\"\n]\n",
            &["b", "d"]
        ),
        "[workspace]\nmembers = [\n    \"a\",\n    \"b\",\n    \"c\",\n    \"d\"\n]\n"
    );
}

#[test]
fn an_entry_equal_to_a_listed_one_goes_behind_it_and_the_comment_stays_with_the_listed_one() {
    // The command refuses a context that exists, so this only happens for a member without a
    // directory; cargo then rejects the duplicate, which the lock step reports.
    assert_eq!(
        add(
            "[workspace]\nmembers = [\n    # the first\n    \"a\",\n    \"b\",\n]\n",
            &["a"]
        ),
        "[workspace]\nmembers = [\n    # the first\n    \"a\",\n    \"a\",\n    \"b\",\n]\n"
    );
}

#[test]
fn the_members_are_listed_as_written_and_a_manifest_without_any_lists_none() {
    assert_eq!(
        members(BLOCK),
        ["crates/shared/domain", "crates/shop/domain", "xtask"]
    );
    assert_eq!(members("[workspace]\nmembers = [1, \"a\"]\n"), ["a"]);
    assert_eq!(members("[workspace]\n"), Vec::<String>::new());
    assert_eq!(members("[workspace\n"), Vec::<String>::new());
}
