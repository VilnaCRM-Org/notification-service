use std::path::{Path, PathBuf};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::header::{ACCEPT, CONTENT_TYPE};
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde_json::{Value, json};
use shared_application::http::Problem;
use shared_application::http::fallback::panic_response;
use test_support::static_topic::StaticTopic;
use tower::ServiceExt as _;

use super::*;
use crate::bootstrap::router;
use crate::bootstrap::tests::wiring::adapters_on;
use crate::cli::ServeArgs;
use crate::config::Config;
use crate::graphql::handler::{GET_TEXT, POST_TEXT};
use crate::http::docs::ASSET_TEXT;
use crate::http::origin::ORIGIN_NOT_ALLOWED;

/// The committed snapshot of the document, written by `make generate-openapi-spec`.
const SNAPSHOT: &str = include_str!("../../../../.github/openapi-spec/spec.yaml");

/// The bound of every request of these tests.
const LIMIT: Duration = Duration::from_secs(5);

/// The eleven operations of FR-26: method, path, operation id and tag, in the order of the
/// document (paths sorted, then methods).
const OPERATIONS: [(&str, &str, &str, &str); 11] = [
    ("get", "/api/docs", "api_docs_get", "Documentation"),
    (
        "get",
        "/api/docs/openapi.json",
        "api_docs_openapi_json_get",
        "Documentation",
    ),
    (
        "get",
        "/api/docs/{file}",
        "api_docs_file_get",
        "Documentation",
    ),
    (
        "get",
        "/api/event-types",
        "api_event_types_get",
        "EventStream",
    ),
    (
        "get",
        "/api/events/status",
        "api_events_status_get",
        "EventStream",
    ),
    (
        "get",
        "/api/events/stream",
        "api_events_stream_get",
        "EventStream",
    ),
    ("get", "/api/events/ws", "api_events_ws_get", "EventStream"),
    ("get", "/api/graphql", "api_graphql_get", "GraphQL"),
    ("post", "/api/graphql", "api_graphql_post", "GraphQL"),
    ("get", "/api/health", "api_health_get", "HealthCheck"),
    ("get", "/ping", "ping_get", "HealthCheck"),
];

/// The media types of the bodies that are bytes no inline example can show (FR-26): each is
/// documented with the binary schema of `OpenAPI` 3.1 and without an example.
const BINARY_MEDIA_TYPES: [&str; 1] = ["image/png"];

/// The statuses whose answers never carry a body (RFC 9110): Switching Protocols and No Content.
const WITHOUT_BODY: [&str; 2] = ["101", "204"];

/// The detail of the 404 problem of the router's `not_found` fallback.
const NO_ROUTE: &str = "No route matches the request path.";

/// The calls that would register a route outside the document (FR-27).
const UNDOCUMENTED_REGISTRATIONS: [&str; 3] = [".route(", ".route_service(", ".nest_service("];

/// The source roots the route oracle reads, relative to the workspace: the composition root and
/// the application crates whose routers `api_router()` nests.
const ROUTE_SOURCE_ROOTS: [&str; 3] = [
    "crates/service/src",
    "crates/internal/health-check/application/src",
    "crates/notification/event-stream/application/src",
];

fn json_document() -> Value {
    serde_json::to_value(document()).unwrap()
}

#[rstest]
#[case(
    "/api/health",
    "get",
    "api_health_get",
    "HealthCheck",
    ("Checks the health of the service.", "Returns 204 when the cache responds and the event stream of the node is ready. Returns a 500 problem document that lists every failing dependency otherwise.", "204 500 default")
)]
#[case(
    "/api/event-types",
    "get",
    "api_event_types_get",
    "EventStream",
    ("Retrieves the collection of EventType resources.", "Returns one page of the subscribable event types, in byte order of their names.", "200 400 422 default")
)]
#[case(
    "/api/events/status",
    "get",
    "api_events_status_get",
    "EventStream",
    ("Retrieves the event stream status.", "Returns how far the answering instance has read the event topic, its memory window and its subscribers. It reads only the state the instance holds and answers in every life stage.", "200 default")
)]
#[case(
    "/api/events/stream",
    "get",
    "api_events_stream_get",
    "EventStream",
    ("Opens an event stream of the listed event types.", "Opens a Server-Sent Events stream that delivers every event of the listed types as one frame, in increasing id order: after the cursor of `Last-Event-ID` or `lastEventId` when one is given, live otherwise. The checks run in this order: the query string (400), the types (422), the cursor (400), the node (503); no answer depends on a broker read. docs/streaming-protocol.md describes the protocol.", "200 400 422 503 default")
)]
#[case(
    "/api/events/ws",
    "get",
    "api_events_ws_get",
    "EventStream",
    ("Upgrades to a WebSocket subscription of the listed event types.", "Upgrades the connection to a WebSocket that delivers every event of its types as one text frame, in increasing id order; the client adds and removes types at run time with commands. The checks run in this order: an `Origin` outside `CORS_ALLOWED_ORIGINS` (403), the query string (400), the types (422), the cursor (400), the node (503), then the upgrade (400 or 426). docs/streaming-protocol.md describes the protocol.", "101 400 403 422 426 503 default")
)]
#[case(
    "/api/graphql",
    "post",
    "api_graphql_post",
    "GraphQL",
    ("Executes a GraphQL operation, answered as JSON or as an event stream.", POST_TEXT, "200 400 413 415 422 503 default")
)]
#[case(
    "/api/graphql",
    "get",
    "api_graphql_get",
    "GraphQL",
    ("Runs the GraphQL operation of the query string as an event stream.", GET_TEXT, "200 400 405 414 422 503 default")
)]
#[case(
    "/ping",
    "get",
    "ping_get",
    "HealthCheck",
    ("Checks that the process is alive.", "Answers `pong` without touching any dependency, so it keeps answering while the cache or the event stream is down; the container health check requests it. Readiness is `GET /api/health`.", "200 default")
)]
#[case(
    "/api/docs",
    "get",
    "api_docs_get",
    "Documentation",
    ("Serves the API documentation page.", "Serves the Swagger UI page, which renders `GET /api/docs/openapi.json` and loads its files from `GET /api/docs/{file}`, nothing from a third-party origin. It is answered directly, without a redirect, under its own content security policy.", "200 default")
)]
#[case(
    "/api/docs/openapi.json",
    "get",
    "api_docs_openapi_json_get",
    "Documentation",
    ("Serves the OpenAPI document.", "Serves this document as JSON: every operation the service serves, the one `make generate-openapi-spec` writes to `.github/openapi-spec/spec.yaml`.", "200 default")
)]
#[case(
    "/api/docs/{file}",
    "get",
    "api_docs_file_get",
    "Documentation",
    ("Serves a file of the documentation page.", ASSET_TEXT, "200 400 404 default")
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

/// Every operation of the document as `(method, path, operation)`.
fn operations(json: &Value) -> Vec<(String, String, Value)> {
    let mut operations = Vec::new();
    for (path, item) in json["paths"].as_object().unwrap() {
        for (method, operation) in item.as_object().unwrap() {
            operations.push((method.clone(), path.clone(), operation.clone()));
        }
    }
    operations
}

/// Whether `content`, a media type object, or `schema` carries an example.
fn has_example(content: &Value) -> bool {
    content.get("example").is_some()
        || content
            .get("examples")
            .and_then(Value::as_object)
            .is_some_and(|examples| !examples.is_empty())
        || content["schema"]
            .get("examples")
            .and_then(Value::as_array)
            .is_some_and(|examples| !examples.is_empty())
}

/// The faults of one parameter: a description, a schema and an example; a default is not an
/// example (FR-26).
fn parameter_faults(id: &str, parameter: &Value) -> Vec<String> {
    let name = parameter["name"].as_str().unwrap_or_default();
    let mut faults = Vec::new();
    if parameter["description"].as_str().is_none_or(str::is_empty) {
        faults.push(format!("{id}: parameter {name} has no description"));
    }
    if parameter.get("schema").is_none() {
        faults.push(format!("{id}: parameter {name} has no schema"));
    }
    if !has_example(parameter) {
        faults.push(format!("{id}: parameter {name} has no example"));
    }
    faults
}

/// The binary schema of `OpenAPI` 3.1 for a body of `media_type`: a string of that content type.
fn binary_schema(media_type: &str) -> Value {
    json!({"type": "string", "contentMediaType": media_type})
}

/// The faults of the media type `media_type` of one answer, named `at`: a binary body has the
/// binary schema and no example, any other body a schema and an example.
fn media_type_faults(at: &str, media_type: &str, entry: &Value) -> Vec<String> {
    let mut faults = Vec::new();
    if BINARY_MEDIA_TYPES.contains(&media_type) {
        if entry.get("schema") != Some(&binary_schema(media_type)) {
            faults.push(format!("{at}: not the binary schema"));
        }
        if has_example(entry) {
            faults.push(format!("{at}: an example of bytes"));
        }
        return faults;
    }
    if entry.get("schema").is_none() {
        faults.push(format!("{at}: no schema"));
    }
    if !has_example(entry) {
        faults.push(format!("{at}: no example"));
    }
    faults
}

/// The faults of one answer, `default` included (FR-26): a media type unless the status never has
/// a body, the faults of each media type, and, for a problem answer, `application/problem+json`
/// with the `Problem` schema.
fn response_faults(id: &str, status: &str, response: &Value) -> Vec<String> {
    let mut faults = Vec::new();
    let Some(content) = response.get("content").and_then(Value::as_object) else {
        if !WITHOUT_BODY.contains(&status) {
            faults.push(format!("{id} {status}: no media type"));
        }
        return faults;
    };
    for (media_type, entry) in content {
        faults.extend(media_type_faults(
            &format!("{id} {status} {media_type}"),
            media_type,
            entry,
        ));
    }
    if status.starts_with('4') || status.starts_with('5') || status == "default" {
        let problem = &content
            .get("application/problem+json")
            .map(|entry| &entry["schema"]);
        if *problem != Some(&json!({"$ref": "#/components/schemas/Problem"})) {
            faults.push(format!("{id} {status}: not the problem schema"));
        }
    }
    faults
}

/// Every way in which an operation is not fully documented (FR-26).
fn documentation_faults(operation: &Value) -> Vec<String> {
    let id = operation["operationId"].as_str().unwrap_or_default();
    let mut faults = Vec::new();
    for field in ["summary", "description"] {
        if operation[field].as_str().is_none_or(str::is_empty) {
            faults.push(format!("{id}: no {field}"));
        }
    }
    for parameter in operation["parameters"].as_array().into_iter().flatten() {
        faults.extend(parameter_faults(id, parameter));
    }
    if let Some(body) = operation.get("requestBody") {
        for (media_type, entry) in body["content"].as_object().unwrap() {
            if entry.get("schema").is_none() || !has_example(entry) {
                faults.push(format!(
                    "{id}: request body {media_type} lacks a schema or an example"
                ));
            }
        }
    }
    let responses = operation["responses"].as_object().unwrap();
    if !responses.contains_key("default") {
        faults.push(format!("{id}: no default answer"));
    }
    for (status, response) in responses {
        faults.extend(response_faults(id, status, response));
    }
    faults
}

#[test]
fn documents_exactly_the_eleven_operations() {
    let json = json_document();

    let found: Vec<(String, String, String, Value)> = operations(&json)
        .into_iter()
        .map(|(method, path, operation)| {
            let id = operation["operationId"].as_str().unwrap().to_owned();
            (method, path, id, operation["tags"].clone())
        })
        .collect();
    let expected: Vec<(String, String, String, Value)> = OPERATIONS
        .iter()
        .map(|(method, path, id, tag)| {
            (
                (*method).to_owned(),
                (*path).to_owned(),
                (*id).to_owned(),
                json!([tag]),
            )
        })
        .collect();
    assert_eq!(found, expected);
    let faults: Vec<String> = operations(&json)
        .iter()
        .flat_map(|(_, _, operation)| documentation_faults(operation))
        .collect();
    assert_eq!(faults, Vec::<String>::new());
}

#[tokio::test]
async fn every_default_example_is_the_500_problem_of_the_panic_catcher() {
    let caught = panic_response(Box::new("a handler panicked"));
    let body = caught.into_body().collect().await.unwrap().to_bytes();
    let problem: Value = serde_json::from_slice(&body).unwrap();
    let json = json_document();

    assert_eq!(problem["status"], 500);
    for (method, path, operation) in operations(&json) {
        assert_eq!(
            operation["responses"]["default"]["content"]["application/problem+json"]["example"],
            problem,
            "{method} {path}"
        );
    }
}

#[rstest]
#[case::no_summary("summary", json!(""), "x: no summary")]
#[case::no_description("description", json!(null), "x: no description")]
#[case::no_default("responses", json!({"200": {"description": "ok"}}), "x: no default answer")]
#[case::no_example(
    "responses",
    json!({"200": {"description": "ok", "content": {"text/plain": {"schema": {"type": "string"}}}}, "default": {"description": "d", "content": {"application/problem+json": {"schema": {"$ref": "#/components/schemas/Problem"}}}}}),
    "ping_get 200 text/plain: no example"
)]
#[case::no_schema(
    "responses",
    json!({"200": {"description": "ok", "content": {"text/plain": {"example": "pong"}}}, "default": {"description": "d", "content": {"application/problem+json": {"schema": {"$ref": "#/components/schemas/Problem"}}}}}),
    "ping_get 200 text/plain: no schema"
)]
#[case::not_a_problem(
    "responses",
    json!({"200": {"description": "ok", "content": {"text/plain": {"schema": {"type": "string"}, "example": "pong"}}}, "default": {"description": "d", "content": {"application/json": {"schema": {"type": "object"}}}}}),
    "ping_get default: not the problem schema"
)]
#[case::undescribed_parameter(
    "parameters",
    json!([{"name": "q", "schema": {"type": "string"}, "example": "a"}]),
    "ping_get: parameter q has no description"
)]
#[case::parameter_without_schema(
    "parameters",
    json!([{"name": "q", "description": "d", "example": "a"}]),
    "ping_get: parameter q has no schema"
)]
#[case::parameter_without_example(
    "parameters",
    json!([{"name": "q", "description": "d", "schema": {"type": "string"}}]),
    "ping_get: parameter q has no example"
)]
#[case::body_without_example(
    "requestBody",
    json!({"content": {"application/json": {"schema": {"type": "object"}}}}),
    "ping_get: request body application/json lacks a schema or an example"
)]
#[case::default_without_example(
    "responses",
    json!({"200": {"description": "ok", "content": {"text/plain": {"schema": {"type": "string"}, "example": "pong"}}}, "default": {"description": "d", "content": {"application/problem+json": {"schema": {"$ref": "#/components/schemas/Problem"}}}}}),
    "ping_get default application/problem+json: no example"
)]
#[case::no_media_type(
    "responses",
    json!({"200": {"description": "ok"}, "default": {"description": "d", "content": {"application/problem+json": {"schema": {"$ref": "#/components/schemas/Problem"}, "example": {}}}}}),
    "ping_get 200: no media type"
)]
#[case::binary_without_its_schema(
    "responses",
    json!({"200": {"description": "ok", "content": {"image/png": {}}}, "default": {"description": "d", "content": {"application/problem+json": {"schema": {"$ref": "#/components/schemas/Problem"}, "example": {}}}}}),
    "ping_get 200 image/png: not the binary schema"
)]
#[case::binary_with_an_example(
    "responses",
    json!({"200": {"description": "ok", "content": {"image/png": {"schema": {"type": "string", "contentMediaType": "image/png"}, "example": "PNG"}}}, "default": {"description": "d", "content": {"application/problem+json": {"schema": {"$ref": "#/components/schemas/Problem"}, "example": {}}}}}),
    "ping_get 200 image/png: an example of bytes"
)]
fn the_documentation_check_reports_each_missing_part(
    #[case] field: &str,
    #[case] value: Value,
    #[case] fault: &str,
) {
    let json = json_document();
    let mut operation = json["paths"]["/ping"]["get"].clone();
    if fault.starts_with("x:") {
        operation["operationId"] = json!("x");
    }
    operation[field] = value;

    assert!(
        documentation_faults(&operation).contains(&fault.to_owned()),
        "{:?}",
        documentation_faults(&operation)
    );
}

#[test]
fn a_schema_example_counts_as_the_example_of_a_parameter_and_a_default_does_not() {
    let with_default =
        json!({"name": "page", "description": "d", "schema": {"type": "integer", "default": 1}});
    let with_examples =
        json!({"name": "v", "description": "d", "schema": {"type": "string", "examples": ["{}"]}});

    assert_eq!(
        parameter_faults("x", &with_default),
        ["x: parameter page has no example"]
    );
    assert_eq!(parameter_faults("x", &with_examples), Vec::<String>::new());
}

#[test]
fn a_binary_body_with_its_schema_and_the_answers_without_a_body_are_documented() {
    let binary = json!({"description": "ok", "content": {"image/png": {"schema": {"type": "string", "contentMediaType": "image/png"}}}});
    let upgrade = json!({"description": "Switching Protocols"});
    let empty = json!({"description": "No Content"});

    assert_eq!(response_faults("x", "200", &binary), Vec::<String>::new());
    assert_eq!(response_faults("x", "101", &upgrade), Vec::<String>::new());
    assert_eq!(response_faults("x", "204", &empty), Vec::<String>::new());
}

/// The non-test Rust sources under `root`: every `.rs` file except the `tests.rs` files and the
/// files of a `tests` directory, as paths relative to the workspace.
fn non_test_sources(workspace: &Path, root: &str) -> Vec<(String, String)> {
    let mut pending = vec![workspace.join(root)];
    let mut found = Vec::new();
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "tests") {
                continue;
            }
            pending.extend(
                std::fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && !path.ends_with("tests.rs")
        {
            let relative = path.strip_prefix(workspace).unwrap();
            found.push((
                relative.to_string_lossy().replace('\\', "/"),
                std::fs::read_to_string(&path).unwrap(),
            ));
        }
    }
    found.sort_unstable();
    found
}

/// `file:line` of every line of `source` that registers a route outside the document.
fn undocumented_registrations(file: &str, source: &str) -> Vec<String> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            UNDOCUMENTED_REGISTRATIONS
                .iter()
                .any(|call| line.contains(call))
        })
        .map(|(index, _)| format!("{file}:{}", index.saturating_add(1)))
        .collect()
}

/// The workspace root, two levels above this crate.
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn every_route_is_registered_through_routes() {
    let workspace = workspace();
    let sources: Vec<(String, String)> = ROUTE_SOURCE_ROOTS
        .iter()
        .flat_map(|root| non_test_sources(&workspace, root))
        .collect();

    let names: Vec<&str> = sources.iter().map(|(file, _)| file.as_str()).collect();
    for expected in [
        "crates/service/src/http/router.rs",
        "crates/service/src/openapi.rs",
        "crates/internal/health-check/application/src/http.rs",
        "crates/notification/event-stream/application/src/http.rs",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} is not read: {names:?}"
        );
    }
    assert!(
        names
            .iter()
            .all(|name| !name.ends_with("tests.rs") && !name.contains("/tests/"))
    );
    let found: Vec<String> = sources
        .iter()
        .flat_map(|(file, source)| undocumented_registrations(file, source))
        .collect();
    assert_eq!(found, Vec::<String>::new());
}

#[rstest]
#[case::route(".route(\"/planted\", get(ping))")]
#[case::route_service(".route_service(\"/planted\", service)")]
#[case::nest_service(".nest_service(\"/planted\", service)")]
fn the_route_oracle_names_the_file_and_line_of_a_planted_registration(#[case] planted: &str) {
    let file = "crates/service/src/http/router.rs";
    let source = std::fs::read_to_string(workspace().join(file)).unwrap();
    let marker = "        .merge(operational)\n";
    let line = source
        .lines()
        .position(|line| line == marker.trim_end())
        .unwrap()
        .saturating_add(2);

    let copy = source.replacen(marker, &format!("{marker}        {planted}\n"), 1);

    assert_eq!(
        undocumented_registrations(file, &source),
        Vec::<String>::new()
    );
    assert_eq!(
        undocumented_registrations(file, &copy),
        [format!("{file}:{line}")]
    );
}

#[test]
fn the_route_oracle_skips_the_test_sources() {
    let workspace = workspace();

    let sources = non_test_sources(&workspace, "crates/service/src/http");

    let names: Vec<&str> = sources.iter().map(|(file, _)| file.as_str()).collect();
    assert!(
        names.contains(&"crates/service/src/http/vary.rs"),
        "{names:?}"
    );
    assert!(
        !names.contains(&"crates/service/src/http/vary/tests.rs"),
        "{names:?}"
    );
    assert!(
        std::fs::read_to_string(workspace.join("crates/service/src/http/vary/tests.rs"))
            .unwrap()
            .contains(".route(")
    );
}

/// The configuration of `app_env` with the defaults of every other variable.
fn config(app_env: &str) -> Config {
    let args = ServeArgs {
        app_env: Some(app_env.to_owned()),
        kafka: crate::cli::KafkaArgs {
            bootstrap_servers: Some("localhost:9092".to_owned()),
            ..crate::cli::KafkaArgs::default()
        },
        ..ServeArgs::default()
    };
    Config::from_args(args).unwrap()
}

/// The assembled router of `app_env`, without its background work.
fn assembled(app_env: &str) -> Router {
    let topic = StaticTopic::empty();
    router(
        adapters_on(&topic, Box::new(topic.clone())),
        &config(app_env),
    )
    .unwrap()
}

/// The status and the body of the answer of `app` to `request`, read within [`LIMIT`].
async fn answer(app: &Router, request: Request<Body>) -> (StatusCode, Vec<u8>, String) {
    tokio::time::timeout(LIMIT, async {
        let response = app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .map(|value| value.to_str().unwrap().to_owned())
            .unwrap_or_default();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, body.to_vec(), content_type)
    })
    .await
    .unwrap()
}

/// The path of an operation with a sample value for each path parameter.
fn sample_path(path: &str) -> String {
    path.replace("{file}", "swagger-ui.css")
}

/// Whether an answer is one of the router's own: the 404 problem of the `not_found` fallback or
/// any 405, which only the `method_not_allowed` fallback answers to a request that accepts any
/// type.
fn is_a_fallback(status: StatusCode, body: &[u8]) -> bool {
    let detail = serde_json::from_slice::<Value>(body).unwrap_or(Value::Null)["detail"].clone();
    status == StatusCode::METHOD_NOT_ALLOWED
        || (status == StatusCode::NOT_FOUND && detail == json!(NO_ROUTE))
}

#[tokio::test]
async fn every_operation_is_served() {
    for app_env in ["dev", "test", "prod"] {
        let app = assembled(app_env);
        for (method, path, id, _) in OPERATIONS {
            let request = Request::builder()
                .method(Method::from_bytes(method.to_uppercase().as_bytes()).unwrap())
                .uri(sample_path(path))
                .header(ACCEPT, "*/*")
                .body(Body::empty())
                .unwrap();

            let (status, body, _) = answer(&app, request).await;

            assert!(!is_a_fallback(status, &body), "{app_env} {id}: {status}");
        }
    }
}

#[tokio::test]
async fn the_fallback_check_recognises_both_fallbacks() {
    let app = assembled("test");
    let unknown = Request::get("/api/unknown").body(Body::empty()).unwrap();
    let wrong_method = Request::delete("/ping").body(Body::empty()).unwrap();
    let unknown_asset = Request::get("/api/docs/nope.js")
        .body(Body::empty())
        .unwrap();

    let (status, body, _) = answer(&app, unknown).await;
    assert!(is_a_fallback(status, &body));
    let (status, body, _) = answer(&app, wrong_method).await;
    assert!(is_a_fallback(status, &body));
    let (status, body, _) = answer(&app, unknown_asset).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!is_a_fallback(status, &body));
}

#[tokio::test]
async fn the_served_document_is_the_exported_one() {
    let (app, _) = operational_router().split_for_parts();
    let request = Request::get("/api/docs/openapi.json")
        .body(Body::empty())
        .unwrap();

    let (status, body, content_type) = answer(&app, request).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "application/json");
    let served: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(served, serde_json::to_value(document()).unwrap());
    assert_eq!(yaml().unwrap(), SNAPSHOT);
}

/// The values of every `href` and `src` attribute of `page`.
fn references(page: &str) -> Vec<String> {
    let mut values = Vec::new();
    for attribute in ["href=\"", "src=\""] {
        for part in page.split(attribute).skip(1) {
            values.push(part.split('"').next().unwrap().to_owned());
        }
    }
    values
}

#[tokio::test]
async fn the_page_assets_are_served() {
    let app = assembled("prod");
    let json = json_document();
    let documented = json["paths"]["/api/docs/{file}"]["get"]["responses"]["200"]["content"]
        .as_object()
        .unwrap()
        .clone();
    let page = Request::get("/api/docs").body(Body::empty()).unwrap();
    let (_, page, _) = answer(&app, page).await;
    let assets = references(&String::from_utf8(page).unwrap());
    assert_eq!(assets.len(), 5);

    for asset in assets {
        let request = Request::get(asset.as_str()).body(Body::empty()).unwrap();

        let (status, body, content_type) = answer(&app, request).await;

        assert_eq!(status, StatusCode::OK, "{asset}");
        assert!(
            documented.contains_key(&content_type),
            "{asset}: {content_type}"
        );
        assert!(!body.is_empty(), "{asset}");
    }
    let unknown = Request::get("/api/docs/nope.js")
        .body(Body::empty())
        .unwrap();
    let (status, body, content_type) = answer(&app, unknown).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type, "application/problem+json");
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap(),
        json!({"type": "about:blank", "title": "Not Found", "status": 404})
    );
}

#[tokio::test]
async fn an_asset_path_deeper_than_one_segment_is_the_404_of_the_router() {
    let app = assembled("prod");
    let request = Request::get("/api/docs/a/swagger-ui.css")
        .body(Body::empty())
        .unwrap();

    let (status, body, _) = answer(&app, request).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["detail"],
        NO_ROUTE
    );
}

#[test]
fn the_graphql_router_documents_one_get_and_one_post_operation_as_the_document_does() {
    let graphql = serde_json::to_value(graphql_router().into_openapi()).unwrap();
    let path = &graphql["paths"]["/api/graphql"];

    assert_eq!(
        path.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["get", "post"]
    );
    assert_eq!(path["get"]["operationId"], "api_graphql_get");
    assert_eq!(path["post"]["operationId"], "api_graphql_post");
    assert_eq!(
        *path,
        serde_json::to_value(document()).unwrap()["paths"]["/api/graphql"]
    );
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
    assert_eq!(info["version"], "2.1.0");
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
            { "name": "Documentation", "description": "The API documentation page and its document." },
            { "name": "EventStream", "description": "Event catalog and event stream status endpoints." },
            { "name": "GraphQL", "description": "GraphQL queries and subscriptions over Server-Sent Events." },
            { "name": "HealthCheck", "description": "Runtime health-check endpoints." },
        ])
    );
    assert!(json["components"].get("securitySchemes").is_none());
    assert!(json.get("security").is_none());
}

#[test]
fn defines_exactly_the_nine_schemas() {
    let json = json_document();
    let schemas = json["components"]["schemas"].as_object().unwrap();

    let names: Vec<&str> = schemas.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "DependencyFailure",
            "EventStreamStatus",
            "EventType",
            "GraphQLEvent",
            "GraphQLRequest",
            "GraphQLResponse",
            "Problem",
            "SseFrame",
            "Violation"
        ]
    );
}

#[test]
fn the_event_type_schema_carries_the_grammar_of_the_names() {
    let json = json_document();

    let name = &json["components"]["schemas"]["EventType"]["properties"]["name"];
    assert_eq!(name["pattern"], "^[a-z][a-z0-9]*([._-][a-z0-9]+)*$");
    assert_eq!(name["maxLength"], 128);
    assert_eq!(
        json["paths"]["/api/event-types"]["get"]["responses"]["200"]["content"]["application/json"]
            ["schema"],
        json!({"type": "array", "items": {"$ref": "#/components/schemas/EventType"}})
    );
}

#[test]
fn the_status_schema_carries_the_twelve_fields_and_the_status_route_returns_it() {
    let json = json_document();

    let schema = &json["components"]["schemas"]["EventStreamStatus"];
    let mut fields: Vec<&str> = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "brokerReachable",
            "endOffset",
            "lag",
            "position",
            "reason",
            "rejectedRecords",
            "replayCapped",
            "retentionLosses",
            "state",
            "subscribers",
            "windowFirst",
            "windowLast"
        ]
    );
    assert_eq!(
        json["paths"]["/api/events/status"]["get"]["responses"]["200"]["content"]["application/json"]
            ["schema"],
        json!({"$ref": "#/components/schemas/EventStreamStatus"})
    );
}

#[test]
fn the_streaming_operations_answer_the_frame_schema_and_the_upgrade() {
    let json = json_document();

    let stream = &json["paths"]["/api/events/stream"]["get"]["responses"]["200"]["content"];
    assert_eq!(
        stream["text/event-stream"]["schema"],
        json!({"$ref": "#/components/schemas/SseFrame"})
    );
    let upgraded = &json["paths"]["/api/events/ws"]["get"]["responses"]["101"];
    assert!(upgraded.get("content").is_none());
    let mut headers: Vec<&str> = upgraded["headers"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    headers.sort_unstable();
    assert_eq!(headers, ["Connection", "Sec-WebSocket-Accept", "Upgrade"]);
}

#[test]
fn the_refused_origin_example_is_the_answer_of_the_origin_check() {
    let json = json_document();

    let refused = &json["paths"]["/api/events/ws"]["get"]["responses"]["403"]["content"]["application/problem+json"]
        ["example"];
    assert_eq!(
        *refused,
        serde_json::to_value(Problem::new(StatusCode::FORBIDDEN).with_detail(ORIGIN_NOT_ALLOWED))
            .unwrap()
    );
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
