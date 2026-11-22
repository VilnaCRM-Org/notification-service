use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode, header};
use health_check_domain::Dependency;
use http_body_util::BodyExt as _;
use serde_json::Value;
use test_support::probe::StubProbe;
use test_support::static_topic::StaticTopic;
use tokio::sync::watch;

use crate::background::Running;
use crate::bootstrap::tests::wiring::{adapters_on, paced_on, record};
use crate::bootstrap::{Adapters, application, router};
use crate::cli::ServeArgs;
use crate::config::Config;
use crate::graphql::QueryStringLimit;
use crate::graphql::handler::{
    EXPLORER_CONTENT_SECURITY_POLICY, MAX_OPERATION_NAME_CHARS, MAX_QUERY_CHARS,
};
use crate::graphql::schema::builder;
use crate::http::docs::DOCS_CONTENT_SECURITY_POLICY;
use crate::http::router::{GRAPHQL_PATH, assemble};
use crate::http::security_headers::DEFAULT_CONTENT_SECURITY_POLICY;
use tower::ServiceExt as _;

/// Method, path, body, expected status and expected content security policy.
type Route<'a> = (Method, String, Option<&'a str>, StatusCode, &'a str);

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

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

fn adapters() -> Adapters {
    let topic = StaticTopic::empty();
    adapters_on(&topic, Box::new(topic.clone()))
}

async fn send_to(app: &Router, request: Request<Body>) -> Answer {
    tokio::time::timeout(Duration::from_secs(5), async {
        let response = app.clone().oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let bytes = body.collect().await.unwrap().to_bytes();
        Answer {
            status: parts.status,
            headers: parts.headers,
            body: String::from_utf8_lossy(&bytes).into_owned(),
        }
    })
    .await
    .unwrap()
}

async fn send(app_env: &str, request: Request<Body>) -> Answer {
    let app = router(adapters(), &config(app_env)).unwrap();
    send_to(&app, request).await
}

fn request(method: Method, path: &str, body: Option<&str>) -> Request<Body> {
    let builder = Request::builder().method(method).uri(path);
    match body {
        Some(body) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    }
}

fn assert_common_headers(answer: &Answer, policy: &str, route: &str) {
    assert!(answer.headers.contains_key("x-request-id"), "{route}");
    assert_eq!(
        answer.headers["x-content-type-options"], "nosniff",
        "{route}"
    );
    assert_eq!(answer.headers["x-frame-options"], "DENY", "{route}");
    assert_eq!(answer.headers["referrer-policy"], "no-referrer", "{route}");
    assert_eq!(answer.headers["content-security-policy"], policy, "{route}");
}

#[tokio::test]
async fn every_route_carries_the_request_id_and_the_security_headers() {
    let graphql = r#"{"query":"{ eventTypes { name } }"}"#;
    let routes: Vec<Route<'_>> = vec![
        (
            Method::GET,
            "/api/event-types".into(),
            None,
            StatusCode::OK,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/event-types?page=0".into(),
            None,
            StatusCode::UNPROCESSABLE_ENTITY,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/events/status".into(),
            None,
            StatusCode::OK,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::POST,
            "/api/graphql".into(),
            Some(graphql),
            StatusCode::OK,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/graphql".into(),
            None,
            StatusCode::OK,
            EXPLORER_CONTENT_SECURITY_POLICY,
        ),
        // The tail of a router without its background work never runs, so the probe of the event
        // stream fails and the health answer is the 500 problem.
        (
            Method::GET,
            "/api/health".into(),
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/ping".into(),
            None,
            StatusCode::OK,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/docs".into(),
            None,
            StatusCode::OK,
            DOCS_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/docs/openapi.json".into(),
            None,
            StatusCode::OK,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/docs/swagger-ui.css".into(),
            None,
            StatusCode::OK,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/docs/missing.js".into(),
            None,
            StatusCode::NOT_FOUND,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/docs/%FF".into(),
            None,
            StatusCode::BAD_REQUEST,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/unknown".into(),
            None,
            StatusCode::NOT_FOUND,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::DELETE,
            "/api/event-types".into(),
            None,
            StatusCode::METHOD_NOT_ALLOWED,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
    ];
    let app = router(adapters(), &config("dev")).unwrap();
    for (method, path, body, status, policy) in routes {
        let route = format!("{method} {path}");
        let answer = send_to(&app, request(method, &path, body)).await;
        assert_eq!(answer.status, status, "{route}: {}", answer.body);
        assert_common_headers(&answer, policy, &route);
    }
}

#[tokio::test]
async fn an_unknown_route_is_the_404_problem() {
    let answer = send("test", request(Method::GET, "/api/unknown", None)).await;

    assert_eq!(answer.status, StatusCode::NOT_FOUND);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["status"], 404);
    assert_eq!(body["detail"], "No route matches the request path.");
}

#[tokio::test]
async fn a_wrong_method_is_the_405_problem_with_the_allowed_methods() {
    let answer = send("test", request(Method::DELETE, "/api/event-types", None)).await;

    assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let mut allowed: Vec<_> = answer.headers[header::ALLOW]
        .to_str()
        .unwrap()
        .split(',')
        .map(str::trim)
        .collect();
    allowed.sort_unstable();
    assert_eq!(allowed, ["GET", "HEAD"]);
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["status"], 405);
}

#[tokio::test]
async fn a_panicking_probe_is_the_bare_500_problem() {
    let adapters = adapters().with_probe(Arc::new(StubProbe::panicking(Dependency::Cache)));
    let app = router(adapters, &config("test")).unwrap();

    let answer = send_to(&app, request(Method::GET, "/api/health", None)).await;

    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert_eq!(
        answer.body,
        r#"{"type":"about:blank","title":"Internal Server Error","status":500}"#
    );
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "panic");
}

#[tokio::test]
async fn a_body_over_the_limit_is_the_413_problem() {
    let mut config = config("test");
    config.max_body_bytes = 1024;
    let app = router(adapters(), &config).unwrap();
    let body = format!(
        r#"{{"query":"{{ eventTypes {{ name }} }}","padding":"{}"}}"#,
        "x".repeat(2048)
    );

    let answer = send_to(&app, request(Method::POST, "/api/graphql", Some(&body))).await;

    assert_eq!(answer.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "413");
}

/// `count` copies of the character U+1F600 in their longest JSON form, a `\u` escape pair: twelve
/// bytes per character.
fn escaped_wide(count: usize) -> String {
    r"\ud83d\ude00".repeat(count)
}

/// `count` copies of the character U+1F600 in their longest query-string form, four
/// percent-encoded bytes: twelve bytes per character.
fn encoded_wide(count: usize) -> String {
    "%F0%9F%98%80".repeat(count)
}

/// The GraphQL body whose `query` and `operationName` hold `query` and `name` copies of U+1F600 in
/// their longest form.
fn widest_body(query: usize, name: usize) -> String {
    format!(
        r#"{{"query":"{}","operationName":"{}","variables":{{}},"extensions":{{}}}}"#,
        escaped_wide(query),
        escaped_wide(name)
    )
}

/// A GET of the GraphQL path that negotiates the event stream, whose `query` and `operationName`
/// hold `query` and `name` copies of U+1F600 in their longest form, and the length of its query
/// string.
fn widest_get(query: usize, name: usize) -> (Request<Body>, usize) {
    let query_string = format!(
        "query={}&operationName={}&variables=%7B%7D&extensions=%7B%7D",
        encoded_wide(query),
        encoded_wide(name)
    );
    let length = query_string.len();
    let request = Request::get(format!("{GRAPHQL_PATH}?{query_string}"))
        .header(header::ACCEPT, "text/event-stream")
        .body(Body::empty())
        .unwrap();
    (request, length)
}

/// The longest request-target, the path with its query string, that the HTTP server (hyper) lets
/// through on an HTTP/1.1 connection, in bytes: `u16::MAX - 1` (decision M6). It is not reached
/// in process, so the test below checks it by length.
const SERVER_TARGET_LIMIT: usize = 65_534;

/// The size limit alone never refuses a `query` or an `operationName` that the document admits
/// (FR-29): with the default `HTTP_MAX_BODY_BYTES`, a `query` and an `operationName` at their
/// bounds, every character in its longest form, are executed on both forms, and one character
/// more is the 422 problem, not the 413, the 414 problem or the bare 414 of the HTTP server. The
/// GET at the bounds stays within the smaller of `HTTP_MAX_BODY_BYTES` and the 65 521 bytes of
/// query string that the HTTP server lets through after `/api/graphql?` (FR-4, decision M6).
#[tokio::test]
async fn the_longest_document_and_operation_name_fit_the_default_size_limit_in_any_encoding() {
    let config = config("test");
    let app = router(adapters(), &config).unwrap();
    assert_eq!(escaped_wide(1).len(), 12);
    assert_eq!(encoded_wide(1).len(), 12);
    let body = widest_body(MAX_QUERY_CHARS, MAX_OPERATION_NAME_CHARS);
    let (get, query_string_length) = widest_get(MAX_QUERY_CHARS, MAX_OPERATION_NAME_CHARS);
    let request_target_length = GRAPHQL_PATH
        .len()
        .saturating_add(1)
        .saturating_add(query_string_length);
    assert!(body.len() <= config.max_body_bytes, "{}", body.len());
    assert!(
        query_string_length <= config.max_body_bytes,
        "{query_string_length}"
    );
    assert!(
        request_target_length <= SERVER_TARGET_LIMIT,
        "{request_target_length}"
    );

    let posted = send_to(&app, request(Method::POST, GRAPHQL_PATH, Some(&body))).await;
    let streamed = send_to(&app, get).await;

    assert_eq!(posted.status, StatusCode::OK, "{}", posted.body);
    assert_eq!(posted.headers[header::CONTENT_TYPE], "application/json");
    assert_eq!(streamed.status, StatusCode::OK, "{}", streamed.body);
    assert_eq!(streamed.headers[header::CONTENT_TYPE], "text/event-stream");
    for (query, name, path) in [
        (MAX_QUERY_CHARS + 1, MAX_OPERATION_NAME_CHARS, "query"),
        (
            MAX_QUERY_CHARS,
            MAX_OPERATION_NAME_CHARS + 1,
            "operationName",
        ),
    ] {
        let body = widest_body(query, name);
        let posted = send_to(&app, request(Method::POST, GRAPHQL_PATH, Some(&body))).await;
        let streamed = send_to(&app, widest_get(query, name).0).await;

        for answer in [posted, streamed] {
            assert_eq!(answer.status, StatusCode::UNPROCESSABLE_ENTITY, "{path}");
            let problem: Value = serde_json::from_str(&answer.body).unwrap();
            assert_eq!(problem["violations"][0]["propertyPath"], path);
            assert_eq!(problem["violations"][0]["code"], "length");
        }
    }
}

#[tokio::test(start_paused = true)]
async fn a_request_over_its_budget_is_the_503_problem() {
    let mut config = config("test");
    config.request_timeout = Duration::from_secs(1);
    config.health_probe_timeout = Duration::from_secs(60);
    let adapters = adapters().with_probe(Arc::new(StubProbe::hanging(Dependency::Cache)));
    let app = router(adapters, &config).unwrap();

    let answer = send_to(&app, request(Method::GET, "/api/health", None)).await;

    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["detail"], "The request exceeded its time budget.");
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "503");
}

#[tokio::test]
async fn the_explorer_exists_only_in_dev() {
    let dev = send("dev", request(Method::GET, "/api/graphql", None)).await;
    assert_eq!(dev.status, StatusCode::OK);
    assert!(
        dev.headers[header::CONTENT_TYPE]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(
        dev.headers[header::CONTENT_SECURITY_POLICY],
        EXPLORER_CONTENT_SECURITY_POLICY
    );
    for app_env in ["test", "prod"] {
        let answer = send(app_env, request(Method::GET, "/api/graphql", None)).await;
        assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED, "{app_env}");
        assert_eq!(answer.headers[header::ALLOW], "POST", "{app_env}");
        assert_eq!(
            answer.headers[header::CONTENT_TYPE],
            "application/problem+json",
            "{app_env}"
        );
    }
}

#[tokio::test]
async fn the_other_routes_behave_the_same_in_every_mode() {
    let probes = [
        (Method::GET, "/ping", None),
        (Method::GET, "/api/health", None),
        (Method::GET, "/api/event-types", None),
        (Method::GET, "/api/events/status", None),
        (Method::GET, "/api/unknown", None),
        (
            Method::POST,
            "/api/graphql",
            Some(r#"{"query":"{ eventTypes { name } }"}"#),
        ),
    ];
    for (method, path, body) in probes {
        let reference = send("dev", request(method.clone(), path, body)).await;
        for app_env in ["test", "prod"] {
            let answer = send(app_env, request(method.clone(), path, body)).await;
            assert_eq!(answer.status, reference.status, "{app_env} {path}");
            assert_eq!(answer.body, reference.body, "{app_env} {path}");
        }
    }
}

#[tokio::test]
async fn a_valid_inbound_request_id_is_kept_and_an_invalid_one_is_replaced() {
    let kept = Request::get("/ping")
        .header("x-request-id", "abc-123")
        .body(Body::empty())
        .unwrap();
    let kept = send("test", kept).await;
    assert_eq!(kept.headers["x-request-id"], "abc-123");

    let invalid = Request::get("/ping")
        .header("x-request-id", "a b")
        .body(Body::empty())
        .unwrap();
    let replaced = send("test", invalid).await;
    assert_ne!(replaced.headers["x-request-id"], "a b");
    assert_eq!(replaced.headers["x-request-id"].len(), 36);
}

#[tokio::test]
async fn no_response_is_compressed_and_no_other_protocol_is_advertised() {
    let app = router(adapters(), &config("test")).unwrap();
    for path in [
        "/ping",
        "/api/event-types",
        "/api/docs/swagger-ui-bundle.js",
        "/api/unknown",
    ] {
        let mut bodies = Vec::new();
        for accept_encoding in [None, Some("gzip, deflate, br, zstd")] {
            let mut builder = Request::get(path);
            if let Some(value) = accept_encoding {
                builder = builder.header(header::ACCEPT_ENCODING, value);
            }
            let response = app
                .clone()
                .oneshot(builder.body(Body::empty()).unwrap())
                .await
                .unwrap();
            for name in [
                header::CONTENT_ENCODING,
                header::ALT_SVC,
                header::STRICT_TRANSPORT_SECURITY,
            ] {
                assert!(
                    !response.headers().contains_key(&name),
                    "{path} {accept_encoding:?}: {name}"
                );
            }
            bodies.push(response.into_body().collect().await.unwrap().to_bytes());
        }
        assert!(!bodies[0].is_empty(), "{path}");
        assert_eq!(bodies[0], bodies[1], "{path}");
    }
}

/// The types a handler parameter of this crate may have: their rejections are problems.
const ALLOWED_EXTRACTORS: [&str; 5] = [
    "State<ApiSchema>",
    "ApiPath<",
    "ApiJson<",
    "ValidatedQuery<",
    "ValidatedJson<",
];

/// The handlers that may also take the `HeaderMap`, which is infallible: the two `GraphQL`
/// handlers that can answer an event stream read `Accept` and `Last-Event-ID` (the recorded
/// streaming exception, NFR-13).
const HEADER_MAP_HANDLERS: [&str; 2] = ["execute", "subscribe"];

/// The handlers that may also take the `RawQuery`, which is infallible: the one GET handler of the
/// `GraphQL` path bounds the query string as received and reads it strictly (FR-4, NFR-13).
const RAW_QUERY_HANDLERS: [&str; 1] = ["subscribe"];

/// Whether the handler `handler` may take a parameter of type `kind`.
fn is_allowed(handler: &str, kind: &str) -> bool {
    ALLOWED_EXTRACTORS
        .iter()
        .any(|allowed| kind.starts_with(allowed))
        || (kind == "HeaderMap" && HEADER_MAP_HANDLERS.contains(&handler))
        || (kind == "RawQuery" && RAW_QUERY_HANDLERS.contains(&handler))
}

/// The handler sources of the crate: every file under `src/http` and `src/graphql` with a
/// `pub async fn`, except the request id middleware, whose parameters are not extractors.
const HANDLER_SOURCES: [(&str, &str); 3] = [
    ("http/docs.rs", include_str!("../docs.rs")),
    ("http/ping.rs", include_str!("../ping.rs")),
    (
        "graphql/handler.rs",
        include_str!("../../graphql/handler.rs"),
    ),
];

/// Returns `name: parameter` for every parameter of a `pub async fn` of `source` whose type is not
/// an allowed extractor.
fn disallowed_parameters(source: &str) -> Vec<String> {
    let mut disallowed = Vec::new();
    for chunk in source.split("pub async fn ").skip(1) {
        let (name, after) = chunk.split_once('(').unwrap();
        let mut depth = 1_usize;
        let mut parameters = vec![String::new()];
        for character in after.chars() {
            match character {
                '(' | '<' => depth = depth.saturating_add(1),
                ')' | '>' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if depth == 0 {
                break;
            }
            if character == ',' && depth == 1 {
                parameters.push(String::new());
            } else if let Some(current) = parameters.last_mut() {
                current.push(character);
            }
        }
        for parameter in parameters.iter().map(|parameter| parameter.trim()) {
            if parameter.is_empty() {
                continue;
            }
            let kind = parameter
                .split_once(": ")
                .map_or(parameter, |(_, kind)| kind)
                .trim();
            if !is_allowed(name.trim(), kind) {
                disallowed.push(format!("{}: {parameter}", name.trim()));
            }
        }
    }
    disallowed
}

/// The production files under `directory` of `src` that hold a `pub async fn`, as paths relative
/// to `src`.
fn files_with_handlers(directory: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut pending = vec![root.join(directory)];
    let mut found = Vec::new();
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            pending.extend(
                std::fs::read_dir(&path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if !path.ends_with("tests.rs")
            && std::fs::read_to_string(&path)
                .unwrap()
                .contains("pub async fn ")
        {
            let relative = path.strip_prefix(&root).unwrap();
            found.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    found.sort_unstable();
    found
}

#[test]
fn handlers_use_only_the_problem_aware_extractors() {
    for (file, source) in HANDLER_SOURCES {
        assert_eq!(
            disallowed_parameters(source),
            Vec::<String>::new(),
            "{file}"
        );
    }
    assert!(
        HANDLER_SOURCES[0]
            .1
            .contains("ApiPath(file): ApiPath<String>")
    );
    assert!(HANDLER_SOURCES[2].1.contains("ApiJson(body): ApiJson<"));
    assert!(HANDLER_SOURCES[2].1.contains(
        "pub async fn execute(\n    State(schema): State<ApiSchema>,\n    headers: HeaderMap,\n"
    ));
    for handler in RAW_QUERY_HANDLERS {
        assert!(
            HANDLER_SOURCES[2].1.contains(&format!(
                "pub async fn {handler}(\n    State(schema): State<ApiSchema>,\n    headers: HeaderMap,\n    RawQuery(query): RawQuery,\n) -> Response {{"
            )),
            "{handler}"
        );
    }
    let mut expected: Vec<String> = HANDLER_SOURCES
        .iter()
        .map(|(file, _)| (*file).to_owned())
        .chain(std::iter::once("http/request_id.rs".to_owned()))
        .collect();
    expected.sort_unstable();
    let mut found = files_with_handlers("http");
    found.extend(files_with_handlers("graphql"));
    found.sort_unstable();
    assert_eq!(found, expected);
}

#[rstest::rstest]
#[case::raw_path("pub async fn h(Path(file): Path<String>) -> Response {}")]
#[case::raw_json("pub async fn h(State(s): State<ApiSchema>, Json(b): Json<Value>) -> Response {}")]
#[case::raw_query("pub async fn h(Query(q): Query<Params>) -> Response {}")]
#[case::raw_body("pub async fn h(body: String) -> Response {}")]
#[case::header_map_of_another_handler("pub async fn h(headers: HeaderMap) -> Response {}")]
#[case::header_map_of_the_explorer("pub async fn explorer(headers: HeaderMap) -> Response {}")]
#[case::a_type_that_starts_like_the_header_map(
    "pub async fn execute(headers: HeaderMapLike) -> Response {}"
)]
#[case::raw_json_of_execute(
    "pub async fn execute(headers: HeaderMap, Json(b): Json<Value>) -> Response {}"
)]
#[case::raw_query_of_execute(
    "pub async fn execute(headers: HeaderMap, RawQuery(q): RawQuery) -> Response {}"
)]
#[case::raw_query_of_the_explorer("pub async fn explorer(RawQuery(q): RawQuery) -> Response {}")]
#[case::raw_query_of_another_handler("pub async fn h(RawQuery(q): RawQuery) -> Response {}")]
#[case::query_of_subscribe(
    "pub async fn subscribe(headers: HeaderMap, Query(q): Query<Params>) -> Response {}"
)]
#[case::query_of_subscribe_beside_its_raw_query(
    "pub async fn subscribe(RawQuery(r): RawQuery, Query(q): Query<Params>) -> Response {}"
)]
#[case::header_map_of_the_former_dev_handler(
    "pub async fn subscribe_or_explore(headers: HeaderMap) -> Response {}"
)]
#[case::raw_query_of_the_former_dev_handler(
    "pub async fn subscribe_or_explore(RawQuery(r): RawQuery) -> Response {}"
)]
#[case::a_type_that_starts_like_the_raw_query(
    "pub async fn subscribe(RawQuery(q): RawQueryLike) -> Response {}"
)]
fn the_extractor_check_reports_a_planted_violation(#[case] source: &str) {
    assert_eq!(disallowed_parameters(source).len(), 1, "{source}");
}

#[test]
fn the_extractor_check_accepts_the_header_map_of_the_two_streaming_handlers_only() {
    let source = "pub async fn execute(State(schema): State<ApiSchema>, headers: HeaderMap, \
        ApiJson(body): ApiJson<Map<String, Value>>) -> Result<Response, Problem> {}";

    assert_eq!(disallowed_parameters(source), Vec::<String>::new());
    for handler in RAW_QUERY_HANDLERS {
        assert_eq!(
            disallowed_parameters(&source.replace("fn execute", &format!("fn {handler}"))),
            Vec::<String>::new(),
            "{handler}"
        );
    }
    assert_eq!(
        disallowed_parameters(&source.replace("fn execute", "fn explorer")),
        ["explorer: headers: HeaderMap"]
    );
}

#[test]
fn the_extractor_check_accepts_the_raw_query_of_the_get_handler_only() {
    let source = "pub async fn subscribe(State(schema): State<ApiSchema>, headers: HeaderMap, \
        RawQuery(query): RawQuery) -> Response {}";

    assert_eq!(disallowed_parameters(source), Vec::<String>::new());
    assert_eq!(
        disallowed_parameters(&source.replace("fn subscribe", "fn subscribe_or_explore")),
        [
            "subscribe_or_explore: headers: HeaderMap",
            "subscribe_or_explore: RawQuery(query): RawQuery"
        ]
    );
    assert_eq!(
        disallowed_parameters(&source.replace("fn subscribe", "fn execute")),
        ["execute: RawQuery(query): RawQuery"]
    );
}

#[test]
fn the_extractor_check_accepts_the_allowed_extractors() {
    let source = "pub async fn h(State(s): State<ApiSchema>, ApiPath(f): ApiPath<String>, \
        ApiJson(b): ApiJson<Map<String, Value>>, ValidatedQuery(q): ValidatedQuery<Q>, \
        ValidatedJson(j): ValidatedJson<J>) -> Response {}";

    assert_eq!(disallowed_parameters(source), Vec::<String>::new());
}

const ORIGIN: &str = "https://app.example.com";
/// The `Sec-WebSocket-Key` of the test handshakes: sixteen zero bytes in base64, a valid key that
/// no secrets scanner takes for a credential.
const HANDSHAKE_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAA==";

fn config_with_origins(origins: crate::config::AllowedOrigins) -> Config {
    let mut config = config("test");
    config.allowed_origins = origins;
    config
}

fn from_origin(
    method: Method,
    path: &str,
    body: Option<&str>,
    origin: Option<&str>,
) -> Request<Body> {
    let mut request = request(method, path, body);
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
    }
    request
}

/// The probe of the cache behind a kind of answer.
type CacheProbe = fn(Dependency) -> StubProbe;

/// A probe of the cache that makes the health route the 500 problem.
const PANICKING: CacheProbe = StubProbe::panicking;

/// A probe of the cache that keeps the health route past the request budget.
const HANGING: CacheProbe = StubProbe::hanging;

/// Method, path, body, status and the probe of the cache of a request to one kind of answer.
type Kind = (
    Method,
    &'static str,
    Option<&'static str>,
    StatusCode,
    CacheProbe,
);

/// A request to each kind of answer: a success, GraphQL, the 404, 405, 422 and 500 problems, the
/// 503 problem of the event stream of an unstarted hub and the 503 problem of the request budget.
fn answers_of_every_kind() -> Vec<Kind> {
    vec![
        (
            Method::GET,
            "/api/event-types",
            None,
            StatusCode::OK,
            PANICKING,
        ),
        (
            Method::POST,
            "/api/graphql",
            Some(r#"{"query":"{ eventTypes { name } }"}"#),
            StatusCode::OK,
            PANICKING,
        ),
        (
            Method::GET,
            "/api/unknown",
            None,
            StatusCode::NOT_FOUND,
            PANICKING,
        ),
        (
            Method::DELETE,
            "/api/event-types",
            None,
            StatusCode::METHOD_NOT_ALLOWED,
            PANICKING,
        ),
        (
            Method::GET,
            "/api/event-types?page=0",
            None,
            StatusCode::UNPROCESSABLE_ENTITY,
            PANICKING,
        ),
        (
            Method::GET,
            "/api/health",
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
            PANICKING,
        ),
        (
            Method::GET,
            "/api/events/stream?types=a",
            None,
            StatusCode::SERVICE_UNAVAILABLE,
            PANICKING,
        ),
        (
            Method::GET,
            "/api/health",
            None,
            StatusCode::SERVICE_UNAVAILABLE,
            HANGING,
        ),
    ]
}

/// The router of `config` whose probe of the cache is `probe`; its event stream never starts.
fn probed_router(probe: CacheProbe, config: &Config) -> Router {
    let adapters = adapters().with_probe(Arc::new(probe(Dependency::Cache)));
    router(adapters, config).unwrap()
}

/// The request budget of the answers of every kind.
const REQUEST_BUDGET: Duration = Duration::from_secs(1);

/// `config` with [`REQUEST_BUDGET`] and a health probe timeout far above it, so that a hanging
/// probe of the cache ends in the 503 problem of the budget; the tests run on paused time.
fn budgeted(mut config: Config) -> Config {
    config.request_timeout = REQUEST_BUDGET;
    config.health_probe_timeout = Duration::from_secs(60);
    config
}

/// The body of an answer with the generated request id blanked out wherever it is repeated, so
/// that two answers to the same request compare equal.
fn comparable_body(answer: &Answer) -> String {
    let id = answer.headers["x-request-id"].to_str().unwrap();
    answer.body.replace(id, "<request-id>")
}

/// The headers of an answer as sorted name and value pairs, without the named ones.
fn comparable_headers(headers: &HeaderMap, without: &[&str]) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = headers
        .iter()
        .filter(|(name, _)| !without.contains(&name.as_str()))
        .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_owned()))
        .collect();
    pairs.sort_unstable();
    pairs
}

/// Asserts that two answers have the same status, the same body and the same headers apart from
/// the generated request id and the named headers.
fn assert_same_answer(left: &Answer, right: &Answer, without: &[&str], route: &str) {
    assert_eq!(left.status, right.status, "{route}");
    assert_eq!(comparable_body(left), comparable_body(right), "{route}");
    let mut ignored = vec!["x-request-id"];
    ignored.extend_from_slice(without);
    assert_eq!(
        comparable_headers(&left.headers, &ignored),
        comparable_headers(&right.headers, &ignored),
        "{route}"
    );
}

/// Whether `headers` hold a CORS header: an `access-control-*` header, or a `vary` header that
/// lists `origin`. The `vary: accept` that every answer to a GET of the `GraphQL` path carries is
/// not one (NFR-17).
fn has_cors_headers(headers: &HeaderMap) -> bool {
    headers
        .keys()
        .any(|name| name.as_str().starts_with("access-control-"))
        || vary_values(headers).iter().any(|value| value == "origin")
}

#[tokio::test(start_paused = true)]
async fn without_allowed_origins_no_answer_changes() {
    let without_origins = budgeted(config("test"));

    for (method, path, body, status, probe) in answers_of_every_kind() {
        let route = format!("{method} {path} {status}");
        let app = probed_router(probe, &without_origins);
        let with = send_to(&app, from_origin(method.clone(), path, body, Some(ORIGIN))).await;
        let without = send_to(&app, from_origin(method.clone(), path, body, None)).await;
        assert_eq!(without.status, status, "{route}");
        assert!(!has_cors_headers(&with.headers), "{route}");
        assert!(!has_cors_headers(&without.headers), "{route}");
        assert_same_answer(&with, &without, &[], &route);
    }
    let app = probed_router(PANICKING, &without_origins);
    let preflight = |path: &str| {
        Request::options(path)
            .header(header::ORIGIN, ORIGIN)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
            .body(Body::empty())
            .unwrap()
    };
    let known = send_to(&app, preflight("/api/event-types")).await;
    assert_eq!(known.status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(!has_cors_headers(&known.headers));
    let unknown = send_to(&app, preflight("/api/unknown")).await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
    assert!(!has_cors_headers(&unknown.headers));
}

#[tokio::test(start_paused = true)]
async fn an_allowed_origin_gets_the_cors_headers_on_every_kind_of_answer() {
    let origins = crate::config::AllowedOrigins::List(vec![ORIGIN.parse().unwrap()]);
    let allowing = budgeted(config_with_origins(origins));
    let without_origins = budgeted(config("test"));

    for (method, path, body, status, probe) in answers_of_every_kind() {
        let route = format!("{method} {path} {status}");
        let app = probed_router(probe, &allowing);
        let unset = probed_router(probe, &without_origins);
        let baseline = send_to(&unset, from_origin(method.clone(), path, body, None)).await;
        let allowed = send_to(&app, from_origin(method.clone(), path, body, Some(ORIGIN))).await;
        assert_eq!(allowed.status, status, "{route}");
        assert_same_answer(
            &allowed,
            &baseline,
            &["vary", "access-control-allow-origin"],
            &route,
        );
        assert_eq!(
            allowed.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN],
            ORIGIN,
            "{route}"
        );
        assert!(
            allowed.headers[header::VARY]
                .to_str()
                .unwrap()
                .contains("origin"),
            "{route}"
        );
        assert_common_headers(&allowed, DEFAULT_CONTENT_SECURITY_POLICY, &route);

        let other = send_to(
            &app,
            from_origin(
                method.clone(),
                path,
                body,
                Some("https://other.example.com"),
            ),
        )
        .await;
        assert_eq!(other.status, status, "{route}");
        assert!(
            !other
                .headers
                .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            "{route}"
        );

        let without = send_to(&app, from_origin(method.clone(), path, body, None)).await;
        assert_eq!(without.status, status, "{route}");
        assert!(
            !without
                .headers
                .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            "{route}"
        );
        // The CORS layer adds `vary` to every answer so that a cache keeps the answers to
        // different origins apart; nothing else of an answer without `Origin` changes.
        assert_same_answer(&without, &baseline, &["vary"], &route);
    }
}

#[tokio::test]
async fn a_preflight_is_answered_by_the_router_for_every_path() {
    for origins in [
        crate::config::AllowedOrigins::Any,
        crate::config::AllowedOrigins::List(vec![ORIGIN.parse().unwrap()]),
    ] {
        let app = router(adapters(), &config_with_origins(origins.clone())).unwrap();
        for path in ["/api/event-types", "/api/unknown"] {
            let request = Request::options(path)
                .header(header::ORIGIN, ORIGIN)
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap();

            let answer = send_to(&app, request).await;

            assert_eq!(answer.status, StatusCode::OK, "{origins:?} {path}");
            assert_eq!(answer.body, "", "{path}");
            assert_eq!(answer.headers[header::ACCESS_CONTROL_MAX_AGE], "600");
            assert!(answer.headers.contains_key("x-request-id"), "{path}");
        }
    }
}

#[tokio::test]
async fn a_websocket_handshake_from_a_refused_origin_is_the_403_problem() {
    let app = router(adapters(), &config("test")).unwrap();
    let request = Request::get("/api/event-types")
        .header(header::ORIGIN, ORIGIN)
        .header(header::SEC_WEBSOCKET_KEY, HANDSHAKE_KEY)
        .body(Body::empty())
        .unwrap();

    let answer = send_to(&app, request).await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["detail"], "the origin is not allowed");
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "403");
}

#[tokio::test]
async fn the_cors_layer_wraps_the_refusal_of_a_websocket_handshake() {
    let origins = crate::config::AllowedOrigins::List(vec![ORIGIN.parse().unwrap()]);
    let app = router(adapters(), &config_with_origins(origins)).unwrap();
    let request = Request::get("/api/event-types")
        .header(header::ORIGIN, "https://other.example.com")
        .header(header::SEC_WEBSOCKET_KEY, HANDSHAKE_KEY)
        .body(Body::empty())
        .unwrap();

    let answer = send_to(&app, request).await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["detail"], "the origin is not allowed");
    // The CORS layer adds `vary` only to the answers that pass through it, so the 403 carries it
    // only when the CORS layer is outside the origin check of the handshake.
    assert!(
        answer.headers[header::VARY]
            .to_str()
            .unwrap()
            .contains("origin")
    );
    assert!(
        !answer
            .headers
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "403");
}

#[tokio::test]
async fn the_event_stream_of_an_unstarted_hub_is_the_503_problem_with_the_common_headers() {
    let answer = send(
        "test",
        request(Method::GET, "/api/events/stream?types=user.created", None),
    )
    .await;

    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["detail"], "the node is starting");
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "stream");
}

/// The bound of the start and the shutdown of a started application.
const LIMIT: Duration = Duration::from_secs(10);

/// The router of the application of `config` on an empty static topic, whose tail is paced with
/// the fetch wait of `config`, with its background work running, once its status route reports
/// `ready` (bounded by [`LIMIT`]; the polls are spaced by a timer, so paused time advances).
async fn started(config: &Config) -> (Router, Running) {
    let (app, running, _topic) = started_on_topic(config).await;
    (app, running)
}

/// [`started`], also returning the static topic the application reads, so a test can append a
/// record while it serves.
async fn started_on_topic(config: &Config) -> (Router, Running, StaticTopic) {
    let topic = StaticTopic::empty();
    let (adapters, _fetches) = paced_on(&topic, config.kafka.fetch_wait);
    let application = application(adapters, config).unwrap();
    let running = application.background.spawn();
    let app = application.router;
    tokio::time::timeout(LIMIT, async {
        while !send_to(&app, request(Method::GET, "/api/events/status", None))
            .await
            .body
            .contains(r#""state":"ready""#)
        {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the node becomes ready");
    (app, running, topic)
}

/// The status and the headers of the answer to `request`, whose body is dropped unread: the body
/// of a subscription does not end.
async fn head(app: &Router, request: Request<Body>) -> Answer {
    let response = tokio::time::timeout(Duration::from_secs(5), app.clone().oneshot(request))
        .await
        .expect("the request answers in time")
        .unwrap();
    Answer {
        status: response.status(),
        headers: response.headers().clone(),
        body: String::new(),
    }
}

#[tokio::test(start_paused = true)]
async fn the_event_stream_of_a_started_application_opens_with_the_cors_headers_of_an_allowed_origin()
 {
    let origins = crate::config::AllowedOrigins::List(vec![ORIGIN.parse().unwrap()]);
    let (app, running) = started(&config_with_origins(origins)).await;
    let path = "/api/events/stream?types=user.created";

    let allowed = head(&app, from_origin(Method::GET, path, None, Some(ORIGIN))).await;
    let other = head(
        &app,
        from_origin(Method::GET, path, None, Some("https://other.example.com")),
    )
    .await;
    tokio::time::timeout(LIMIT, running.shutdown())
        .await
        .expect("the shutdown ends");

    assert_eq!(allowed.status, StatusCode::OK);
    assert_eq!(allowed.headers[header::CONTENT_TYPE], "text/event-stream");
    assert_eq!(allowed.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);
    assert!(
        allowed.headers[header::VARY]
            .to_str()
            .unwrap()
            .contains("origin")
    );
    assert_common_headers(&allowed, DEFAULT_CONTENT_SECURITY_POLICY, "sse");
    assert_eq!(other.status, StatusCode::OK);
    assert_eq!(other.headers[header::CONTENT_TYPE], "text/event-stream");
    assert!(
        !other
            .headers
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN)
    );
    assert!(
        other.headers[header::VARY]
            .to_str()
            .unwrap()
            .contains("origin")
    );
}

#[test]
fn the_streaming_routes_are_served_and_in_the_openapi_document() {
    let document = serde_json::to_value(crate::openapi::document()).unwrap();

    let paths = document["paths"].as_object().unwrap();
    assert!(paths.contains_key("/api/events/status"));
    for path in ["/api/events/stream", "/api/events/ws"] {
        let item = paths
            .get(path)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{path} is not a path of the document: {paths:?}"));
        assert_eq!(
            item.keys().map(String::as_str).collect::<Vec<_>>(),
            ["get"],
            "{path}"
        );
    }
}

#[tokio::test]
async fn the_websocket_route_of_an_unstarted_hub_is_the_503_problem_with_the_common_headers() {
    let answer = send(
        "test",
        request(Method::GET, "/api/events/ws?types=user.created", None),
    )
    .await;

    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["detail"], "the node is starting");
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "ws");
}

#[tokio::test]
async fn a_websocket_handshake_of_the_event_stream_from_a_refused_origin_is_the_403_before_its_checks()
 {
    let app = router(adapters(), &config("test")).unwrap();
    let request = Request::get("/api/events/ws?types=User.Created&lastEventId=x")
        .header(header::ORIGIN, ORIGIN)
        .header(header::SEC_WEBSOCKET_KEY, HANDSHAKE_KEY)
        .body(Body::empty())
        .unwrap();

    let answer = send_to(&app, request).await;

    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["detail"], "the origin is not allowed");
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "ws 403");
}

/// Every `vary` value of `headers`, split at the commas, trimmed and in lower case, in order.
fn vary_values(headers: &HeaderMap) -> Vec<String> {
    header_list(headers, &header::VARY)
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

/// The names of the headers of an answer, sorted.
fn header_names(headers: &HeaderMap) -> Vec<&str> {
    let mut names: Vec<&str> = headers.keys().map(HeaderName::as_str).collect();
    names.sort_unstable();
    names
}

/// The explorer page the router answers in `dev`.
const EXPLORER_PAGE: &str = include_str!("../../graphql/explorer.html");

/// The headers of the explorer page: those of today plus `vary`.
const EXPLORER_HEADERS: [&str; 8] = [
    "content-length",
    "content-security-policy",
    "content-type",
    "referrer-policy",
    "vary",
    "x-content-type-options",
    "x-frame-options",
    "x-request-id",
];

/// The headers of the 405 problem of a GET outside `dev`: those of today plus `vary`.
const METHOD_NOT_ALLOWED_HEADERS: [&str; 9] = [
    "allow",
    "content-length",
    "content-security-policy",
    "content-type",
    "referrer-policy",
    "vary",
    "x-content-type-options",
    "x-frame-options",
    "x-request-id",
];

/// The headers of a POST answer of the `GraphQL` path, which `vary_accept` leaves as they are.
const POST_HEADERS: [&str; 7] = [
    "content-length",
    "content-security-policy",
    "content-type",
    "referrer-policy",
    "x-content-type-options",
    "x-frame-options",
    "x-request-id",
];

#[tokio::test]
async fn a_get_or_a_head_of_the_graphql_path_varies_on_accept_in_every_mode() {
    for method in [Method::GET, Method::HEAD] {
        let dev = send("dev", request(method.clone(), GRAPHQL_PATH, None)).await;
        assert_eq!(dev.status, StatusCode::OK, "{method}");
        assert_eq!(
            dev.headers[header::CONTENT_TYPE],
            "text/html; charset=utf-8",
            "{method}"
        );
        assert_eq!(
            dev.headers[header::CONTENT_SECURITY_POLICY],
            EXPLORER_CONTENT_SECURITY_POLICY,
            "{method}"
        );
        assert_eq!(vary_values(&dev.headers), ["accept"], "{method}");
        assert_eq!(header_names(&dev.headers), EXPLORER_HEADERS, "{method}");
        assert_common_headers(&dev, EXPLORER_CONTENT_SECURITY_POLICY, "explorer");

        for app_env in ["test", "prod"] {
            let route = format!("{app_env} {method}");
            let answer = send(app_env, request(method.clone(), GRAPHQL_PATH, None)).await;
            assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED, "{route}");
            assert_eq!(answer.headers[header::ALLOW], "POST", "{route}");
            assert_eq!(
                answer.headers[header::CONTENT_TYPE],
                "application/problem+json",
                "{route}"
            );
            assert_eq!(vary_values(&answer.headers), ["accept"], "{route}");
            assert_eq!(
                header_names(&answer.headers),
                METHOD_NOT_ALLOWED_HEADERS,
                "{route}"
            );
            assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, &route);
        }
    }
    let page = send("dev", request(Method::GET, GRAPHQL_PATH, None)).await;
    assert_eq!(page.body, EXPLORER_PAGE);
    let path_with_query = format!("{GRAPHQL_PATH}?query=%7B%20eventTypes%20%7B%20name%20%7D%20%7D");
    let refused = send("prod", request(Method::GET, &path_with_query, None)).await;
    assert_eq!(refused.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(
        refused.body,
        r#"{"type":"about:blank","title":"Method Not Allowed","status":405}"#
    );
    assert_eq!(vary_values(&refused.headers), ["accept"]);
}

#[tokio::test]
async fn a_refused_websocket_handshake_on_the_graphql_path_varies_on_accept() {
    let app = router(adapters(), &config("test")).unwrap();
    let request = Request::get(GRAPHQL_PATH)
        .header(header::ORIGIN, ORIGIN)
        .header(header::SEC_WEBSOCKET_KEY, HANDSHAKE_KEY)
        .body(Body::empty())
        .unwrap();

    let answer = send_to(&app, request).await;

    // `vary_accept` sits outside the origin check of a handshake, so the 403 carries the value.
    assert_eq!(answer.status, StatusCode::FORBIDDEN);
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["detail"], "the origin is not allowed");
    assert_eq!(vary_values(&answer.headers), ["accept"]);
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "403");
}

#[tokio::test]
async fn no_post_answer_and_no_answer_of_another_path_gains_a_vary_value() {
    let mut limited = config("test");
    limited.max_body_bytes = 1024;
    let app = router(adapters(), &limited).unwrap();
    let oversized = format!(
        r#"{{"query":"{{ eventTypes {{ name }} }}","padding":"{}"}}"#,
        "x".repeat(2048)
    );
    let posts = [
        (r#"{"query":"{ eventTypes { name } }"}"#, StatusCode::OK),
        (r#"{"variables":{}}"#, StatusCode::UNPROCESSABLE_ENTITY),
        ("{", StatusCode::BAD_REQUEST),
        (oversized.as_str(), StatusCode::PAYLOAD_TOO_LARGE),
    ];
    for (body, status) in posts {
        let answer = send_to(&app, request(Method::POST, GRAPHQL_PATH, Some(body))).await;
        assert_eq!(answer.status, status, "{}", answer.body);
        assert_eq!(header_names(&answer.headers), POST_HEADERS, "{status}");
        assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "post");
    }
    let others = [
        (Method::GET, "/ping"),
        (Method::HEAD, "/ping"),
        (Method::GET, "/api/event-types"),
        (Method::GET, "/api/events/status"),
        (Method::GET, "/api/docs/openapi.json"),
        (Method::GET, "/api/unknown"),
        (Method::GET, "/api/graphql/"),
        (Method::DELETE, GRAPHQL_PATH),
    ];
    for (method, path) in others {
        let answer = send_to(&app, request(method.clone(), path, None)).await;
        assert!(
            !answer.headers.contains_key(header::VARY),
            "{method} {path}: {:?}",
            answer.headers
        );
    }
}

#[tokio::test]
async fn an_allowed_origin_appends_vary_origin_after_vary_accept() {
    let origins = crate::config::AllowedOrigins::List(vec![ORIGIN.parse().unwrap()]);
    for (app_env, status) in [
        ("dev", StatusCode::OK),
        ("test", StatusCode::METHOD_NOT_ALLOWED),
    ] {
        let mut allowing = config(app_env);
        allowing.allowed_origins = origins.clone();
        let app = router(adapters(), &allowing).unwrap();

        for origin in [Some(ORIGIN), None] {
            let route = format!("{app_env} {origin:?}");
            let answer = send_to(&app, from_origin(Method::GET, GRAPHQL_PATH, None, origin)).await;
            let values = vary_values(&answer.headers);
            assert_eq!(answer.status, status, "{route}");
            assert_eq!(
                answer.headers.get_all(header::VARY).iter().next().unwrap(),
                "accept",
                "{route}"
            );
            assert_eq!(
                values.iter().filter(|value| *value == "accept").count(),
                1,
                "{route}: {values:?}"
            );
            assert!(values.iter().any(|value| value == "origin"), "{route}");
        }
        let allowed = send_to(
            &app,
            from_origin(Method::GET, GRAPHQL_PATH, None, Some(ORIGIN)),
        )
        .await;
        assert_eq!(allowed.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);

        let post = send_to(
            &app,
            from_origin(
                Method::POST,
                GRAPHQL_PATH,
                Some(r#"{"query":"{ eventTypes { name } }"}"#),
                Some(ORIGIN),
            ),
        )
        .await;
        let values = vary_values(&post.headers);
        assert_eq!(post.status, StatusCode::OK, "{app_env}");
        assert!(!values.iter().any(|value| value == "accept"), "{values:?}");
        assert!(values.iter().any(|value| value == "origin"), "{values:?}");
    }
}

/// The source of the router, read by the test of the middleware order.
const ROUTER_SOURCE: &str = include_str!("../router.rs");

/// The layers of `assemble`, outermost first, each by a text that only its own line holds.
const MIDDLEWARE_ORDER: [&str; 11] = [
    ".layer(from_fn(sanitize_request_id))",
    ".layer(SetRequestIdLayer::new(",
    ".layer(PropagateRequestIdLayer::new(",
    ".layer(security_headers::layer())",
    "TraceLayer::new_for_http()",
    ".option_layer(origin::cors(",
    ".layer(from_fn(vary_accept))",
    "origin::websocket_origin,",
    ".layer(CatchPanicLayer::custom(",
    "request_timeout))",
    ".layer(DefaultBodyLimit::max(",
];

/// The layers as the documentation comment of `assemble` lists them, outermost first, before the
/// body limit that ends the list.
const DOCUMENTED_ORDER: [&str; 8] = [
    "the three request id layers",
    "security headers",
    "tracing",
    "the CORS layer",
    "`vary_accept`",
    "the origin check of a WebSocket handshake",
    "the panic catcher",
    "the request budget",
];

/// The offset of each of `markers` in `text`, each of which must occur exactly once.
fn offsets(text: &str, markers: &[&str]) -> Vec<usize> {
    markers
        .iter()
        .map(|marker| {
            assert_eq!(text.matches(marker).count(), 1, "{marker} in {text}");
            text.find(marker).unwrap()
        })
        .collect()
}

#[test]
fn the_middleware_order_is_the_documented_one() {
    let (before, after) = ROUTER_SOURCE.split_once("pub fn assemble(").unwrap();
    let code = after.split_once("rest.with_state(state)").unwrap().0;
    let comment = before
        .rsplit_once("/// Builds the complete router.")
        .unwrap()
        .1
        .lines()
        .map(|line| line.trim_start_matches("///").trim())
        .collect::<Vec<_>>()
        .join(" ");
    let listing = comment.split_once("the outermost: ").unwrap().1;
    let (listing, _) = listing.split_once("the body limit.").unwrap();

    let in_code = offsets(code, &MIDDLEWARE_ORDER);
    let in_comment = offsets(listing, &DOCUMENTED_ORDER);

    assert!(in_code.is_sorted(), "{in_code:?}");
    assert!(in_comment.is_sorted(), "{in_comment:?}");
    assert!(listing.ends_with("the request budget and "), "{listing}");
}

/// A query bus whose every query waits for ever: a query over SSE that asks it outlasts any
/// request budget.
#[derive(Debug)]
struct PendingQueryBus;

#[async_trait::async_trait]
impl shared_domain::QueryBus for PendingQueryBus {
    async fn ask_erased(
        &self,
        _query_type: std::any::TypeId,
        _name: &'static str,
        _query: shared_domain::ErasedMessage,
    ) -> Result<shared_domain::ErasedMessage, shared_domain::ErasedBusError> {
        std::future::pending().await
    }
}

/// The router of `config` assembled around a started node whose query bus never answers; its
/// schema holds the query-string bound `HTTP_MAX_BODY_BYTES`, as the bootstrap's does.
fn router_with_a_pending_query_bus(config: &Config) -> Router {
    router_around_a_pending_query_bus(
        config,
        Some(QueryStringLimit {
            max_bytes: config.max_body_bytes,
        }),
    )
}

/// The router of `config` assembled around a started node whose query bus never answers, with
/// `limit` in its schema data when there is one.
fn router_around_a_pending_query_bus(config: &Config, limit: Option<QueryStringLimit>) -> Router {
    use event_stream_application::{
        Hub, HubSettings, ReplaySettings, StatusSource, StreamSettings, Subscriptions, TailState,
        Window,
    };

    let hub = Hub::new(HubSettings {
        max_subscribers: 4,
        buffer: 8,
    });
    hub.start(event_stream_domain::Cursor::new(1));
    let window = Arc::new(Window::new(16, usize::MAX));
    let (_tail, tail_state) = watch::channel(TailState::default());
    let subscriptions = Subscriptions::new(
        Arc::clone(&hub),
        Arc::clone(&window),
        Arc::new(StaticTopic::empty()),
        tail_state.clone(),
        ReplaySettings {
            max_events: 1_000,
            max_data_bytes: 65_536,
            permits: 1,
        },
    );
    let event_stream = event_stream_application::http::EventStreamState {
        query_bus: Arc::new(PendingQueryBus),
        subscriptions: Arc::new(subscriptions),
        status: Arc::new(StatusSource::new(
            tail_state,
            window,
            Arc::<Hub>::clone(&hub),
        )),
        hub,
        settings: StreamSettings {
            keep_alive: config.streams.keep_alive,
        },
    };
    let health_check = health_check_application::HealthCheckState {
        service: Arc::new(health_check_application::HealthService::new(
            Vec::new(),
            config.health_probe_timeout,
        )),
    };
    let schema = limit.map_or_else(
        || builder().data(event_stream.clone()).finish(),
        |limit| builder().data(event_stream.clone()).data(limit).finish(),
    );
    let state = crate::http::AppState {
        health_check,
        event_stream,
    };
    assemble(state, schema, config)
}

#[tokio::test(start_paused = true)]
async fn a_query_over_sse_that_outlasts_the_budget_is_the_503_problem() {
    let mut config = config("test");
    config.request_timeout = Duration::from_secs(1);
    let app = router_with_a_pending_query_bus(&config);
    let request = Request::post(GRAPHQL_PATH)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "text/event-stream")
        .body(Body::from(r#"{"query":"{ eventTypes { name } }"}"#))
        .unwrap();

    let answer = send_to(&app, request).await;

    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["detail"], "The request exceeded its time budget.");
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "budget");
}

#[tokio::test(start_paused = true)]
async fn a_query_over_sse_within_the_budget_of_the_assembled_router_is_answered() {
    let config = config("test");
    let app = router_with_a_pending_query_bus(&config);
    let request = Request::post(GRAPHQL_PATH)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "text/event-stream")
        .body(Body::from(
            r#"{"query":"{ eventStreamStatus { subscribers } }"}"#,
        ))
        .unwrap();

    let answer = send_to(&app, request).await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.headers[header::CONTENT_TYPE], "text/event-stream");
    assert_eq!(
        answer.body,
        ":\n\nevent: next\ndata: {\"data\":{\"eventStreamStatus\":{\"subscribers\":0}}}\n\nevent: complete\ndata:\n\n"
    );
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "sse query");
}

/// The next chunk of the event-stream `body`, within [`LIMIT`].
async fn next_chunk(body: &mut Body) -> String {
    let frame = tokio::time::timeout(LIMIT, body.frame())
        .await
        .expect("the body writes in time")
        .expect("the body goes on")
        .unwrap();
    String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_graphql_subscription_outlives_the_request_budget() {
    let keep_alive = Duration::from_secs(2);
    let mut config = config("test");
    config.request_timeout = Duration::from_secs(1);
    config.streams.keep_alive = keep_alive;
    let (app, running, topic) = started_on_topic(&config).await;
    let subscribe = Request::post(GRAPHQL_PATH)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "text/event-stream")
        .body(Body::from(
            r#"{"query":"subscription { events(types: [\"user.created\"]) { id type } }"}"#,
        ))
        .unwrap();

    let response = tokio::time::timeout(LIMIT, app.clone().oneshot(subscribe))
        .await
        .expect("the head is written in time")
        .unwrap();
    let head = tokio::time::Instant::now();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/event-stream"
    );
    let mut body = response.into_body();
    for n in 0..4_u32 {
        assert_eq!(next_chunk(&mut body).await, ":\n\n", "comment {n}");
        assert_eq!(head.elapsed(), keep_alive.saturating_mul(n), "comment {n}");
    }
    let id = topic.append(Some(record(1, "user.created", 0))).sequence();
    assert_eq!(
        next_chunk(&mut body).await,
        format!(
            "event: next\nid: {id}\ndata: {{\"data\":{{\"events\":{{\"id\":\"{id}\",\"type\":\"user.created\"}}}}}}\n\n"
        )
    );
    let delivered = tokio::time::Instant::now();
    assert!(delivered.duration_since(head) >= keep_alive.saturating_mul(3));
    assert_eq!(next_chunk(&mut body).await, ":\n\n");
    assert_eq!(delivered.elapsed(), keep_alive);
    let status = send_to(&app, request(Method::GET, "/api/events/status", None)).await;
    let status: Value = serde_json::from_str(&status.body).unwrap();
    assert_eq!(status["subscribers"], 1);
    drop(body);
    tokio::time::timeout(LIMIT, running.shutdown())
        .await
        .expect("the shutdown ends");
}

// ---- the GET form of the GraphQL path (FR-2, FR-4, FR-5, FR-6) ----

/// The subscription of the GET tests, percent-encoded as the `query` parameter.
const GET_SUBSCRIPTION: &str = "query=subscription%20%7B%20events(types%3A%20%5B%22user.created%22%5D)%20%7B%20id%20type%20data%20%7D%20%7D";

/// A request of `method` to the `GraphQL` path with `query_string` and the request headers
/// `headers`.
fn graphql_request(method: Method, query_string: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("{GRAPHQL_PATH}?{query_string}"));
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::empty()).unwrap()
}

/// The subscriber count of the status route of `app`.
async fn subscribers(app: &Router) -> Value {
    let status = send_to(app, request(Method::GET, "/api/events/status", None)).await;
    serde_json::from_str::<Value>(&status.body).unwrap()["subscribers"].clone()
}

/// The status of the node of `app` once its window holds the event `last`, polled every 10 ms
/// within [`LIMIT`] (the polls are spaced by a timer, so paused time advances).
async fn window_reaches(app: &Router, last: u64) {
    let last = last.to_string();
    tokio::time::timeout(LIMIT, async {
        loop {
            let status = send_to(app, request(Method::GET, "/api/events/status", None)).await;
            let status: Value = serde_json::from_str(&status.body).unwrap();
            if status["windowLast"] == last.as_str() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the window reaches the event");
}

#[tokio::test(start_paused = true)]
async fn a_get_with_sse_is_answered_as_the_post_form_in_every_mode() {
    for app_env in ["dev", "test", "prod"] {
        let (app, running, topic) = started_on_topic(&config(app_env)).await;
        let post = Request::post(GRAPHQL_PATH)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "text/event-stream")
            .body(Body::from(
                r#"{"query":"subscription { events(types: [\"user.created\"]) { id type data } }"}"#,
            ))
            .unwrap();
        let get = graphql_request(
            Method::GET,
            GET_SUBSCRIPTION,
            &[("accept", "text/event-stream")],
        );

        let posted = tokio::time::timeout(LIMIT, app.clone().oneshot(post))
            .await
            .expect("the POST head is written in time")
            .unwrap();
        let got = tokio::time::timeout(LIMIT, app.clone().oneshot(get))
            .await
            .expect("the GET head is written in time")
            .unwrap();

        assert_eq!(got.status(), StatusCode::OK, "{app_env}");
        assert_eq!(got.status(), posted.status(), "{app_env}");
        assert_eq!(
            comparable_headers(got.headers(), &["x-request-id"]),
            comparable_headers(posted.headers(), &["x-request-id"]),
            "{app_env}"
        );
        assert_eq!(got.headers()[header::CONTENT_TYPE], "text/event-stream");
        assert_eq!(vary_values(got.headers()), ["accept"], "{app_env}");
        assert_eq!(subscribers(&app).await, 2, "{app_env}");
        let mut got = got.into_body();
        let mut posted = posted.into_body();
        assert_eq!(next_chunk(&mut got).await, ":\n\n", "{app_env}");
        assert_eq!(next_chunk(&mut posted).await, ":\n\n", "{app_env}");
        let value = record(7, "user.created", 0);
        let envelope = String::from_utf8(value.clone()).unwrap();
        let id = topic.append(Some(value)).sequence();
        let block = next_chunk(&mut got).await;
        assert_eq!(block, next_chunk(&mut posted).await, "{app_env}");
        assert_eq!(
            block,
            format!(
                "event: next\nid: {id}\ndata: {{\"data\":{{\"events\":{{\"id\":\"{id}\",\"type\":\"user.created\",\"data\":{envelope}}}}}}}\n\n"
            ),
            "{app_env}"
        );
        drop(got);
        drop(posted);
        assert_eq!(subscribers(&app).await, 0, "{app_env}");
        tokio::time::timeout(LIMIT, running.shutdown())
            .await
            .expect("the shutdown ends");
    }
}

#[tokio::test(start_paused = true)]
async fn the_last_event_id_header_of_a_get_wins_over_its_argument() {
    let (app, running, topic) = started_on_topic(&config("test")).await;
    let ids: Vec<String> = (1..=3)
        .map(|n| {
            topic
                .append(Some(record(n, "user.created", 0)))
                .sequence()
                .to_string()
        })
        .collect();
    window_reaches(&app, 3).await;
    let with_argument = |cursor: &str| {
        format!(
            "query=subscription%20%7B%20events(types%3A%20%5B%22user.created%22%5D%2C%20lastEventId%3A%20%22{cursor}%22)%20%7B%20id%20%7D%20%7D"
        )
    };
    let cases = [
        (with_argument("1"), vec![("last-event-id", "2")], &ids[2]),
        (with_argument("1"), vec![], &ids[1]),
        (
            GET_SUBSCRIPTION.to_owned(),
            vec![("last-event-id", "1")],
            &ids[1],
        ),
    ];

    for (query_string, cursor, first) in cases {
        let mut headers = vec![("accept", "text/event-stream")];
        headers.extend(cursor.iter().copied());
        let response = head_of(&app, graphql_request(Method::GET, &query_string, &headers)).await;
        let mut body = response.into_body();
        assert_eq!(next_chunk(&mut body).await, ":\n\n", "{query_string}");
        let block = next_chunk(&mut body).await;
        assert!(
            block.starts_with(&format!("event: next\nid: {first}\n")),
            "{query_string} {cursor:?}: {block}"
        );
    }
    tokio::time::timeout(LIMIT, running.shutdown())
        .await
        .expect("the shutdown ends");
}

/// The response to `request`, within five seconds, with its body unread.
async fn head_of(app: &Router, request: Request<Body>) -> axum::http::Response<Body> {
    tokio::time::timeout(Duration::from_secs(5), app.clone().oneshot(request))
        .await
        .expect("the request answers in time")
        .unwrap()
}

#[tokio::test(start_paused = true)]
async fn a_head_that_negotiates_sse_is_the_event_stream_head_without_a_body_and_frees_its_subscriber()
 {
    let (app, running) = started(&config("test")).await;
    assert_eq!(subscribers(&app).await, 0);

    let answer = send_to(
        &app,
        graphql_request(
            Method::HEAD,
            GET_SUBSCRIPTION,
            &[("accept", "text/event-stream")],
        ),
    )
    .await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.headers[header::CONTENT_TYPE], "text/event-stream");
    assert_eq!(answer.headers[header::CACHE_CONTROL], "no-cache");
    assert_eq!(vary_values(&answer.headers), ["accept"]);
    assert_eq!(answer.body, "");
    assert_eq!(subscribers(&app).await, 0);
    tokio::time::timeout(LIMIT, running.shutdown())
        .await
        .expect("the shutdown ends");
}

#[tokio::test]
async fn a_get_that_does_not_negotiate_sse_keeps_its_answer_with_vary_accept_in_every_mode() {
    let browser = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";
    for accept in [None, Some(browser), Some("application/json")] {
        let headers: Vec<(&str, &str)> = accept.iter().map(|value| ("accept", *value)).collect();
        let dev = send(
            "dev",
            graphql_request(Method::GET, GET_SUBSCRIPTION, &headers),
        )
        .await;
        assert_eq!(dev.status, StatusCode::OK, "{accept:?}");
        assert_eq!(dev.body, EXPLORER_PAGE, "{accept:?}");
        assert_eq!(
            dev.headers[header::CONTENT_SECURITY_POLICY],
            EXPLORER_CONTENT_SECURITY_POLICY
        );
        assert_eq!(vary_values(&dev.headers), ["accept"], "{accept:?}");
        for app_env in ["test", "prod"] {
            let answer = send(
                app_env,
                graphql_request(Method::GET, GET_SUBSCRIPTION, &headers),
            )
            .await;
            assert_eq!(
                answer.status,
                StatusCode::METHOD_NOT_ALLOWED,
                "{app_env} {accept:?}"
            );
            assert_eq!(
                answer.headers[header::ALLOW],
                "POST",
                "{app_env} {accept:?}"
            );
            assert_eq!(
                answer.body,
                r#"{"type":"about:blank","title":"Method Not Allowed","status":405}"#
            );
            assert_eq!(
                vary_values(&answer.headers),
                ["accept"],
                "{app_env} {accept:?}"
            );
        }
    }
}

#[tokio::test]
async fn a_get_with_a_wildcard_accept_negotiates_sse_and_one_without_accept_does_not() {
    let wildcard = send(
        "test",
        graphql_request(Method::GET, GET_SUBSCRIPTION, &[("accept", "*/*")]),
    )
    .await;
    let without = send("test", graphql_request(Method::GET, GET_SUBSCRIPTION, &[])).await;

    // The node of `send` never starts, so the subscription it negotiated is the 503 of the hub.
    assert_eq!(wildcard.status, StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = serde_json::from_str(&wildcard.body).unwrap();
    assert_eq!(body["detail"], "the node is starting");
    assert_eq!(vary_values(&wildcard.headers), ["accept"]);
    assert_eq!(without.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(without.headers[header::ALLOW], "POST");
}

#[tokio::test]
async fn the_other_methods_of_the_graphql_path_allow_get_head_and_post_in_every_mode() {
    for app_env in ["dev", "test", "prod"] {
        for method in [Method::PUT, Method::DELETE, Method::PATCH] {
            let route = format!("{app_env} {method}");

            let answer = send(app_env, request(method.clone(), GRAPHQL_PATH, None)).await;

            assert_eq!(answer.status, StatusCode::METHOD_NOT_ALLOWED, "{route}");
            assert_eq!(answer.headers[header::ALLOW], "GET,HEAD,POST", "{route}");
            assert_eq!(
                answer.body, r#"{"type":"about:blank","title":"Method Not Allowed","status":405}"#,
                "{route}"
            );
            assert!(!answer.headers.contains_key(header::VARY), "{route}");
        }
    }
}

#[tokio::test(start_paused = true)]
async fn a_get_query_over_sse_that_outlasts_the_budget_is_the_503_problem_with_vary_accept() {
    let mut config = config("test");
    config.request_timeout = Duration::from_secs(1);
    let app = router_with_a_pending_query_bus(&config);
    let get = graphql_request(
        Method::GET,
        "query=%7B%20eventTypes%20%7B%20name%20%7D%20%7D",
        &[("accept", "text/event-stream")],
    );

    let answer = send_to(&app, get).await;

    assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["detail"], "The request exceeded its time budget.");
    assert_eq!(vary_values(&answer.headers), ["accept"]);
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "budget");
}

#[tokio::test(start_paused = true)]
async fn a_get_query_over_sse_within_the_budget_of_the_assembled_router_is_answered() {
    let app = router_with_a_pending_query_bus(&config("prod"));
    let get = graphql_request(
        Method::GET,
        "query=%7B%20eventStreamStatus%20%7B%20subscribers%20%7D%20%7D",
        &[("accept", "text/event-stream")],
    );

    let answer = send_to(&app, get).await;

    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(
        answer.body,
        ":\n\nevent: next\ndata: {\"data\":{\"eventStreamStatus\":{\"subscribers\":0}}}\n\nevent: complete\ndata:\n\n"
    );
    assert_eq!(vary_values(&answer.headers), ["accept"]);
}

#[tokio::test]
async fn a_get_with_sse_on_a_schema_without_the_query_string_limit_is_the_500_problem_with_vary_accept()
 {
    let config = config("test");
    let app = router_around_a_pending_query_bus(&config, None);

    let answer = send_to(
        &app,
        graphql_request(
            Method::GET,
            GET_SUBSCRIPTION,
            &[("accept", "text/event-stream")],
        ),
    )
    .await;

    assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        answer.body,
        r#"{"type":"about:blank","title":"Internal Server Error","status":500}"#
    );
    assert_eq!(vary_values(&answer.headers), ["accept"]);
}

// ---- browser access to both forms of the GraphQL path (FR-21, NFR-17) ----

#[rstest::rstest]
#[case::no_header(&[], false)]
#[case::vary_accept_alone(&[("vary", "accept")], false)]
#[case::the_head_of_an_event_stream(
    &[("content-type", "text/event-stream"), ("cache-control", "no-cache"), ("vary", "accept")],
    false
)]
#[case::a_vary_value_that_only_contains_the_word(&[("vary", "accept, x-origin")], false)]
#[case::vary_origin(&[("vary", "origin")], true)]
#[case::vary_accept_and_origin(&[("vary", "accept, origin")], true)]
#[case::vary_origin_in_a_field_of_its_own(&[("vary", "accept"), ("vary", "Origin")], true)]
#[case::the_vary_of_every_preflight_request_header(
    &[("vary", "origin, access-control-request-method, access-control-request-headers")],
    true
)]
#[case::an_access_control_header(&[("access-control-allow-origin", "*")], true)]
#[case::the_max_age_of_a_preflight(&[("access-control-max-age", "600")], true)]
fn a_cors_header_is_an_access_control_header_or_a_vary_value_origin(
    #[case] pairs: &[(&str, &str)],
    #[case] expected: bool,
) {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        headers.append(
            name.parse::<HeaderName>().unwrap(),
            value.parse::<HeaderValue>().unwrap(),
        );
    }

    assert_eq!(has_cors_headers(&headers), expected, "{pairs:?}");
}

/// An origin that the allow-list of [`ORIGIN`] does not hold.
const OTHER_ORIGIN: &str = "https://other.example.com";

/// The allow-list that holds [`ORIGIN`] only.
fn only_the_origin() -> crate::config::AllowedOrigins {
    crate::config::AllowedOrigins::List(vec![ORIGIN.parse().unwrap()])
}

/// A value of `CORS_ALLOWED_ORIGINS` and what its answers carry.
struct AllowList {
    origins: crate::config::AllowedOrigins,
    /// The `Access-Control-Allow-Origin` of an answer to [`ORIGIN`].
    allowed: &'static str,
    /// The `Access-Control-Allow-Origin` of an answer to [`OTHER_ORIGIN`], if any.
    to_others: Option<&'static str>,
    /// Whether its answers list `origin` in `vary`: an answer that names the origin depends on it,
    /// so a cache must keep the answers to different origins apart; an answer of `*` does not.
    varies: bool,
}

/// The allow-list of [`ORIGIN`]: its answers name the origin and vary on it, and its answers to
/// [`OTHER_ORIGIN`] carry no `Access-Control-Allow-Origin`.
fn the_origin_alone() -> AllowList {
    AllowList {
        origins: only_the_origin(),
        allowed: ORIGIN,
        to_others: None,
        varies: true,
    }
}

/// The allow-lists of the CORS cases: [`the_origin_alone`] and `*`, whose answers to every origin
/// carry `*`.
fn allow_lists() -> [AllowList; 2] {
    [
        the_origin_alone(),
        AllowList {
            origins: crate::config::AllowedOrigins::Any,
            allowed: "*",
            to_others: Some("*"),
            varies: false,
        },
    ]
}

/// The values of every `name` field of `headers`, split at the commas and trimmed, in order.
fn header_list(headers: &HeaderMap, name: &HeaderName) -> Vec<String> {
    headers
        .get_all(name)
        .iter()
        .flat_map(|value| value.to_str().unwrap().split(','))
        .map(|value| value.trim().to_owned())
        .collect()
}

/// Asserts that no header of an answer allows credentials and none exposes a response header.
fn assert_no_credentials_and_no_exposed_header(headers: &HeaderMap, route: &str) {
    assert!(
        !headers.contains_key(header::ACCESS_CONTROL_ALLOW_CREDENTIALS),
        "{route}: {headers:?}"
    );
    assert!(
        !headers.contains_key(header::ACCESS_CONTROL_EXPOSE_HEADERS),
        "{route}: {headers:?}"
    );
}

/// Asserts that a page of [`ORIGIN`], allowed by `list`, can read `answer`: its
/// `Access-Control-Allow-Origin` is the one of `list`, its `vary` lists `origin` exactly when
/// `list` varies and lists `accept` exactly `accepts` times, and no header allows credentials or
/// exposes a response header.
fn assert_readable_by_the_page(answer: &Answer, list: &AllowList, accepts: usize, route: &str) {
    assert_eq!(
        answer.headers[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        list.allowed,
        "{route}"
    );
    let values = vary_values(&answer.headers);
    assert_eq!(
        values.iter().any(|value| value == "origin"),
        list.varies,
        "{route}: {values:?}"
    );
    assert_eq!(
        values.iter().filter(|value| *value == "accept").count(),
        accepts,
        "{route}: {values:?}"
    );
    assert_no_credentials_and_no_exposed_header(&answer.headers, route);
}

/// The preflight of the POST form from [`ORIGIN`] with the request headers `request_headers`.
fn post_preflight(request_headers: &str) -> Request<Body> {
    Request::options(GRAPHQL_PATH)
        .header(header::ORIGIN, ORIGIN)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, request_headers)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn the_preflight_of_the_post_form_allows_both_forms_to_an_allowed_origin() {
    for list in allow_lists() {
        for app_env in ["dev", "test", "prod"] {
            let mut allowing = config(app_env);
            allowing.allowed_origins = list.origins.clone();
            let app = router(adapters(), &allowing).unwrap();
            for request_headers in ["content-type", "content-type, last-event-id"] {
                let route = format!("{} {app_env} {request_headers}", list.allowed);

                let answer = send_to(&app, post_preflight(request_headers)).await;

                assert_eq!(answer.status, StatusCode::OK, "{route}");
                assert_eq!(answer.body, "", "{route}");
                // The CORS layer answers a preflight itself, so `vary_accept` never sees it.
                assert_readable_by_the_page(&answer, &list, 0, &route);
                assert_eq!(
                    header_list(&answer.headers, &header::ACCESS_CONTROL_ALLOW_METHODS),
                    ["GET", "POST"],
                    "{route}"
                );
                assert_eq!(
                    header_list(&answer.headers, &header::ACCESS_CONTROL_ALLOW_HEADERS)
                        .iter()
                        .map(|name| name.to_ascii_lowercase())
                        .collect::<Vec<_>>(),
                    ["content-type", "last-event-id"],
                    "{route}"
                );
                assert_eq!(
                    answer.headers[header::ACCESS_CONTROL_MAX_AGE],
                    "600",
                    "{route}"
                );
                assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, &route);
            }
        }
    }
}

/// The bound of the query string and of the body of the CORS cases, which the query string of the
/// 414 case exceeds.
const FORM_BYTES: usize = 1024;

/// The body of the subscription of the POST form.
const POST_SUBSCRIPTION: &str =
    r#"{"query":"subscription { events(types: [\"user.created\"]) { id type } }"}"#;

/// A request of one form of the `GraphQL` path that negotiates the event stream: method, path and
/// query string, content type, body and the status of its answer.
type FormCase = (
    Method,
    String,
    Option<&'static str>,
    Option<String>,
    StatusCode,
);

/// The subscription of each form, which a started node answers with the event-stream head.
fn event_streams_of_both_forms() -> [FormCase; 2] {
    [
        (
            Method::POST,
            GRAPHQL_PATH.to_owned(),
            Some("application/json"),
            Some(POST_SUBSCRIPTION.to_owned()),
            StatusCode::OK,
        ),
        (
            Method::GET,
            format!("{GRAPHQL_PATH}?{GET_SUBSCRIPTION}"),
            None,
            None,
            StatusCode::OK,
        ),
    ]
}

/// The requests of both forms that a node which never starts, bounded by [`FORM_BYTES`], answers
/// with a problem: the 503 problem of the refused subscriber of each form (FR-16), the transport
/// problems 415, 422 and 400 of the POST form and 414, 400 and 422 of the GET form (FR-6).
fn refusals_of_both_forms() -> Vec<FormCase> {
    let json = Some("application/json");
    vec![
        (
            Method::POST,
            GRAPHQL_PATH.to_owned(),
            json,
            Some(POST_SUBSCRIPTION.to_owned()),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            Method::GET,
            format!("{GRAPHQL_PATH}?{GET_SUBSCRIPTION}"),
            None,
            None,
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            Method::POST,
            GRAPHQL_PATH.to_owned(),
            Some("text/plain"),
            Some(POST_SUBSCRIPTION.to_owned()),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
        ),
        (
            Method::POST,
            GRAPHQL_PATH.to_owned(),
            json,
            Some(r#"{"variables":{}}"#.to_owned()),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            Method::POST,
            GRAPHQL_PATH.to_owned(),
            json,
            Some("{".to_owned()),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            format!("{GRAPHQL_PATH}?query={}", "x".repeat(FORM_BYTES)),
            None,
            None,
            StatusCode::URI_TOO_LONG,
        ),
        (
            Method::GET,
            format!("{GRAPHQL_PATH}?query=%ZZ"),
            None,
            None,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            format!("{GRAPHQL_PATH}?{GET_SUBSCRIPTION}&variables=1"),
            None,
            None,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ]
}

/// The request of `case` with `Accept: text/event-stream`, from `origin` when there is one.
fn form_request(case: &FormCase, origin: Option<&str>) -> Request<Body> {
    let (method, uri, content_type, body, _) = case;
    let mut builder = Request::builder()
        .method(method.clone())
        .uri(uri.as_str())
        .header(header::ACCEPT, "text/event-stream");
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, *content_type);
    }
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    builder
        .body(body.clone().map_or_else(Body::empty, Body::from))
        .unwrap()
}

/// The route of `case` in the messages of the CORS tests.
fn form_route(case: &FormCase) -> String {
    let (method, uri, content_type, _, status) = case;
    format!("{method} {uri} {content_type:?} {status}")
}

/// A node that never starts, bounded by [`FORM_BYTES`], with the allow-list `origins`.
fn refusing_router(origins: crate::config::AllowedOrigins) -> Router {
    let mut refusing = config_with_origins(origins);
    refusing.max_body_bytes = FORM_BYTES;
    router(adapters(), &refusing).unwrap()
}

#[tokio::test]
async fn an_allowed_origin_reads_the_problems_of_both_forms() {
    let unset = refusing_router(crate::config::AllowedOrigins::NoOrigin);
    for list in allow_lists() {
        let app = refusing_router(list.origins.clone());
        for case in refusals_of_both_forms() {
            let route = format!("{} {}", list.allowed, form_route(&case));
            let accepts = usize::from(case.0 == Method::GET);

            let answer = send_to(&app, form_request(&case, Some(ORIGIN))).await;
            let other = send_to(&app, form_request(&case, Some(OTHER_ORIGIN))).await;
            let baseline = send_to(&unset, form_request(&case, None)).await;

            assert_eq!(answer.status, case.4, "{route}: {}", answer.body);
            assert_eq!(
                answer.headers[header::CONTENT_TYPE],
                "application/problem+json",
                "{route}"
            );
            assert_readable_by_the_page(&answer, &list, accepts, &route);
            // The CORS layer changes nothing but `vary` and `Access-Control-Allow-Origin`.
            assert_same_answer(
                &answer,
                &baseline,
                &["vary", "access-control-allow-origin"],
                &route,
            );
            assert_eq!(other.status, case.4, "{route}");
            assert_eq!(
                other
                    .headers
                    .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                    .map(|value| value.to_str().unwrap()),
                list.to_others,
                "{route}"
            );
            assert_no_credentials_and_no_exposed_header(&other.headers, &route);
        }
    }
}

#[tokio::test(start_paused = true)]
async fn an_allowed_origin_reads_the_event_stream_of_both_forms() {
    for list in allow_lists() {
        let (app, running) = started(&config_with_origins(list.origins.clone())).await;
        for case in event_streams_of_both_forms() {
            let route = format!("{} {}", list.allowed, form_route(&case));

            let answer = head(&app, form_request(&case, Some(ORIGIN))).await;
            let other = head(&app, form_request(&case, Some(OTHER_ORIGIN))).await;

            assert_eq!(answer.status, StatusCode::OK, "{route}");
            assert_eq!(
                answer.headers[header::CONTENT_TYPE],
                "text/event-stream",
                "{route}"
            );
            assert_readable_by_the_page(&answer, &list, 1, &route);
            assert_eq!(other.status, StatusCode::OK, "{route}");
            assert_eq!(
                other.headers[header::CONTENT_TYPE],
                "text/event-stream",
                "{route}"
            );
            assert_eq!(
                other
                    .headers
                    .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                    .map(|value| value.to_str().unwrap()),
                list.to_others,
                "{route}"
            );
            assert_no_credentials_and_no_exposed_header(&other.headers, &route);
        }
        tokio::time::timeout(LIMIT, running.shutdown())
            .await
            .expect("the shutdown ends");
    }
}

#[tokio::test(start_paused = true)]
async fn the_get_form_is_served_to_an_allowed_origin_as_an_event_source_sends_it_without_a_preflight()
 {
    let (app, running, topic) = started_on_topic(&config_with_origins(only_the_origin())).await;
    let ids: Vec<String> = (1..=2)
        .map(|n| {
            topic
                .append(Some(record(n, "user.created", 0)))
                .sequence()
                .to_string()
        })
        .collect();
    window_reaches(&app, 2).await;
    // The headers of an `EventSource` request: `Origin`, `Accept: text/event-stream` and, on
    // reconnect, `Last-Event-ID`, all set by the browser, which sends no preflight before it. No
    // `OPTIONS` reaches the node before either GET.
    let connect = [("origin", ORIGIN), ("accept", "text/event-stream")];
    let reconnect = [
        ("origin", ORIGIN),
        ("accept", "text/event-stream"),
        ("last-event-id", ids[0].as_str()),
    ];

    let first = head_of(
        &app,
        graphql_request(Method::GET, GET_SUBSCRIPTION, &connect),
    )
    .await;
    let again = head_of(
        &app,
        graphql_request(Method::GET, GET_SUBSCRIPTION, &reconnect),
    )
    .await;

    for (response, name) in [(&first, "connect"), (&again, "reconnect")] {
        let answer = Answer {
            status: response.status(),
            headers: response.headers().clone(),
            body: String::new(),
        };
        assert_eq!(answer.status, StatusCode::OK, "{name}");
        assert_eq!(
            answer.headers[header::CONTENT_TYPE],
            "text/event-stream",
            "{name}"
        );
        assert_readable_by_the_page(&answer, &the_origin_alone(), 1, name);
    }
    let mut first = first.into_body();
    assert_eq!(next_chunk(&mut first).await, ":\n\n");
    let mut again = again.into_body();
    assert_eq!(next_chunk(&mut again).await, ":\n\n");
    let block = next_chunk(&mut again).await;
    assert!(
        block.starts_with(&format!("event: next\nid: {}\n", ids[1])),
        "{block}"
    );
    drop(first);
    drop(again);
    tokio::time::timeout(LIMIT, running.shutdown())
        .await
        .expect("the shutdown ends");
}

#[tokio::test(start_paused = true)]
async fn without_allowed_origins_no_answer_of_either_form_carries_a_cors_header() {
    let app = refusing_router(crate::config::AllowedOrigins::NoOrigin);
    for case in refusals_of_both_forms() {
        let route = form_route(&case);
        let accepts: &[&str] = if case.0 == Method::GET {
            &["accept"]
        } else {
            &[]
        };

        let with = send_to(&app, form_request(&case, Some(ORIGIN))).await;
        let without = send_to(&app, form_request(&case, None)).await;

        assert_eq!(with.status, case.4, "{route}: {}", with.body);
        assert!(!has_cors_headers(&with.headers), "{route}");
        assert_eq!(vary_values(&with.headers), accepts, "{route}");
        assert_same_answer(&with, &without, &[], &route);
    }

    let preflight = send_to(&app, post_preflight("content-type, last-event-id")).await;

    assert_eq!(preflight.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(preflight.headers[header::ALLOW], "GET,HEAD,POST");
    assert_eq!(
        preflight.body,
        r#"{"type":"about:blank","title":"Method Not Allowed","status":405}"#
    );
    assert!(!has_cors_headers(&preflight.headers));
    assert!(!preflight.headers.contains_key(header::VARY));

    let (app, running) = started(&config("test")).await;
    for case in event_streams_of_both_forms() {
        let route = form_route(&case);

        let answer = head(&app, form_request(&case, Some(ORIGIN))).await;

        assert_eq!(answer.status, StatusCode::OK, "{route}");
        assert_eq!(
            answer.headers[header::CONTENT_TYPE],
            "text/event-stream",
            "{route}"
        );
        assert!(!has_cors_headers(&answer.headers), "{route}");
        assert_eq!(vary_values(&answer.headers), ["accept"], "{route}");
    }
    tokio::time::timeout(LIMIT, running.shutdown())
        .await
        .expect("the shutdown ends");
}
