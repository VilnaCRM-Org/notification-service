use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::header::{ACCEPT, ALLOW, CONTENT_TYPE};
use axum::http::{Method, Request};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use test_support::static_topic::StaticTopic;
use tower::ServiceExt as _;

use super::*;
use crate::bootstrap::router;
use crate::bootstrap::tests::wiring::adapters_on;
use crate::cli::ServeArgs;
use crate::config::{AppEnv, Config};
use crate::graphql::QueryStringLimit;
use crate::graphql::handler::ExplorerPage;
use crate::graphql::schema::builder;
use crate::graphql::single_root;
use crate::openapi::{document, graphql_router};

/// The bound of every request of these tests.
const LIMIT: Duration = Duration::from_secs(5);

/// The body limit of the routers of these tests, in bytes: above the body of the example
/// subscription.
const BODY_LIMIT: usize = 256;

/// The one error of a valid subscription on the JSON transport.
const TRANSPORT_ERROR: &str = "Subscriptions are not supported on this transport.";

/// The `GraphQL` routes, registered as the service registers them, over a schema with the
/// query-string bound and the body limit [`BODY_LIMIT`] and the explorer flag of `app_env`.
fn graphql(app_env: AppEnv) -> Router {
    let schema = builder()
        .data(QueryStringLimit {
            max_bytes: BODY_LIMIT,
        })
        .data(ExplorerPage::of(app_env))
        .finish();
    let (routes, _document) = graphql_router().split_for_parts();
    routes
        .with_state(schema)
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

/// The status, the headers and the body of the answer of `app` to `request` as JSON (`Null` when
/// it is not JSON), read within [`LIMIT`].
async fn answer(app: Router, request: Request<Body>) -> (u16, axum::http::HeaderMap, Value) {
    tokio::time::timeout(LIMIT, async {
        let response = app.oneshot(request).await.unwrap();
        let status = response.status().as_u16();
        let headers = response.headers().clone();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            headers,
            serde_json::from_slice(&body).unwrap_or(Value::Null),
        )
    })
    .await
    .unwrap()
}

/// A POST of `body` as `content_type`.
fn post(content_type: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/api/graphql")
        .header(CONTENT_TYPE, content_type)
        .body(Body::from(body.to_owned()))
        .unwrap()
}

/// A GET of the path with `query_string`, negotiating the event stream.
fn get(query_string: &str) -> Request<Body> {
    Request::get(format!("/api/graphql?{query_string}"))
        .header(ACCEPT, "text/event-stream")
        .body(Body::empty())
        .unwrap()
}

/// The answer of the document to `status` of the operation `method` of the `GraphQL` path.
fn documented(method: &str, status: &str) -> Value {
    serde_json::to_value(document()).unwrap()["paths"]["/api/graphql"][method]["responses"][status]
        .clone()
}

/// The example problem of `status` of the operation `method`.
fn problem_of(method: &str, status: &str) -> Value {
    documented(method, status)["content"]["application/problem+json"]["example"].clone()
}

#[tokio::test]
async fn the_problem_examples_of_the_post_form_are_its_answers() {
    let oversized = format!(r#"{{"query":"{}"}}"#, "a".repeat(BODY_LIMIT));
    let cases = [
        ("400", post("application/json", "x")),
        ("413", post("application/json", &oversized)),
        ("415", post("text/plain", "{}")),
        ("422", post("application/json", r#"{"query":1}"#)),
    ];

    for (status, request) in cases {
        let (answered, _, body) = answer(graphql(AppEnv::Test), request).await;

        assert_eq!(answered.to_string(), status);
        assert_eq!(body, problem_of("post", status), "{status}");
    }
}

#[tokio::test]
async fn the_problem_examples_of_the_get_form_are_its_answers() {
    let cases = [
        ("400", get("operationName=a")),
        ("414", get(&format!("query={}", "a".repeat(BODY_LIMIT)))),
        ("422", get("query=a&variables=1")),
    ];

    for (status, request) in cases {
        let (answered, _, body) = answer(graphql(AppEnv::Test), request).await;

        assert_eq!(answered.to_string(), status);
        assert_eq!(body, problem_of("get", status), "{status}");
    }
}

#[tokio::test]
async fn the_405_example_is_the_answer_to_a_get_that_does_not_negotiate_outside_dev() {
    let request = Request::get("/api/graphql").body(Body::empty()).unwrap();

    let (status, headers, body) = answer(graphql(AppEnv::Prod), request).await;

    assert_eq!(status, 405);
    assert_eq!(headers[ALLOW], "POST");
    assert_eq!(body, problem_of("get", "405"));
    assert!(documented("get", "405")["headers"].get("Allow").is_some());
}

#[test]
fn the_503_example_is_the_refusal_of_a_node_that_is_starting() {
    let example = problem_of("post", "503");

    assert_eq!(example, problem_of("get", "503"));
    assert_eq!(
        example,
        json!({"type": "about:blank", "title": "Service Unavailable", "status": 503, "detail": "the node is starting"})
    );
}

#[tokio::test]
async fn the_example_subscription_is_a_valid_document_of_one_root_field() {
    let body = json!({"query": EXAMPLE_SUBSCRIPTION, "operationName": EXAMPLE_OPERATION});
    let mut request = async_graphql::Request::new(EXAMPLE_SUBSCRIPTION);

    let (status, _, answered) = answer(
        graphql(AppEnv::Test),
        post("application/json", &body.to_string()),
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(
        answered,
        json!({"data": null, "errors": [{"message": TRANSPORT_ERROR}]})
    );
    assert!(single_root::check(&mut request).is_ok());
    assert!(EXAMPLE_SUBSCRIPTION.contains(&format!("subscription {EXAMPLE_OPERATION} ")));
    assert!(EXAMPLE_SUBSCRIPTION.contains(&format!(r#"types: ["{EXAMPLE_TYPE}"]"#)));
}

#[tokio::test]
async fn the_example_query_result_is_the_answer_of_a_catalog_of_the_example_type() {
    let topic = StaticTopic::empty();
    let args = ServeArgs {
        app_env: Some("test".to_owned()),
        kafka: crate::cli::KafkaArgs {
            bootstrap_servers: Some("localhost:9092".to_owned()),
            ..crate::cli::KafkaArgs::default()
        },
        event_types: Some(EXAMPLE_TYPE.to_owned()),
        ..ServeArgs::default()
    };
    let app = router(
        adapters_on(&topic, Box::new(topic.clone())),
        &Config::from_args(args).unwrap(),
    )
    .unwrap();
    let body = json!({"query": EXAMPLE_QUERY});

    let (status, _, answered) = answer(app, post("application/json", &body.to_string())).await;

    assert_eq!(status, 200);
    assert_eq!(answered, query_result_example());
    assert_eq!(
        documented("post", "200")["content"]["application/json"]["example"],
        answered
    );
}

#[test]
fn the_example_blocks_are_a_subscription_result_and_the_end_of_a_single_result() {
    let next = next_event_example();
    let result: Value = serde_json::from_str(next["data"].as_str().unwrap()).unwrap();

    assert_eq!(next["event"], "next");
    assert_eq!(next["id"], EXAMPLE_ID);
    assert_eq!(result["data"]["events"]["id"], EXAMPLE_ID);
    assert_eq!(result["data"]["events"]["type"], EXAMPLE_TYPE);
    assert_eq!(result["data"]["events"]["data"]["type"], EXAMPLE_TYPE);
    assert_eq!(
        complete_event_example(),
        json!({"event": "complete", "data": ""})
    );
}

#[test]
fn the_event_schema_admits_the_empty_data_of_complete_before_the_json_of_a_result() {
    let schema = serde_json::to_value(GraphQLEvent::schema()).unwrap();

    assert_eq!(
        schema,
        json!({
            "type": "object",
            "description": EVENT_TEXT,
            "required": ["event", "data"],
            "properties": {
                "data": {
                    "description": "The execution result as one line of compact JSON for `next`; empty for `complete`.",
                    "anyOf": [
                        {"type": "string", "maxLength": 0},
                        {
                            "type": "string",
                            "minLength": 1,
                            "contentMediaType": "application/json",
                            "contentSchema": {"$ref": "#/components/schemas/GraphQLResponse"}
                        }
                    ]
                },
                "event": {
                    "type": "string",
                    "description": "`next` for an execution result, `complete` after a single result.",
                    "enum": ["next", "complete"]
                },
                "id": {
                    "type": "string",
                    "description": "The event id of a subscription result in decimal, the `id` of the event; a single result and `complete` have none. An `EventSource` sends the last one back as `Last-Event-ID`.",
                    "examples": [EXAMPLE_ID],
                    "maxLength": 20,
                    "minLength": 1,
                    "pattern": "^[1-9][0-9]*$"
                }
            }
        })
    );
}

#[test]
fn the_request_schema_requires_a_string_query_and_accepts_other_members() {
    let schema = serde_json::to_value(GraphQLRequest::schema()).unwrap();

    assert_eq!(schema["required"], json!(["query"]));
    assert_eq!(schema["properties"]["query"]["type"], "string");
    assert_eq!(schema["properties"]["query"]["maxLength"], MAX_QUERY_CHARS);
    assert_eq!(
        schema["properties"]["operationName"]["type"],
        json!(["string", "null"])
    );
    assert_eq!(
        schema["properties"]["operationName"]["maxLength"],
        MAX_OPERATION_NAME_CHARS
    );
    for member in ["variables", "extensions"] {
        assert_eq!(
            schema["properties"][member]["type"],
            json!(["object", "null"]),
            "{member}"
        );
    }
    assert_eq!(schema["additionalProperties"], true);
    assert_eq!(schema["description"], REQUEST_TEXT);
}

#[test]
fn the_response_schema_always_holds_data_and_may_hold_errors_and_extensions() {
    let schema = serde_json::to_value(GraphQLResponse::schema()).unwrap();

    assert_eq!(schema["required"], json!(["data"]));
    assert!(schema["properties"]["data"].get("type").is_none());
    assert_eq!(schema["properties"]["errors"]["type"], "array");
    assert_eq!(
        schema["properties"]["errors"]["items"]["required"],
        json!(["message"])
    );
    assert_eq!(
        schema["properties"]["errors"]["items"]["properties"]["path"]["items"]["type"],
        json!(["string", "integer"])
    );
    assert_eq!(schema["properties"]["extensions"]["type"], "object");
    assert_eq!(schema["description"], RESPONSE_TEXT);
}

#[test]
fn the_get_parameters_are_the_query_string_of_graphql_over_http() {
    let parameters = serde_json::to_value(GetParams::into_params(|| None)).unwrap();

    let names: Vec<(&str, &str, bool)> = parameters
        .as_array()
        .unwrap()
        .iter()
        .map(|parameter| {
            (
                parameter["name"].as_str().unwrap(),
                parameter["in"].as_str().unwrap(),
                parameter["required"].as_bool().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        names,
        [
            ("query", "query", true),
            ("operationName", "query", false),
            ("variables", "query", false),
            ("extensions", "query", false),
        ]
    );
    assert_eq!(parameters[0]["example"], EXAMPLE_SUBSCRIPTION);
    assert_eq!(parameters[0]["description"], QUERY_TEXT);
    assert_eq!(
        parameters[0]["schema"],
        json!({"type": "string", "maxLength": MAX_QUERY_CHARS})
    );
    assert_eq!(parameters[1]["description"], OPERATION_TEXT);
    assert_eq!(
        parameters[1]["schema"],
        json!({"type": "string", "maxLength": MAX_OPERATION_NAME_CHARS})
    );
    assert_eq!(parameters[1]["example"], EXAMPLE_OPERATION);
    for (index, text) in [(2, VARIABLES_TEXT), (3, EXTENSIONS_TEXT)] {
        assert_eq!(
            parameters[index]["schema"],
            json!({
                "type": "string",
                "examples": ["{}"],
                "contentMediaType": "application/json",
                "contentSchema": {"type": "object"}
            })
        );
        assert_eq!(parameters[index]["description"], text);
        assert_eq!(parameters[index]["example"], "{}");
    }
}

#[test]
fn the_cursor_header_admits_any_value_which_only_a_subscription_checks() {
    let parameters = serde_json::to_value(CursorHeader::into_params(|| None)).unwrap();

    assert_eq!(
        parameters,
        json!([{
            "name": "Last-Event-ID",
            "in": "header",
            "description": CURSOR_TEXT,
            "required": false,
            "schema": {"type": "string"},
            "example": EXAMPLE_ID
        }])
    );
}
