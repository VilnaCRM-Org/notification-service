use axum::http::StatusCode;
use rstest::rstest;
use serde_json::{Value, json};
use shared_application::http::Problem;

use super::*;
use crate::http::origin::ORIGIN_NOT_ALLOWED;

fn json_document() -> Value {
    serde_json::to_value(document()).unwrap()
}

#[rstest]
#[case(
    "/api/health",
    "get",
    "api_health_get",
    "HealthCheck",
    ("Checks the health of the service.", "Returns 204 when the cache and the message broker respond and the event stream of the node is ready. Returns a 500 problem document that lists every failing dependency otherwise.", "204 500 default")
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
fn documents_exactly_the_five_operations_of_the_rest_router() {
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
            "get /api/event-types",
            "get /api/events/status",
            "get /api/events/stream",
            "get /api/events/ws",
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
            { "name": "EventStream", "description": "Event catalog and event stream status endpoints." },
            { "name": "HealthCheck", "description": "Runtime health-check endpoints." },
        ])
    );
    assert!(json["components"].get("securitySchemes").is_none());
    assert!(json.get("security").is_none());
}

#[test]
fn defines_exactly_the_six_schemas() {
    let json = json_document();
    let schemas = json["components"]["schemas"].as_object().unwrap();

    let names: Vec<&str> = schemas.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        [
            "DependencyFailure",
            "EventStreamStatus",
            "EventType",
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
