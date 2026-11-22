use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::handler::Handler;
use axum::http::header::VARY;
use axum::http::{HeaderMap, Method, Request, StatusCode};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::any;
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use shared_application::http::fallback::panic_response;
use shared_application::http::middleware::request_timeout;
use tower::{ServiceBuilder, ServiceExt as _};
use tower_http::catch_panic::CatchPanicLayer;

use super::vary_accept;
use crate::http::router::GRAPHQL_PATH;

/// The request budget of the test router.
const BUDGET: Duration = Duration::from_secs(1);

/// How long the slow handler sleeps: past [`BUDGET`].
const LATE: Duration = Duration::from_secs(2);

/// The bound of every request of these tests, past [`LATE`].
const LIMIT: Duration = Duration::from_secs(5);

/// A path other than the `GraphQL` path.
const OTHER_PATH: &str = "/api/event-types";

/// The `GraphQL` path with a query string, as the GET form of a subscription sends it.
const GRAPHQL_PATH_WITH_QUERY: &str = "/api/graphql?query=subscription%7Bevents%7Bid%7D%7D";

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

/// The routes of `handler` for every method: the `GraphQL` path and [`OTHER_PATH`].
fn routes<H, T>(handler: H) -> Router
where
    H: Handler<T, ()>,
    T: 'static,
{
    Router::new()
        .route(GRAPHQL_PATH, any(handler.clone()))
        .route(OTHER_PATH, any(handler))
}

/// The routes of `handler` behind the layers of the service router that write an answer of their
/// own, in the order of `assemble`: `vary_accept`, the panic catcher and the request budget.
fn app<H, T>(handler: H) -> Router
where
    H: Handler<T, ()>,
    T: 'static,
{
    routes(handler).layer(
        ServiceBuilder::new()
            .layer(from_fn(vary_accept))
            .layer(CatchPanicLayer::custom(panic_response))
            .layer(from_fn_with_state(BUDGET, request_timeout)),
    )
}

/// The router of [`app`] without `vary_accept`: the answers as they are without the middleware.
fn without_vary<H, T>(handler: H) -> Router
where
    H: Handler<T, ()>,
    T: 'static,
{
    routes(handler).layer(
        ServiceBuilder::new()
            .layer(CatchPanicLayer::custom(panic_response))
            .layer(from_fn_with_state(BUDGET, request_timeout)),
    )
}

async fn send(app: &Router, method: Method, uri: &str) -> Answer {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    tokio::time::timeout(LIMIT, async {
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
    .expect("the request answers in time")
}

/// Every `vary` value of `headers`, in order.
fn vary(headers: &HeaderMap) -> Vec<&str> {
    headers
        .get_all(VARY)
        .iter()
        .map(|value| value.to_str().unwrap())
        .collect()
}

/// The requests that `vary_accept` passes on: the other methods of the `GraphQL` path, every
/// method of another path, and the paths that only start like the `GraphQL` path.
fn passed_on() -> Vec<(Method, &'static str)> {
    vec![
        (Method::POST, GRAPHQL_PATH),
        (Method::PUT, GRAPHQL_PATH),
        (Method::PATCH, GRAPHQL_PATH),
        (Method::DELETE, GRAPHQL_PATH),
        (Method::OPTIONS, GRAPHQL_PATH),
        (Method::GET, OTHER_PATH),
        (Method::HEAD, OTHER_PATH),
        (Method::POST, OTHER_PATH),
        (Method::GET, "/api/graphql/"),
        (Method::GET, "/api/graphqlx"),
        (Method::HEAD, "/api/graphql/x"),
    ]
}

#[tokio::test]
async fn a_get_or_a_head_of_the_graphql_path_carries_vary_accept() {
    let app = app(async || "ok");

    for method in [Method::GET, Method::HEAD] {
        for uri in [GRAPHQL_PATH, GRAPHQL_PATH_WITH_QUERY] {
            let answer = send(&app, method.clone(), uri).await;

            assert_eq!(answer.status, StatusCode::OK, "{method} {uri}");
            assert_eq!(vary(&answer.headers), ["accept"], "{method} {uri}");
        }
    }
    assert_eq!(send(&app, Method::GET, GRAPHQL_PATH).await.body, "ok");
}

#[tokio::test]
async fn a_vary_accept_set_by_the_handler_is_not_repeated() {
    let app = app(async || ([(VARY, "accept")], "ok"));

    for method in [Method::GET, Method::HEAD] {
        let answer = send(&app, method.clone(), GRAPHQL_PATH).await;

        assert_eq!(answer.status, StatusCode::OK, "{method}");
        assert_eq!(vary(&answer.headers), ["accept"], "{method}");
    }
}

#[tokio::test]
async fn every_other_request_passes_unchanged() {
    let plain = async || "ok";
    let varying = async || ([(VARY, "origin")], "ok");

    for (method, uri) in passed_on() {
        let route = format!("{method} {uri}");
        let answer = send(&app(plain), method.clone(), uri).await;
        let reference = send(&without_vary(plain), method.clone(), uri).await;
        assert_eq!(vary(&answer.headers), Vec::<&str>::new(), "{route}");
        assert_eq!(answer.status, reference.status, "{route}");
        assert_eq!(answer.headers, reference.headers, "{route}");
        assert_eq!(answer.body, reference.body, "{route}");

        let answer = send(&app(varying), method.clone(), uri).await;
        let reference = send(&without_vary(varying), method.clone(), uri).await;
        assert_eq!(vary(&answer.headers), vary(&reference.headers), "{route}");
        assert_eq!(answer.status, reference.status, "{route}");
        assert_eq!(answer.headers, reference.headers, "{route}");
        assert_eq!(answer.body, reference.body, "{route}");
    }
    let routed = send(&app(varying), Method::POST, GRAPHQL_PATH).await;
    assert_eq!(routed.status, StatusCode::OK);
    assert_eq!(vary(&routed.headers), ["origin"]);
    let unrouted = send(&app(plain), Method::GET, "/api/graphqlx").await;
    assert_eq!(unrouted.status, StatusCode::NOT_FOUND);
}

#[tokio::test(start_paused = true)]
async fn the_503_of_the_request_budget_carries_vary_accept_on_a_get() {
    let app = app(async || {
        tokio::time::sleep(LATE).await;
        "late"
    });

    for method in [Method::GET, Method::HEAD] {
        let answer = send(&app, method.clone(), GRAPHQL_PATH).await;

        assert_eq!(answer.status, StatusCode::SERVICE_UNAVAILABLE, "{method}");
        assert_eq!(vary(&answer.headers), ["accept"], "{method}");
    }
    let get = send(&app, Method::GET, GRAPHQL_PATH).await;
    assert_eq!(
        serde_json::from_str::<Value>(&get.body).unwrap(),
        json!({
            "type": "about:blank",
            "title": "Service Unavailable",
            "status": 503,
            "detail": "The request exceeded its time budget.",
        })
    );
    let post = send(&app, Method::POST, GRAPHQL_PATH).await;
    assert_eq!(post.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(vary(&post.headers), Vec::<&str>::new());
}

#[tokio::test]
async fn the_500_of_the_panic_catcher_carries_vary_accept_on_a_get() {
    let app = app(async || -> &'static str { panic!("the handler fails") });

    for method in [Method::GET, Method::HEAD] {
        let answer = send(&app, method.clone(), GRAPHQL_PATH).await;

        assert_eq!(answer.status, StatusCode::INTERNAL_SERVER_ERROR, "{method}");
        assert_eq!(vary(&answer.headers), ["accept"], "{method}");
    }
    let get = send(&app, Method::GET, GRAPHQL_PATH).await;
    assert_eq!(
        get.body,
        r#"{"type":"about:blank","title":"Internal Server Error","status":500}"#
    );
    let post = send(&app, Method::POST, GRAPHQL_PATH).await;
    assert_eq!(post.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(vary(&post.headers), Vec::<&str>::new());
}
