//! Unit tests of the parent module.

use super::super::config::{Config, Contexts, Layer};
use super::super::model::{Crate, Dependency, Outside, Reach, Workspace};
use super::{Rule, Violation, check};

fn owned(items: &[&str]) -> Vec<String> {
    items.iter().map(|&item| item.to_owned()).collect()
}

fn layer(name: &str, crates: &str, depends_on: &[&str], dev_depends_on: &[&str]) -> Layer {
    Layer {
        name: name.to_owned(),
        crates: owned(&[crates]),
        depends_on: owned(depends_on),
        dev_depends_on: owned(dev_depends_on),
        ..Layer::default()
    }
}

fn config() -> Config {
    Config {
        contexts: Contexts {
            shared: owned(&["shared"]),
        },
        layers: vec![
            Layer {
                directory: Some("domain".to_owned()),
                external: owned(&["thiserror"]),
                dev_external: owned(&["rstest"]),
                forbidden_transitive: owned(&["sqlx"]),
                ..layer("domain", "*-domain", &["domain"], &[])
            },
            Layer {
                directory: Some("application".to_owned()),
                external: owned(&["serde", "axum"]),
                forbidden_transitive: owned(&["sqlx", "moka"]),
                ..layer(
                    "application",
                    "*-application",
                    &["domain", "application"],
                    &["test-support"],
                )
            },
            layer("test-support", "test-support", &["domain"], &[]),
            Layer {
                external: owned(&["*"]),
                ..layer(
                    "composition-root",
                    "service",
                    &["domain", "application"],
                    &[],
                )
            },
        ],
    }
}

/// A dependency as `(name, dev, internal)`.
type Edge = (&'static str, bool, bool);

fn member(name: &str, dependencies: &[Edge]) -> Crate {
    Crate {
        name: name.to_owned(),
        manifest: manifest_of(name),
        inherits_lints: true,
        dependencies: dependencies
            .iter()
            .map(|&(name, dev, internal)| Dependency {
                name: name.to_owned(),
                dev,
                internal,
            })
            .collect(),
        reach: Vec::new(),
    }
}

/// `crates/<context>/<layer>/Cargo.toml` for `<context>-<layer>`, `crates/<name>/Cargo.toml` else.
fn manifest_of(name: &str) -> String {
    for layer in ["domain", "application", "infrastructure"] {
        if let Some(context) = name.strip_suffix(&format!("-{layer}")) {
            return format!("crates/{context}/{layer}/Cargo.toml");
        }
    }
    format!("crates/{name}/Cargo.toml")
}

/// The crate with the packages it reaches, each as a chain whose last element is the package.
fn reaching(mut member: Crate, chains: &[&[&str]]) -> Crate {
    member.reach = chains
        .iter()
        .map(|chain| Reach {
            name: (*chain.last().unwrap()).to_owned(),
            path: owned(chain),
        })
        .collect();
    member
}

/// The crate placed at `manifest`.
fn placed(mut member: Crate, manifest: &str) -> Crate {
    manifest.clone_into(&mut member.manifest);
    member
}

fn check_one(config: &Config, member: Crate) -> Vec<Violation> {
    check(
        config,
        &Workspace {
            crates: vec![member],
            ..Workspace::default()
        },
    )
}

fn rules(violations: &[Violation]) -> Vec<Rule> {
    violations.iter().map(|violation| violation.rule).collect()
}

#[test]
fn rule_identifiers_are_stable() {
    assert_eq!(Rule::Uncovered.id(), "uncovered");
    assert_eq!(Rule::Ambiguous.id(), "ambiguous");
    assert_eq!(Rule::Layer.id(), "layer");
    assert_eq!(Rule::Context.id(), "context");
    assert_eq!(Rule::External.id(), "external");
    assert_eq!(Rule::Transitive.id(), "transitive");
    assert_eq!(Rule::Path.id(), "path");
    assert_eq!(Rule::Lints.id(), "lints");
}

#[test]
fn a_workspace_that_follows_the_rules_is_clean() {
    let workspace = Workspace {
        crates: vec![
            member("shop-domain", &[]),
            member("shop-application", &[("shop-domain", false, true)]),
            member("test-support", &[("shop-domain", false, true)]),
            member(
                "service",
                &[("shop-application", false, true), ("serde", false, false)],
            ),
        ],
        ..Workspace::default()
    };

    assert_eq!(check(&config(), &workspace), []);
}

#[test]
fn a_layer_may_depend_on_itself() {
    let workspace = Workspace {
        crates: vec![
            member("shared-application", &[]),
            member(
                "billing-application",
                &[("shared-application", false, true)],
            ),
        ],
        ..Workspace::default()
    };

    assert_eq!(check(&config(), &workspace), []);
}

#[test]
fn a_crate_that_matches_no_layer_is_uncovered() {
    let violations = check_one(&config(), member("stray", &[]));

    assert_eq!(
        violations,
        [Violation {
            rule: Rule::Uncovered,
            manifest: "crates/stray/Cargo.toml".to_owned(),
            message: "`stray` is not assigned to any layer".to_owned(),
            hint:
                "name the crate `<context>-<layer>` or add its name to the `crates` of a [[layer]]"
                    .to_owned(),
        }]
    );
}

#[test]
fn a_crate_that_matches_two_layers_is_ambiguous() {
    let mut config = config();
    config.layers.push(layer("shop", "shop-*", &[], &[]));

    let violations = check_one(
        &config,
        member("shop-domain", &[("shop-application", false, true)]),
    );

    assert_eq!(
        violations,
        [Violation {
            rule: Rule::Ambiguous,
            manifest: "crates/shop/domain/Cargo.toml".to_owned(),
            message: "`shop-domain` matches more than one layer: domain, shop".to_owned(),
            hint: "make the `crates` patterns of the [[layer]] tables mutually exclusive"
                .to_owned(),
        }]
    );
}

#[test]
fn the_dependencies_of_an_ambiguous_or_uncovered_crate_are_not_judged() {
    let mut config = config();
    config.layers.push(layer("shop", "shop-*", &[], &[]));
    let dependencies = [("service", false, true)];

    assert_eq!(
        rules(&check_one(&config, member("shop-domain", &dependencies))),
        [Rule::Ambiguous]
    );
    assert_eq!(
        rules(&check_one(&config, member("stray", &dependencies))),
        [Rule::Uncovered]
    );
}

#[test]
fn a_crate_without_the_workspace_lints_breaks_the_lints_rule() {
    let mut crate_without_lints = member("shop-domain", &[]);
    crate_without_lints.inherits_lints = false;

    let violations = check_one(&config(), crate_without_lints);

    assert_eq!(
        violations,
        [Violation {
            rule: Rule::Lints,
            manifest: "crates/shop/domain/Cargo.toml".to_owned(),
            message: "`shop-domain` does not inherit the workspace lint policy".to_owned(),
            hint: "add `[lints]` with `workspace = true` to its Cargo.toml".to_owned(),
        }]
    );
}

#[test]
fn the_lints_rule_applies_to_a_crate_outside_every_layer_too() {
    let mut stray = member("stray", &[]);
    stray.inherits_lints = false;

    assert_eq!(
        rules(&check_one(&config(), stray)),
        [Rule::Lints, Rule::Uncovered]
    );
}

#[test]
fn domain_must_not_depend_on_application() {
    let violations = check_one(
        &config(),
        member("shop-domain", &[("shop-application", false, true)]),
    );

    assert_eq!(
        violations,
        [Violation {
            rule: Rule::Layer,
            manifest: "crates/shop/domain/Cargo.toml".to_owned(),
            message: "`shop-domain` (layer `domain`, context `shop`) must not depend on \
                      `shop-application` (layer `application`, context `shop`)"
                .to_owned(),
            hint: "layer `domain` may depend on: [domain]; declare a port (trait) in the inner \
                   layer and implement it in the outer one"
                .to_owned(),
        }]
    );
}

#[test]
fn a_crate_without_a_context_is_named_by_its_layer_alone() {
    let violations = check_one(
        &config(),
        member("test-support", &[("service", false, true)]),
    );

    assert_eq!(
        violations[0].message,
        "`test-support` (layer `test-support`) must not depend on `service` (layer `composition-root`)"
    );
}

#[test]
fn a_dev_dependency_on_a_dev_layer_is_allowed_and_the_same_normal_dependency_is_not() {
    let allowed = check_one(
        &config(),
        member("shop-application", &[("test-support", true, true)]),
    );
    let refused = check_one(
        &config(),
        member("shop-application", &[("test-support", false, true)]),
    );

    assert_eq!(allowed, []);
    assert_eq!(rules(&refused), [Rule::Layer]);
    assert_eq!(
        refused[0].message,
        "`shop-application` (layer `application`, context `shop`) must not depend on `test-support` (layer `test-support`)"
    );
    assert_eq!(
        refused[0].hint,
        "layer `application` may depend on: [domain, application] and, as dev-dependencies, on: \
         [test-support]; declare a port (trait) in the inner layer and implement it in the outer one"
    );
}

#[test]
fn a_dev_dependency_may_use_the_normal_allow_list_too() {
    let violations = check_one(
        &config(),
        member("shop-application", &[("shop-domain", true, true)]),
    );

    assert_eq!(violations, []);
}

#[test]
fn a_dev_dependency_outside_both_lists_is_refused_as_a_dev_dependency() {
    let violations = check_one(
        &config(),
        member("shop-domain", &[("test-support", true, true)]),
    );

    assert_eq!(rules(&violations), [Rule::Layer]);
    assert_eq!(
        violations[0].message,
        "`shop-domain` (layer `domain`, context `shop`) must not have a dev-dependency on `test-support` (layer `test-support`)"
    );
}

#[test]
fn third_party_dependencies_are_not_layer_edges() {
    let violations = check_one(
        &config(),
        member("shop-domain", &[("shop-application", false, false)]),
    );

    assert_eq!(rules(&violations), [Rule::External]);
}

#[test]
fn a_dependency_on_a_crate_without_a_layer_is_not_judged_here() {
    let violations = check_one(&config(), member("shop-domain", &[("stray", false, true)]));

    assert_eq!(violations, []);
}

#[test]
fn every_offending_edge_is_reported_in_manifest_order() {
    let violations = check_one(
        &config(),
        member(
            "shop-domain",
            &[
                ("shop-application", false, true),
                ("shop-domain", false, true),
                ("service", false, true),
            ],
        ),
    );

    assert_eq!(rules(&violations), [Rule::Layer, Rule::Layer]);
    assert!(
        violations[0].message.contains("`shop-application`"),
        "{violations:?}"
    );
    assert!(
        violations[1].message.contains("`service`"),
        "{violations:?}"
    );
}

#[test]
fn violations_of_every_crate_are_collected_in_workspace_order() {
    let workspace = Workspace {
        crates: vec![
            member("stray", &[]),
            member("shop-domain", &[("service", false, true)]),
        ],
        ..Workspace::default()
    };

    let violations = check(&config(), &workspace);

    assert_eq!(rules(&violations), [Rule::Uncovered, Rule::Layer]);
    assert_eq!(violations[0].manifest, "crates/stray/Cargo.toml");
    assert_eq!(violations[1].manifest, "crates/shop/domain/Cargo.toml");
}

#[test]
fn a_context_must_not_depend_on_another_context() {
    let violations = check_one(
        &config(),
        member("audit-domain", &[("shop-domain", false, true)]),
    );

    assert_eq!(
        violations,
        [Violation {
            rule: Rule::Context,
            manifest: "crates/audit/domain/Cargo.toml".to_owned(),
            message: "`audit-domain` (layer `domain`, context `audit`) must not depend on \
                      `shop-domain` (layer `domain`, context `shop`)"
                .to_owned(),
            hint: "context `audit` may depend on itself and on the shared contexts: [shared]; \
                   share what both contexts need through a shared crate"
                .to_owned(),
        }]
    );
}

#[test]
fn a_context_may_depend_on_a_shared_context_and_on_itself() {
    let workspace = Workspace {
        crates: vec![
            member("shared-domain", &[]),
            member("shop-domain", &[("shared-domain", false, true)]),
            member("shop-application", &[("shop-domain", false, true)]),
        ],
        ..Workspace::default()
    };

    assert_eq!(check(&config(), &workspace), []);
}

#[test]
fn a_shared_context_is_judged_like_any_other_when_it_depends_on_a_context() {
    let violations = check_one(
        &config(),
        member("shared-domain", &[("shop-domain", false, true)]),
    );

    assert_eq!(rules(&violations), [Rule::Context]);
}

#[test]
fn the_context_rule_applies_to_dev_dependencies_and_to_the_layer_rule_together() {
    let violations = check_one(
        &config(),
        member(
            "shop-domain",
            &[
                ("audit-application", true, true),
                ("billing-domain", true, true),
            ],
        ),
    );

    assert_eq!(
        rules(&violations),
        [Rule::Layer, Rule::Context, Rule::Context]
    );
}

#[test]
fn crates_without_a_context_are_not_judged_by_the_context_rule() {
    let from_root = check_one(
        &config(),
        member("service", &[("shop-application", false, true)]),
    );
    let to_support = check_one(
        &config(),
        member("audit-application", &[("test-support", true, true)]),
    );

    assert_eq!(from_root, []);
    assert_eq!(to_support, []);
}

#[test]
fn a_target_without_exactly_one_layer_is_not_judged_by_the_context_rule() {
    let mut config = config();
    config.layers.push(layer("shop", "shop-*", &[], &[]));
    let dependencies = [("shop-application", false, true), ("stray", false, true)];

    assert_eq!(
        check_one(&config, member("audit-domain", &dependencies)),
        []
    );
}

#[test]
fn a_third_party_crate_on_the_allow_list_is_allowed() {
    let violations = check_one(
        &config(),
        member("shop-domain", &[("thiserror", false, false)]),
    );

    assert_eq!(violations, []);
}

#[test]
fn a_third_party_crate_outside_the_allow_list_is_refused() {
    let violations = check_one(&config(), member("shop-domain", &[("serde", false, false)]));

    assert_eq!(
        violations,
        [Violation {
            rule: Rule::External,
            manifest: "crates/shop/domain/Cargo.toml".to_owned(),
            message: "`shop-domain` (layer `domain`, context `shop`) must not depend on the \
                      third-party crate `serde`"
                .to_owned(),
            hint: "layer `domain` may use the third-party crates [thiserror]; add the crate to \
                   `external` of the layer only if the architecture allows it"
                .to_owned(),
        }]
    );
}

#[test]
fn a_dev_dependency_may_use_the_dev_allow_list_and_a_normal_dependency_may_not() {
    let allowed = check_one(&config(), member("shop-domain", &[("rstest", true, false)]));
    let normal = check_one(
        &config(),
        member("shop-domain", &[("rstest", false, false)]),
    );
    let listed_normal = check_one(
        &config(),
        member("shop-domain", &[("thiserror", true, false)]),
    );

    assert_eq!(allowed, []);
    assert_eq!(rules(&normal), [Rule::External]);
    assert_eq!(listed_normal, []);
}

#[test]
fn a_dev_dependency_outside_both_allow_lists_is_refused_and_both_lists_are_named() {
    let violations = check_one(
        &config(),
        member("shop-domain", &[("mockall", true, false)]),
    );

    assert_eq!(
        violations,
        [Violation {
            rule: Rule::External,
            manifest: "crates/shop/domain/Cargo.toml".to_owned(),
            message: "`shop-domain` (layer `domain`, context `shop`) must not have a \
                      dev-dependency on the third-party crate `mockall`"
                .to_owned(),
            hint: "layer `domain` may use the third-party crates [thiserror] and, as \
                   dev-dependencies, [rstest]"
                .to_owned(),
        }]
    );
}

#[test]
fn a_star_in_the_allow_list_allows_every_third_party_crate() {
    let violations = check_one(&config(), member("service", &[("anything", false, false)]));

    assert_eq!(violations, []);
}

#[test]
fn a_forbidden_crate_that_is_reachable_breaks_the_transitive_rule() {
    let reaching = reaching(
        member("shop-application", &[]),
        &[&["axum", "sqlx"], &["serde"]],
    );

    let violations = check_one(&config(), reaching);

    assert_eq!(
        violations,
        [Violation {
            rule: Rule::Transitive,
            manifest: "crates/shop/application/Cargo.toml".to_owned(),
            message: "`shop-application` (layer `application`, context `shop`) reaches `sqlx` \
                      through non-dev dependencies: `shop-application` -> `axum` -> `sqlx`"
                .to_owned(),
            hint: "layer `application` must not reach [sqlx, moka]; remove or replace the \
                   dependency that pulls `sqlx` in"
                .to_owned(),
        }]
    );
}

#[test]
fn every_forbidden_crate_that_is_reachable_is_reported_in_reach_order() {
    let reaching = reaching(
        member("shop-application", &[]),
        &[&["moka"], &["serde"], &["sqlx"]],
    );

    let violations = check_one(&config(), reaching);

    assert_eq!(rules(&violations), [Rule::Transitive, Rule::Transitive]);
    assert!(violations[0].message.contains("reaches `moka`"));
    assert!(violations[1].message.contains("reaches `sqlx`"));
}

#[test]
fn reaching_only_allowed_crates_or_a_crate_of_a_layer_without_a_list_is_fine() {
    let allowed = reaching(member("shop-application", &[]), &[&["serde"]]);
    let unlisted = reaching(member("service", &[]), &[&["sqlx"]]);

    assert_eq!(check_one(&config(), allowed), []);
    assert_eq!(check_one(&config(), unlisted), []);
}

#[test]
fn what_an_uncovered_crate_reaches_is_not_judged() {
    let stray = reaching(member("stray", &[]), &[&["sqlx"]]);

    assert_eq!(rules(&check_one(&config(), stray)), [Rule::Uncovered]);
}

#[test]
fn a_crate_at_crates_context_layer_or_with_a_group_is_in_place() {
    let flat = placed(member("shop-domain", &[]), "crates/shop/domain/Cargo.toml");
    let grouped = placed(
        member("shop-domain", &[]),
        "crates/internal/shop/domain/Cargo.toml",
    );

    assert_eq!(check_one(&config(), flat), []);
    assert_eq!(check_one(&config(), grouped), []);
}

#[test]
fn a_crate_outside_the_path_pattern_is_misplaced() {
    for manifest in [
        "crates/shop/core/Cargo.toml",
        "crates/shop-domain/Cargo.toml",
        "crates/other/domain/Cargo.toml",
        "crates/domain/Cargo.toml",
        "crates/a/b/shop/domain/Cargo.toml",
        "src/shop/domain/Cargo.toml",
        "Cargo.toml",
    ] {
        let misplaced = placed(member("shop-domain", &[]), manifest);

        let violations = check_one(&config(), misplaced);

        assert_eq!(rules(&violations), [Rule::Path], "{manifest}");
    }
}

#[test]
fn the_path_diagnostic_names_the_crate_where_it_is_and_where_it_belongs() {
    let misplaced = placed(member("shop-domain", &[]), "crates/shop/core/Cargo.toml");

    let violations = check_one(&config(), misplaced);

    assert_eq!(
        violations,
        [Violation {
            rule: Rule::Path,
            manifest: "crates/shop/core/Cargo.toml".to_owned(),
            message: "`shop-domain` (layer `domain`, context `shop`) is not at \
                      `crates/[<group>/]shop/domain` but at `crates/shop/core`"
                .to_owned(),
            hint: "move the crate to `crates/[<group>/]shop/domain`".to_owned(),
        }]
    );
}

#[test]
fn a_layer_without_a_directory_is_not_checked_for_its_path() {
    let anywhere = placed(member("service", &[]), "somewhere/else/Cargo.toml");

    assert_eq!(check_one(&config(), anywhere), []);
}

#[test]
fn a_layer_with_a_directory_but_without_a_context_accepts_any_name_in_its_place() {
    let mut config = config();
    config.layers.push(Layer {
        directory: Some("tool".to_owned()),
        ..layer("tooling", "xtask", &[], &[])
    });
    let fits = placed(member("xtask", &[]), "crates/any/tool/Cargo.toml");
    let misplaced = placed(member("xtask", &[]), "xtask/Cargo.toml");

    assert_eq!(check_one(&config, fits), []);
    let violations = check_one(&config, misplaced);
    assert_eq!(
        violations[0].message,
        "`xtask` (layer `tooling`) is not at `crates/[<group>/]<name>/tool` but at `xtask`"
    );
    assert_eq!(
        violations[0].hint,
        "move the crate to `crates/[<group>/]<name>/tool`"
    );
}

#[test]
fn a_local_crate_outside_the_members_is_uncovered_whatever_its_name() {
    let workspace = Workspace {
        crates: vec![member("shop-domain", &[])],
        outside: vec![Outside {
            name: "shop-application".to_owned(),
            manifest: "crates/hidden/Cargo.toml".to_owned(),
        }],
    };

    assert_eq!(
        check(&config(), &workspace),
        [Violation {
            rule: Rule::Uncovered,
            manifest: "crates/hidden/Cargo.toml".to_owned(),
            message: "`shop-application` is not assigned to any layer: it is a local crate that is not a member of the workspace".to_owned(),
            hint: "add the crate to `members` of the workspace and name it `<context>-<layer>`, or add its name to the `crates` of a [[layer]]".to_owned(),
        }]
    );
}
