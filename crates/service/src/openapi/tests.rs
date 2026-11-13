use rstest::rstest;
use serde_json::{Value, json};

use super::*;

fn json_document() -> Value {
    serde_json::to_value(document()).unwrap()
}

#[rstest]
#[case(
    "/api/health",
    "get",
    "api_health_get",
    "HealthCheck",
    ("Checks the health of the service.", "Returns 204 when the database, the cache and the message broker respond. Returns a 500 problem document that lists every failing dependency otherwise.", "204 500 default")
)]
#[case(
    "/api/example_api_resources",
    "get",
    "api_example_api_resources_get",
    "ExampleApiResource",
    ("Retrieves the collection of ExampleApiResource resources.", "Returns one page of resources, newest first.", "200 400 422 default")
)]
#[case(
    "/api/example_api_resources",
    "post",
    "api_example_api_resources_post",
    "ExampleApiResource",
    ("Creates an ExampleApiResource resource.", "Validates the input, stores the resource and returns it.", "201 400 413 415 422 default")
)]
#[case(
    "/api/example_api_resources/{id}",
    "get",
    "api_example_api_resources_id_get",
    "ExampleApiResource",
    ("Retrieves an ExampleApiResource resource.", "Returns the resource identified by id.", "200 400 404 default")
)]
fn documents_every_operation(
    #[case] path: &str,
    #[case] method: &str,
    #[case] operation_id: &str,
    #[case] tag: &str,
    #[case] texts: (&str, &str, &str),
) {
    let (summary, description, statuses) = texts;
    let json = json_document();
    let operation = &json["paths"][path][method];

    assert_eq!(operation["operationId"], operation_id);
    assert_eq!(operation["tags"], json!([tag]));
    assert_eq!(operation["summary"], summary);
    assert_eq!(operation["description"], description);
    let mut documented: Vec<&str> = operation["responses"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    documented.sort_unstable();
    assert_eq!(documented, statuses.split(' ').collect::<Vec<_>>());
    assert_eq!(
        operation["responses"]["default"]["description"],
        "Problem response."
    );
    assert_eq!(
        operation["responses"]["default"]["content"]["application/problem+json"]["schema"]["$ref"],
        "#/components/schemas/Problem"
    );
}

#[test]
fn documents_exactly_the_four_operations() {
    let json = json_document();
    let mut operations = Vec::new();
    for (path, item) in json["paths"].as_object().unwrap() {
        for method in item.as_object().unwrap().keys() {
            operations.push(format!("{method} {path}"));
        }
    }

    assert_eq!(
        operations,
        [
            "get /api/example_api_resources",
            "post /api/example_api_resources",
            "get /api/example_api_resources/{id}",
            "get /api/health",
        ]
    );
}

#[test]
fn leaves_the_liveness_route_and_the_docs_routes_out() {
    let json = json_document();
    let paths = json["paths"].as_object().unwrap();

    assert!(!paths.contains_key("/ping"));
    assert!(paths.keys().all(|path| !path.starts_with("/api/docs")));
    assert!(!paths.keys().any(|path| path.contains("graphql")));
}

#[test]
fn carries_the_service_identity_and_the_contract_version() {
    let info = &json_document()["info"];

    assert_eq!(json_document()["openapi"], "3.1.0");
    assert_eq!(
        info["title"],
        concat!(env!("CARGO_PKG_DESCRIPTION"), " API")
    );
    assert_eq!(
        info["description"],
        format!(
            "This API provides endpoints to manage {} data within VilnaCRM.",
            env!("CARGO_PKG_DESCRIPTION")
        )
    );
    assert_eq!(info["version"], "1.0.0");
    assert_eq!(info["version"], API_VERSION);
    assert_eq!(
        info["contact"],
        json!({
            "name": "VilnaCRM Support",
            "url": env!("CARGO_PKG_REPOSITORY"),
            "email": "support@vilnacrm.com",
        })
    );
    assert_eq!(
        info["license"],
        json!({
            "name": "CC0-1.0",
            "url": "https://creativecommons.org/publicdomain/zero/1.0/",
        })
    );
}

#[test]
fn the_identity_comes_from_the_package_manifest_and_is_never_empty() {
    assert_ne!(env!("CARGO_PKG_DESCRIPTION"), "");
    assert!(env!("CARGO_PKG_REPOSITORY").starts_with("https://"));
}

#[test]
fn carries_the_server_the_tags_and_no_security_scheme() {
    let json = json_document();

    assert_eq!(
        json["servers"],
        json!([{ "url": "/", "description": "Current host" }])
    );
    assert_eq!(
        json["tags"],
        json!([
            { "name": "ExampleApiResource", "description": "Template example resource endpoints." },
            { "name": "HealthCheck", "description": "Runtime health-check endpoints." },
        ])
    );
    assert!(json["components"].get("securitySchemes").is_none());
    assert!(json.get("security").is_none());
}

#[test]
fn defines_exactly_the_five_schemas() {
    let json = json_document();
    let schemas = json["components"]["schemas"].as_object().unwrap();

    let names: Vec<&str> = schemas.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "CreateExampleApiResourceRequest",
            "DependencyFailure",
            "ExampleApiResource",
            "Problem",
            "Violation"
        ]
    );
    assert_eq!(
        schemas["CreateExampleApiResourceRequest"]["additionalProperties"],
        false
    );
    assert_eq!(
        schemas["ExampleApiResource"]["properties"]["id"]["format"],
        "uuid"
    );
}

#[test]
fn the_create_response_documents_the_location_header() {
    let json = json_document();

    let created = &json["paths"]["/api/example_api_resources"]["post"]["responses"]["201"];
    assert!(created["headers"]["location"].is_object());
}

#[test]
fn the_yaml_is_the_document_in_yaml() {
    let text = yaml().unwrap();

    assert!(text.starts_with("openapi: 3.1.0\n"));
    assert!(text.contains("operationId: api_health_get\n"));
    assert!(text.ends_with('\n'));
}

#[test]
fn twenty_generations_are_byte_identical() {
    let first = yaml().unwrap();

    for _ in 0..20 {
        assert_eq!(yaml().unwrap(), first);
    }
}

#[test]
fn an_export_error_names_the_document_and_keeps_its_cause() {
    let cause = std::io::Error::other("disk on fire");

    let error = ExportError::new(cause);

    assert_eq!(
        error.to_string(),
        "the OpenAPI document could not be serialised"
    );
    let source = std::error::Error::source(&error).unwrap();
    assert_eq!(source.to_string(), "disk on fire");
}
