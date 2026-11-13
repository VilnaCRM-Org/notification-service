//! Unit tests of the parent module, against the fixture workspace and the fixture models.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::super::Failure;
use super::super::model::Crate;
use super::super::model::{Outside, Reach};
use super::{Edges, config, inherits_lints, is_internal, reach_from, workspace};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/arch");

fn fixture(path: &str) -> PathBuf {
    Path::new(FIXTURES).join(path)
}

fn members() -> Vec<Crate> {
    workspace(&fixture("workspace/Cargo.toml")).unwrap().crates
}

fn find<'members>(members: &'members [Crate], name: &str) -> &'members Crate {
    members.iter().find(|member| member.name == name).unwrap()
}

fn dependency(name: &str, path: Option<&str>) -> cargo_metadata::Dependency {
    serde_json::from_value(json!({
        "name": name,
        "source": null,
        "req": "*",
        "kind": null,
        "optional": false,
        "uses_default_features": true,
        "features": [],
        "target": null,
        "rename": null,
        "registry": null,
        "path": path,
    }))
    .unwrap()
}

#[test]
fn lists_every_member_sorted_by_name_with_its_manifest_relative_to_the_workspace_root() {
    let found: Vec<(String, String)> = members()
        .into_iter()
        .map(|member| (member.name, member.manifest))
        .collect();

    let expected = [
        ("audit-domain", "crates/audit/domain"),
        ("billing-application", "crates/billing/application"),
        ("billing-domain", "crates/billing/domain"),
        ("ledger-domain", "crates/ledger/domain"),
        ("misplaced-domain", "crates/misplaced/core"),
        ("nolints-domain", "crates/nolints/domain"),
        ("overlap-domain", "crates/overlap/domain"),
        ("reports-application", "crates/reports/application"),
        ("shared-domain", "crates/shared/domain"),
        ("shop-application", "crates/shop/application"),
        ("shop-domain", "crates/shop/domain"),
        ("shop-infrastructure", "crates/shop/infrastructure"),
        ("stray", "crates/stray"),
        ("test-support", "crates/test-support"),
    ];
    let expected: Vec<(String, String)> = expected
        .iter()
        .map(|(name, directory)| ((*name).to_owned(), format!("{directory}/Cargo.toml")))
        .collect();
    assert_eq!(found, expected);
}

#[test]
fn only_an_explicit_workspace_true_counts_as_inheriting_the_lints() {
    let members = members();

    assert!(find(&members, "shop-domain").inherits_lints);
    assert!(!find(&members, "nolints-domain").inherits_lints);
}

#[test]
fn a_manifest_that_cannot_be_read_or_parsed_does_not_inherit_the_lints() {
    assert!(!inherits_lints(&fixture("no-such-manifest.toml")));
    // A file that is not TOML at all.
    assert!(!inherits_lints(&fixture("workspace/crates/stray/lib.txt")));
    // A manifest of a workspace root without a `[lints]` table of its own.
    assert!(!inherits_lints(&fixture("workspace/Cargo.toml")));
}

#[test]
fn direct_dependencies_know_whether_they_are_dev_and_whether_they_are_members() {
    let members = members();
    let facts = |name: &str| -> Vec<(String, bool, bool)> {
        find(&members, name)
            .dependencies
            .iter()
            .map(|dependency| (dependency.name.clone(), dependency.dev, dependency.internal))
            .collect()
    };

    assert_eq!(
        facts("shop-domain"),
        [("shop-infrastructure".to_owned(), false, true)]
    );
    assert_eq!(
        facts("shop-application"),
        [
            ("shop-domain".to_owned(), false, true),
            ("test-support".to_owned(), false, true)
        ]
    );
    assert_eq!(
        facts("billing-application"),
        [
            ("billing-domain".to_owned(), false, true),
            ("rstest".to_owned(), true, false),
            ("test-support".to_owned(), true, true)
        ]
    );
    assert_eq!(
        facts("billing-domain"),
        [("shared-domain".to_owned(), false, true)]
    );
    assert_eq!(facts("ledger-domain"), [("serde".to_owned(), false, false)]);
    assert_eq!(facts("stray"), []);
}

#[test]
fn a_dependency_is_internal_when_it_is_a_path_dependency_on_a_member() {
    let members = BTreeSet::from(["member"]);

    assert!(is_internal(
        &dependency("member", Some("/work/member")),
        &members
    ));
    assert!(!is_internal(&dependency("member", None), &members));
    assert!(!is_internal(
        &dependency("vendored", Some("/work/vendored")),
        &members
    ));
    assert!(!is_internal(&dependency("serde", None), &members));
}

#[test]
fn a_missing_manifest_could_not_be_read() {
    let error = workspace(&fixture("no-such-workspace/Cargo.toml")).unwrap_err();

    assert!(matches!(error, Failure::Metadata(_)), "{error}");
    assert!(
        error.to_string().starts_with("cannot read the workspace: "),
        "{error}"
    );
}

#[test]
fn reads_the_layer_model() {
    let model = config(&fixture("valid.toml")).unwrap();

    let layers: Vec<(&str, &[String])> = model
        .layers
        .iter()
        .map(|layer| (layer.name.as_str(), layer.depends_on.as_slice()))
        .collect();
    assert_eq!(layers.len(), 5);
    assert_eq!(model.contexts.shared, ["shared"]);
    assert_eq!(model.layers[0].directory.as_deref(), Some("domain"));
    assert_eq!(
        model.layers[0].external,
        ["async-trait", "thiserror", "uuid"]
    );
    assert_eq!(
        model.layers[0].dev_external,
        ["proptest", "rstest", "tokio"]
    );
    assert_eq!(
        model.layers[0].forbidden_transitive,
        ["axum", "hyper", "reqwest", "sqlx", "tokio"]
    );
    assert_eq!(model.layers[3].directory, None);
    assert_eq!(layers[0].0, "domain");
    assert_eq!(layers[1].0, "application");
    assert_eq!(layers[1].1, ["domain", "application"]);
    assert_eq!(model.layers[1].dev_depends_on, ["test-support"]);
}

#[test]
fn a_model_without_layers_is_valid() {
    assert!(
        config(&fixture("nothing-covered.toml"))
            .unwrap()
            .layers
            .is_empty()
    );
}

#[test]
fn a_missing_model_could_not_be_read() {
    let error = config(&fixture("no-such-model.toml")).unwrap_err();

    assert!(matches!(error, Failure::Read { .. }), "{error}");
    assert!(error.to_string().starts_with("cannot read "), "{error}");
    assert!(
        error.to_string().contains("no-such-model.toml: "),
        "{error}"
    );
}

#[test]
fn a_model_that_is_not_toml_is_invalid() {
    let error = config(&fixture("workspace/crates/stray/lib.txt")).unwrap_err();

    assert!(matches!(error, Failure::Config { .. }), "{error}");
    assert!(error.to_string().starts_with("invalid "), "{error}");
    assert!(error.to_string().contains("lib.txt: "), "{error}");
}

#[test]
fn a_model_with_an_unknown_key_is_invalid_and_names_the_key() {
    let error = config(&fixture("invalid.toml")).unwrap_err();

    assert!(matches!(error, Failure::UnknownKey { .. }), "{error}");
    assert!(
        error
            .to_string()
            .ends_with("invalid.toml: unknown key `typo` in [[layer]]"),
        "{error}"
    );
}

fn reach_names(member: &Crate) -> Vec<&str> {
    member
        .reach
        .iter()
        .map(|reach| reach.name.as_str())
        .collect()
}

fn chain(member: &Crate, name: &str) -> Vec<String> {
    member
        .reach
        .iter()
        .find(|reach| reach.name == name)
        .unwrap()
        .path
        .clone()
}

#[test]
fn a_crate_reaches_what_its_normal_and_build_dependencies_pull_in_transitively() {
    let members = members();

    let reports = find(&members, "reports-application");
    assert_eq!(reach_names(reports), ["axum", "sqlx"]);
    assert_eq!(chain(reports, "axum"), ["axum"]);
    assert_eq!(chain(reports, "sqlx"), ["axum", "sqlx"]);
    assert_eq!(reach_names(find(&members, "ledger-domain")), ["serde"]);
}

#[test]
fn the_reach_follows_workspace_crates_too() {
    let members = members();

    assert_eq!(
        reach_names(find(&members, "billing-application")),
        ["billing-domain", "shared-domain"]
    );
    assert_eq!(
        chain(find(&members, "billing-application"), "shared-domain"),
        ["billing-domain", "shared-domain"]
    );
}

#[test]
fn a_dev_dependency_adds_nothing_to_the_reach() {
    let members = members();

    // `billing-application` has a dev-dependency on a stand-in that depends on `sqlx`, and on
    // `test-support`.
    let member = find(&members, "billing-application");
    assert!(!reach_names(member).contains(&"sqlx"), "{member:?}");
    assert!(!reach_names(member).contains(&"rstest"), "{member:?}");
    assert!(!reach_names(member).contains(&"test-support"), "{member:?}");
    assert_eq!(reach_names(find(&members, "stray")), Vec::<&str>::new());
}

fn edges(list: &[(&str, &[&str])]) -> Edges {
    list.iter()
        .map(|&(name, targets)| {
            (
                name.to_owned(),
                targets.iter().map(|&target| target.to_owned()).collect(),
            )
        })
        .collect()
}

fn reach(name: &str, path: &[&str]) -> Reach {
    Reach {
        name: name.to_owned(),
        path: path.iter().map(|&step| step.to_owned()).collect(),
    }
}

#[test]
fn the_reach_of_a_package_is_every_package_on_a_path_with_a_shortest_chain_sorted_by_name() {
    let graph = edges(&[
        ("a", &["c", "b"]),
        ("b", &["d"]),
        ("c", &["d", "e"]),
        ("d", &["f"]),
        ("e", &["a"]),
        ("unrelated", &["z"]),
    ]);

    assert_eq!(
        reach_from("a", &graph),
        [
            reach("b", &["b"]),
            reach("c", &["c"]),
            reach("d", &["b", "d"]),
            reach("e", &["c", "e"]),
            reach("f", &["b", "d", "f"]),
        ]
    );
}

#[test]
fn a_package_without_edges_reaches_nothing_and_a_cycle_terminates() {
    let graph = edges(&[("a", &["b"]), ("b", &["a"])]);

    assert_eq!(reach_from("lonely", &graph), []);
    assert_eq!(reach_from("a", &graph), [reach("b", &["b"])]);
}

#[test]
fn a_local_crate_that_is_not_a_member_is_listed_and_a_vendored_one_outside_the_root_is_not() {
    let outside = workspace(&fixture("outside/Cargo.toml")).unwrap().outside;

    assert_eq!(
        outside,
        ["audit-core", "billing-core"].map(|name| Outside {
            name: name.to_owned(),
            manifest: format!("crates/{name}/Cargo.toml"),
        })
    );
    // The stand-ins of third-party crates sit next to the workspace, not below its root.
    let fixture_workspace = workspace(&fixture("workspace/Cargo.toml")).unwrap();
    assert_eq!(fixture_workspace.outside, []);
    assert!(!fixture_workspace.crates.is_empty());
}
