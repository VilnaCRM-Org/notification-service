use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use health_check_domain::{Dependency, HealthProbe};
use http_body_util::BodyExt as _;
use serde_json::Value;
use test_support::clock::FixedClock;
use test_support::id::SequenceUuidGenerator;
use test_support::probe::StubProbe;
use test_support::repository::InMemoryExampleApiResourceRepository;
use test_support::static_topic::StaticTopic;

use crate::bootstrap::{Adapters, event_bus, router};
use crate::cli::ServeArgs;
use crate::config::Config;
use crate::graphql::handler::EXPLORER_CONTENT_SECURITY_POLICY;
use crate::http::docs::DOCS_CONTENT_SECURITY_POLICY;
use crate::http::security_headers::DEFAULT_CONTENT_SECURITY_POLICY;
use tower::ServiceExt as _;

const MISSING_ID: &str = "00000000-0000-7000-8000-0000000000ff";

/// Method, path, body, expected status and expected content security policy.
type Route<'a> = (Method, String, Option<&'a str>, StatusCode, &'a str);

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

fn config(app_env: &str) -> Config {
    let mut args = ServeArgs {
        app_env: Some(app_env.to_owned()),
        aws_region: Some("us-east-1".to_owned()),
        ..ServeArgs::default()
    };
    args.database.url = Some("postgres://app:unused@127.0.0.1:1/app".to_owned());
    Config::from_args(args).unwrap()
}

fn adapters() -> Adapters {
    let probes: Vec<Arc<dyn HealthProbe>> = vec![
        Arc::new(StubProbe::healthy(Dependency::Database)),
        Arc::new(StubProbe::healthy(Dependency::Cache)),
        Arc::new(StubProbe::healthy(Dependency::MessageBroker)),
    ];
    Adapters {
        repository: Arc::new(InMemoryExampleApiResourceRepository::new()),
        probes,
        event_bus: event_bus(),
        clock: Arc::new(FixedClock::at(std::time::SystemTime::UNIX_EPOCH)),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(1)),
        topic_probe: Arc::new(StaticTopic::empty()),
        topic_tail: Box::new(StaticTopic::empty()),
        topic_replay: Arc::new(StaticTopic::empty()),
    }
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
    let graphql = r#"{"query":"{ exampleApiResources { id } }"}"#;
    let create = r#"{"name":"Name","description":"Description"}"#;
    let resource = format!("/api/example_api_resources/{MISSING_ID}");
    let routes: Vec<Route<'_>> = vec![
        (
            Method::POST,
            "/api/example_api_resources".into(),
            Some(create),
            StatusCode::CREATED,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            resource,
            None,
            StatusCode::NOT_FOUND,
            DEFAULT_CONTENT_SECURITY_POLICY,
        ),
        (
            Method::GET,
            "/api/example_api_resources".into(),
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
        (
            Method::GET,
            "/api/health".into(),
            None,
            StatusCode::NO_CONTENT,
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
            "/api/example_api_resources".into(),
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
    let answer = send(
        "test",
        request(Method::DELETE, "/api/example_api_resources", None),
    )
    .await;

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
    assert_eq!(allowed, ["GET", "HEAD", "POST"]);
    let body: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["status"], 405);
}

#[tokio::test]
async fn a_panicking_probe_is_the_bare_500_problem() {
    let adapters = adapters().with_probe(Arc::new(StubProbe::panicking(Dependency::Database)));
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
    let body = format!(r#"{{"name":"{}","description":"d"}}"#, "x".repeat(2048));

    let answer = send_to(
        &app,
        request(Method::POST, "/api/example_api_resources", Some(&body)),
    )
    .await;

    assert_eq!(answer.status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        answer.headers[header::CONTENT_TYPE],
        "application/problem+json"
    );
    assert_common_headers(&answer, DEFAULT_CONTENT_SECURITY_POLICY, "413");
}

#[tokio::test(start_paused = true)]
async fn a_request_over_its_budget_is_the_503_problem() {
    let mut config = config("test");
    config.request_timeout = Duration::from_secs(1);
    config.health_probe_timeout = Duration::from_secs(60);
    let adapters = adapters().with_probe(Arc::new(StubProbe::hanging(Dependency::Database)));
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
        (Method::GET, "/api/example_api_resources", None),
        (Method::GET, "/api/unknown", None),
        (
            Method::POST,
            "/api/graphql",
            Some(r#"{"query":"{ exampleApiResources { id } }"}"#),
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
        "/api/example_api_resources",
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
            if !ALLOWED_EXTRACTORS
                .iter()
                .any(|allowed| kind.starts_with(allowed))
            {
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
fn the_extractor_check_reports_a_planted_violation(#[case] source: &str) {
    assert_eq!(disallowed_parameters(source).len(), 1, "{source}");
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

/// Method, path, body and status of a request to each kind of answer: a success, GraphQL, the
/// 404, 405, 422 and 500 problems.
fn answers_of_every_kind() -> Vec<(Method, &'static str, Option<&'static str>, StatusCode)> {
    vec![
        (Method::GET, "/api/event-types", None, StatusCode::OK),
        (
            Method::POST,
            "/api/graphql",
            Some(r#"{"query":"{ exampleApiResources { id } }"}"#),
            StatusCode::OK,
        ),
        (Method::GET, "/api/unknown", None, StatusCode::NOT_FOUND),
        (
            Method::DELETE,
            "/api/event-types",
            None,
            StatusCode::METHOD_NOT_ALLOWED,
        ),
        (
            Method::GET,
            "/api/event-types?page=0",
            None,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            Method::GET,
            "/api/health",
            None,
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ]
}

fn panicking_router(config: &Config) -> Router {
    let adapters = adapters().with_probe(Arc::new(StubProbe::panicking(Dependency::Database)));
    router(adapters, config).unwrap()
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

fn has_cors_headers(headers: &HeaderMap) -> bool {
    headers
        .keys()
        .any(|name| name.as_str().starts_with("access-control-"))
        || headers.contains_key(header::VARY)
}

#[tokio::test]
async fn without_allowed_origins_no_answer_changes() {
    let app = panicking_router(&config("test"));

    for (method, path, body, status) in answers_of_every_kind() {
        let route = format!("{method} {path}");
        let with = send_to(&app, from_origin(method.clone(), path, body, Some(ORIGIN))).await;
        let without = send_to(&app, from_origin(method.clone(), path, body, None)).await;
        assert_eq!(without.status, status, "{route}");
        assert!(!has_cors_headers(&with.headers), "{route}");
        assert!(!has_cors_headers(&without.headers), "{route}");
        assert_same_answer(&with, &without, &[], &route);
    }
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

#[tokio::test]
async fn an_allowed_origin_gets_the_cors_headers_on_every_kind_of_answer() {
    let origins = crate::config::AllowedOrigins::List(vec![ORIGIN.parse().unwrap()]);
    let app = panicking_router(&config_with_origins(origins));
    let unset = panicking_router(&config("test"));

    for (method, path, body, status) in answers_of_every_kind() {
        let route = format!("{method} {path}");
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

#[test]
fn the_event_stream_route_is_served_but_not_in_the_openapi_document() {
    let document = serde_json::to_value(crate::openapi::document()).unwrap();

    let paths = document["paths"].as_object().unwrap();
    assert!(paths.contains_key("/api/events/status"));
    assert!(
        paths.keys().all(|path| !path.contains("stream")),
        "{paths:?}"
    );
}
