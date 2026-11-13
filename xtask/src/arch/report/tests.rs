//! Unit tests of the parent module.

use super::super::config::{Config, Layer};
use super::super::model::{Crate, Workspace};
use super::super::rules::{Rule, Violation};
use super::{Summary, render};

fn rendered(violations: &[Violation], github: bool) -> String {
    let layer = || Layer {
        name: "domain".to_owned(),
        ..Layer::default()
    };
    let member = |name: &str| Crate {
        name: name.to_owned(),
        manifest: String::new(),
        inherits_lints: true,
        dependencies: Vec::new(),
        reach: Vec::new(),
    };
    let config = Config {
        layers: vec![layer(), layer()],
        ..Config::default()
    };
    let workspace = Workspace {
        crates: vec![member("a"), member("b"), member("c")],
        ..Workspace::default()
    };
    render(&Summary::new(&config, &workspace, violations), github)
}

fn violation(rule: Rule, crate_name: &str) -> Violation {
    Violation {
        rule,
        manifest: format!("crates/{crate_name}/Cargo.toml"),
        message: format!("`{crate_name}` must not depend on `b`"),
        hint: "invert it".to_owned(),
    }
}

#[test]
fn a_clean_workspace_gets_a_one_line_verdict() {
    assert_eq!(
        rendered(&[], false),
        "architecture: ok. All 3 crate(s) are assigned to exactly one layer (2 layer(s)); \
         0 violation(s)"
    );
}

#[test]
fn each_violation_is_a_compiler_style_block() {
    assert_eq!(
        rendered(&[violation(Rule::Layer, "a")], false),
        "error[arch::layer]: `a` must not depend on `b`\n  --> crates/a/Cargo.toml\n   = help: invert it\n\n\
         architecture: FAILED. 1 violation(s) in 3 crate(s), 2 layer(s) checked"
    );
}

#[test]
fn blocks_are_separated_by_blank_lines_in_the_order_given() {
    let text = rendered(
        &[violation(Rule::Layer, "a"), violation(Rule::Lints, "b")],
        false,
    );

    assert_eq!(
        text,
        "error[arch::layer]: `a` must not depend on `b`\n  --> crates/a/Cargo.toml\n   = help: invert it\n\n\
         error[arch::lints]: `b` must not depend on `b`\n  --> crates/b/Cargo.toml\n   = help: invert it\n\n\
         architecture: FAILED. 2 violation(s) in 3 crate(s), 2 layer(s) checked"
    );
}

#[test]
fn github_mode_adds_one_annotation_per_violation() {
    let text = rendered(
        &[violation(Rule::Layer, "a"), violation(Rule::Uncovered, "b")],
        true,
    );

    assert_eq!(
        text,
        "error[arch::layer]: `a` must not depend on `b`\n  --> crates/a/Cargo.toml\n   = help: invert it\n\
         ::error file=crates/a/Cargo.toml,title=Architecture (layer)::`a` must not depend on `b`\n\n\
         error[arch::uncovered]: `b` must not depend on `b`\n  --> crates/b/Cargo.toml\n   = help: invert it\n\
         ::error file=crates/b/Cargo.toml,title=Architecture (uncovered)::`b` must not depend on `b`\n\n\
         architecture: FAILED. 2 violation(s) in 3 crate(s), 2 layer(s) checked"
    );
}

#[test]
fn github_mode_adds_nothing_to_a_clean_report() {
    assert_eq!(rendered(&[], true), rendered(&[], false));
}

fn annotation_of(manifest: &str, message: &str) -> String {
    let text = rendered(
        &[Violation {
            rule: Rule::Layer,
            manifest: manifest.to_owned(),
            message: message.to_owned(),
            hint: "invert it".to_owned(),
        }],
        true,
    );
    text.lines()
        .find(|line| line.starts_with("::error "))
        .unwrap()
        .to_owned()
}

#[test]
fn a_message_with_a_percent_sign_and_line_breaks_is_escaped_in_the_annotation() {
    let text = rendered(
        &[Violation {
            rule: Rule::Layer,
            manifest: "crates/a/Cargo.toml".to_owned(),
            message: "100% of\r\nthe: lines, here\nand there".to_owned(),
            hint: "invert it".to_owned(),
        }],
        true,
    );

    assert!(
        text.contains(
            "\n::error file=crates/a/Cargo.toml,title=Architecture (layer)::\
             100%25 of%0D%0Athe: lines, here%0Aand there\n"
        ),
        "{text}"
    );
}

#[test]
fn the_plain_report_does_not_escape_anything() {
    let text = rendered(
        &[Violation {
            rule: Rule::Layer,
            manifest: "dir:one,two/Cargo.toml".to_owned(),
            message: "100% of\nthe lines".to_owned(),
            hint: "invert it".to_owned(),
        }],
        false,
    );

    assert!(
        text.starts_with("error[arch::layer]: 100% of\nthe lines\n  --> dir:one,two/Cargo.toml\n")
    );
    assert!(!text.contains("%25") && !text.contains("%0A"));
}

#[test]
fn a_property_value_with_a_comma_a_colon_a_percent_sign_or_a_line_break_is_escaped() {
    assert_eq!(
        annotation_of("dir:one,two%\r\n/Cargo.toml", "m"),
        "::error file=dir%3Aone%2Ctwo%25%0D%0A/Cargo.toml,title=Architecture (layer)::m"
    );
}

#[test]
fn a_message_keeps_its_colons_and_commas() {
    assert_eq!(
        annotation_of("Cargo.toml", "a: b, c"),
        "::error file=Cargo.toml,title=Architecture (layer)::a: b, c"
    );
}
