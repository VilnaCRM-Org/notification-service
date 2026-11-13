//! Unit tests of the parent module.

use super::{Layer, Rendered, render};

#[test]
fn a_layer_names_its_directory() {
    assert_eq!(Layer::Domain.directory(), "domain");
    assert_eq!(Layer::Application.directory(), "application");
    assert_eq!(Layer::Infrastructure.directory(), "infrastructure");
}

#[test]
fn every_layer_is_listed_once_with_the_domain_first() {
    let directories: Vec<&str> = Layer::ALL.iter().map(|layer| layer.directory()).collect();

    assert_eq!(directories, ["domain", "application", "infrastructure"]);
}

fn manifest(layer: &str, description: &str) -> String {
    format!(
        "[package]\n\
         name = \"invoice-ledger-{layer}\"\n\
         description = \"Bounded context invoice-ledger, {layer} layer: {description}\"\n\
         version.workspace = true\n\
         edition.workspace = true\n\
         rust-version.workspace = true\n\
         license.workspace = true\n\
         publish.workspace = true\n\
         repository.workspace = true\n\
         \n\
         [lints]\n\
         workspace = true\n"
    )
}

#[test]
fn the_domain_manifest_inherits_the_package_fields_and_the_lints_and_has_no_dependency() {
    let rendered = render(Layer::Domain, "invoice-ledger");

    assert_eq!(
        rendered.manifest,
        manifest(
            "domain",
            "aggregates, value objects, domain events and the ports of the context"
        )
    );
}

#[test]
fn the_application_manifest_is_the_same_shape_with_its_own_name_and_description() {
    let rendered = render(Layer::Application, "invoice-ledger");

    assert_eq!(
        rendered.manifest,
        manifest(
            "application",
            "commands, queries, event subscribers and the API-facing adapters"
        )
    );
}

#[test]
fn the_infrastructure_manifest_is_the_same_shape_with_its_own_name_and_description() {
    let rendered = render(Layer::Infrastructure, "invoice-ledger");

    assert_eq!(
        rendered.manifest,
        manifest(
            "infrastructure",
            "adapters for the ports of the domain layer"
        )
    );
}

#[test]
fn the_library_holds_only_crate_documentation_for_its_layer() {
    for (layer, start) in [
        (Layer::Domain, "domain layer: aggregates, value objects"),
        (Layer::Application, "application layer: commands, queries"),
        (Layer::Infrastructure, "infrastructure layer: the adapters"),
    ] {
        let Rendered { lib, manifest } = render(layer, "invoice-ledger");

        assert!(
            lib.starts_with(&format!("//! Bounded context `invoice-ledger`, {start}")),
            "{lib}"
        );
        assert!(lib.ends_with(".\n"), "{lib}");
        assert!(lib.lines().all(|line| line.starts_with("//!")), "{lib}");
        assert!(!lib.contains("{{"), "{lib}");
        assert!(!manifest.contains("{{"), "{manifest}");
    }
}

#[test]
fn every_occurrence_of_the_context_name_is_replaced() {
    let rendered = render(Layer::Application, "x");

    assert_eq!(rendered.manifest.matches("\"x-application\"").count(), 1);
    assert!(rendered.manifest.contains("Bounded context x, application"));
    assert!(rendered.lib.contains("`x`"));
}

#[test]
fn the_layers_state_their_own_dependency_direction() {
    let domain = render(Layer::Domain, "c").lib;
    let application = render(Layer::Application, "c").lib;
    let infrastructure = render(Layer::Infrastructure, "c").lib;

    assert!(domain.contains("framework-free"), "{domain}");
    assert!(
        application.contains("never on the\n//! infrastructure layer"),
        "{application}"
    );
    assert!(
        infrastructure.contains("never on the\n//! application layer"),
        "{infrastructure}"
    );
}
