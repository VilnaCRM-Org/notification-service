//! Unit tests of the parent module.

use toml::Table;

use super::{Config, Layer, UnknownKey, capture, check_keys, pattern_matches};

fn layer(name: &str, crates: &[&str]) -> Layer {
    Layer {
        name: name.to_owned(),
        crates: crates.iter().map(|&pattern| pattern.to_owned()).collect(),
        ..Layer::default()
    }
}

fn parsed(text: &str) -> Table {
    text.parse().unwrap()
}

fn keys(text: &str) -> Result<(), UnknownKey> {
    check_keys(&parsed(text))
}

fn unknown(table: &'static str, key: &str) -> Result<(), UnknownKey> {
    Err(UnknownKey {
        table,
        key: key.to_owned(),
    })
}

#[test]
fn a_pattern_without_a_star_must_be_equal() {
    assert!(pattern_matches("service", "service"));
    assert!(!pattern_matches("service", "service-api"));
    assert!(!pattern_matches("service", "my-service"));
}

#[test]
fn a_star_matches_any_run_of_characters() {
    assert!(pattern_matches("*-domain", "health-check-domain"));
    assert!(pattern_matches("shared-*", "shared-domain"));
    assert!(pattern_matches("a*a", "aa"));
    assert!(pattern_matches("*", "anything"));
    assert!(!pattern_matches("*-domain", "domain"));
    assert!(!pattern_matches("a*a", "a"));
    assert!(!pattern_matches("shared-*", "unshared-domain"));
    assert!(!pattern_matches("*-domain", "health-check-domain-events"));
}

#[test]
fn capture_returns_the_text_the_star_matched() {
    assert_eq!(
        capture("*-domain", "some-module-domain"),
        Some("some-module")
    );
    assert_eq!(capture("shared-*", "shared-kernel"), Some("kernel"));
    assert_eq!(capture("a*b", "a-b"), Some("-"));
    assert_eq!(capture("*", "all"), Some("all"));
    assert_eq!(capture("a*b", "ab"), Some(""));
}

#[test]
fn capture_is_none_without_a_star_or_without_a_match() {
    assert_eq!(capture("service", "service"), None);
    assert_eq!(capture("*-domain", "some-module-application"), None);
    assert_eq!(capture("a*a", "a"), None);
}

#[test]
fn a_layer_covers_a_crate_when_one_pattern_matches() {
    let layer = layer("tooling", &["xtask", "*-tool"]);

    assert!(layer.covers("xtask"));
    assert!(layer.covers("fmt-tool"));
    assert!(!layer.covers("service"));
    assert!(!layer.covers("xtask-extra"));
}

#[test]
fn layers_of_lists_every_layer_that_covers_the_crate() {
    let config = Config {
        layers: vec![
            layer("domain", &["*-domain"]),
            layer("shop", &["shop-*"]),
            layer("service", &["service"]),
        ],
        ..Config::default()
    };
    let names = |crate_name: &str| -> Vec<&str> {
        config
            .layers_of(crate_name)
            .into_iter()
            .map(|layer| layer.name.as_str())
            .collect()
    };

    assert_eq!(names("shop-domain"), ["domain", "shop"]);
    assert_eq!(names("service"), ["service"]);
    assert_eq!(names("stray"), Vec::<&str>::new());
}

#[test]
fn the_context_is_the_text_the_star_matched() {
    let layer = layer("domain", &["*-domain"]);

    assert_eq!(layer.context_of("some-module-domain"), Some("some-module"));
    assert_eq!(layer.context_of("shared-domain"), Some("shared"));
}

#[test]
fn a_crate_without_a_star_match_has_no_context() {
    assert_eq!(layer("root", &["service"]).context_of("service"), None);
    assert_eq!(layer("domain", &["*-domain"]).context_of("-domain"), None);
    assert_eq!(
        layer("domain", &["*-domain"]).context_of("shop-application"),
        None
    );
}

#[test]
fn the_context_comes_from_the_first_pattern_that_matches() {
    let layer = layer("domain", &["service", "*-domain", "domain-*"]);

    assert_eq!(layer.context_of("service"), None);
    assert_eq!(layer.context_of("shop-domain"), Some("shop"));
    assert_eq!(layer.context_of("domain-shop"), Some("shop"));
}

#[test]
fn the_schema_reads_layers_with_optional_lists() {
    let config: Config = toml::from_str(
        "[[layer]]\nname = 'a'\ncrates = ['a-*']\ndepends_on = ['b']\ndev_depends_on = ['c']\n\
         [[layer]]\nname = 'b'\ncrates = ['b']\n",
    )
    .unwrap();

    assert_eq!(config.layers.len(), 2);
    assert_eq!(config.layers[0].name, "a");
    assert_eq!(config.layers[0].crates, ["a-*"]);
    assert_eq!(config.layers[0].depends_on, ["b"]);
    assert_eq!(config.layers[0].dev_depends_on, ["c"]);
    assert_eq!(config.layers[1].depends_on, Vec::<String>::new());
    assert_eq!(config.layers[1].dev_depends_on, Vec::<String>::new());
}

#[test]
fn the_schema_reads_the_allow_lists_the_directory_and_the_shared_contexts() {
    let config: Config = toml::from_str(
        "[contexts]\nshared = ['shared', 'kernel']\n\
         [[layer]]\nname = 'a'\ncrates = ['a-*']\ndirectory = 'a'\nexternal = ['x']\n\
         dev_external = ['y']\nforbidden_transitive = ['z']\n\
         [[layer]]\nname = 'b'\ncrates = ['b']\n",
    )
    .unwrap();

    assert_eq!(config.contexts.shared, ["shared", "kernel"]);
    let (a, b) = (&config.layers[0], &config.layers[1]);
    assert_eq!(a.directory.as_deref(), Some("a"));
    assert_eq!(a.external, ["x"]);
    assert_eq!(a.dev_external, ["y"]);
    assert_eq!(a.forbidden_transitive, ["z"]);
    assert_eq!(b.directory, None);
    assert_eq!(b.external, Vec::<String>::new());
    assert_eq!(b.dev_external, Vec::<String>::new());
    assert_eq!(b.forbidden_transitive, Vec::<String>::new());
}

#[test]
fn a_model_without_a_contexts_table_has_no_shared_context() {
    let config: Config = toml::from_str("[contexts]\n").unwrap();

    assert_eq!(config.contexts.shared, Vec::<String>::new());
    assert!(!toml::from_str::<Config>("").unwrap().is_shared("shared"));
}

#[test]
fn a_context_is_shared_when_it_is_listed_by_exact_name() {
    let config: Config = toml::from_str("[contexts]\nshared = ['shared']\n").unwrap();

    assert!(config.is_shared("shared"));
    assert!(!config.is_shared("shop"));
    assert!(!config.is_shared("shared-kernel"));
}

fn lists(external: &[&str], dev_external: &[&str], forbidden: &[&str]) -> Layer {
    let owned = |items: &[&str]| items.iter().map(|&item| item.to_owned()).collect();
    Layer {
        external: owned(external),
        dev_external: owned(dev_external),
        forbidden_transitive: owned(forbidden),
        ..layer("domain", &["*-domain"])
    }
}

#[test]
fn a_third_party_crate_is_allowed_by_the_external_list_and_by_the_dev_list_as_a_dev_dependency() {
    let layer = lists(&["serde", "aws-*"], &["rstest"], &[]);

    assert!(layer.allows_external("serde", false));
    assert!(layer.allows_external("serde", true));
    assert!(layer.allows_external("aws-config", false));
    assert!(layer.allows_external("rstest", true));
    assert!(!layer.allows_external("rstest", false));
    assert!(!layer.allows_external("axum", false));
    assert!(!layer.allows_external("axum", true));
}

#[test]
fn a_layer_without_lists_allows_no_third_party_crate_and_a_star_allows_all() {
    assert!(!lists(&[], &[], &[]).allows_external("serde", true));
    assert!(lists(&["*"], &[], &[]).allows_external("anything", false));
}

#[test]
fn reaching_a_crate_is_forbidden_when_a_pattern_of_the_list_matches() {
    let layer = lists(&["sqlx"], &["sqlx"], &["sqlx", "hyper*"]);

    assert!(layer.forbids_reaching("sqlx"));
    assert!(layer.forbids_reaching("hyper-util"));
    assert!(!layer.forbids_reaching("serde"));
    assert!(!lists(&[], &[], &[]).forbids_reaching("sqlx"));
}

#[test]
fn a_file_without_layers_is_an_empty_model() {
    let config: Config = toml::from_str("").unwrap();

    assert!(config.layers.is_empty());
}

#[test]
fn a_layer_needs_a_name_and_crates() {
    assert!(toml::from_str::<Config>("[[layer]]\ncrates = []\n").is_err());
    assert!(toml::from_str::<Config>("[[layer]]\nname = 'a'\n").is_err());
}

#[test]
fn every_key_of_the_schema_is_known() {
    let text = "[contexts]\nshared = ['shared']\n[[layer]]\nname = 'a'\ncrates = ['a']\n\
                directory = 'a'\ndepends_on = []\ndev_depends_on = []\nexternal = []\n\
                dev_external = []\nforbidden_transitive = []\n";

    assert_eq!(keys(text), Ok(()));
    assert_eq!(keys(""), Ok(()));
}

#[test]
fn an_unknown_top_level_key_is_rejected() {
    assert_eq!(keys("layers = []\n"), unknown("the top level", "layers"));
}

#[test]
fn an_unknown_key_of_the_contexts_table_is_rejected() {
    assert_eq!(
        keys("[contexts]\nshared = []\nother = []\n"),
        unknown("[contexts]", "other")
    );
}

#[test]
fn an_unknown_key_of_any_layer_is_rejected() {
    let text = "[[layer]]\nname = 'a'\ncrates = []\n[[layer]]\nname = 'b'\ncrates = []\ntypo = 1\n";

    assert_eq!(keys(text), unknown("[[layer]]", "typo"));
}

#[test]
fn keys_of_a_table_that_has_the_wrong_shape_are_left_to_the_schema() {
    assert_eq!(keys("contexts = 1\nlayer = 2\n"), Ok(()));
    assert_eq!(keys("layer = [1, 2]\n"), Ok(()));
}
