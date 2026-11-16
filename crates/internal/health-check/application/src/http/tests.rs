use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, StatusCode};
use axum::response::Response;
use health_check_domain::{Dependency, HealthProbe};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde_json::{Value, json};
use test_support::probe::StubProbe;
use tower::ServiceExt as _;

use super::{HealthCheckState, router};
use crate::service::HealthService;

fn app(probes: Vec<StubProbe>) -> Router {
    let probes = probes
        .into_iter()
        .map(|probe| -> Arc<dyn HealthProbe> { Arc::new(probe) })
        .collect();
    let state = HealthCheckState {
        service: Arc::new(HealthService::new(probes, Duration::from_secs(3))),
    };
    let (router, _openapi) = router::<HealthCheckState>().split_for_parts();
    router.with_state(state)
}

/// Every dependency the service probes, in registration order.
const DEPENDENCIES: [Dependency; 3] = [
    Dependency::Cache,
    Dependency::MessageBroker,
    Dependency::EventStream,
];

fn probes(failing: &[Dependency]) -> Vec<StubProbe> {
    DEPENDENCIES
        .into_iter()
        .map(|dependency| {
            if failing.contains(&dependency) {
                StubProbe::failing(dependency)
            } else {
                StubProbe::healthy(dependency)
            }
        })
        .collect()
}

async fn get_health(app: Router) -> Response {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    app.oneshot(request).await.unwrap()
}

async fn body_bytes(response: Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

#[tokio::test]
async fn healthy_probes_answer_204_with_an_empty_body() {
    let response = get_health(app(probes(&[]))).await;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(body_bytes(response).await, Vec::<u8>::new());
}

#[tokio::test]
async fn failing_probes_answer_the_500_problem_that_names_each_failure() {
    let response = get_health(app(probes(&[
        Dependency::MessageBroker,
        Dependency::EventStream,
    ])))
    .await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    let bytes = body_bytes(response).await;
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(!text.contains("Cache is not working"));
    assert!(!text.contains("stub probe failure"));
    assert!(!text.contains("probe failed"));
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({
            "type": "/problems/dependency-unavailable",
            "title": "Internal Server Error",
            "status": 500,
            "detail": "Message broker is not available; Event stream is not ready",
            "failures": [
                {"dependency": "message_broker", "message": "Message broker is not available"},
                {"dependency": "event_stream", "message": "Event stream is not ready"},
            ],
        })
    );
}

#[rstest]
#[case::cache(&[Dependency::Cache])]
#[case::message_broker(&[Dependency::MessageBroker])]
#[case::event_stream(&[Dependency::EventStream])]
#[case::cache_and_message_broker(&[Dependency::Cache, Dependency::MessageBroker])]
#[case::cache_and_event_stream(&[Dependency::Cache, Dependency::EventStream])]
#[case::message_broker_and_event_stream(&[Dependency::MessageBroker, Dependency::EventStream])]
#[case::every_dependency(&DEPENDENCIES)]
#[tokio::test]
async fn the_500_body_holds_exactly_the_messages_of_the_failing_dependencies(
    #[case] failing: &[Dependency],
) {
    let response = get_health(app(probes(failing))).await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        response.headers().get(CONTENT_TYPE).unwrap(),
        "application/problem+json"
    );
    let text = String::from_utf8(body_bytes(response).await).unwrap();
    let detail = failing
        .iter()
        .map(|dependency| dependency.failure_message())
        .collect::<Vec<_>>()
        .join("; ");
    let failures: Vec<Value> = failing
        .iter()
        .map(|dependency| {
            json!({
                "dependency": dependency.identifier(),
                "message": dependency.failure_message(),
            })
        })
        .collect();
    assert_eq!(
        serde_json::from_str::<Value>(&text).unwrap(),
        json!({
            "type": "/problems/dependency-unavailable",
            "title": "Internal Server Error",
            "status": 500,
            "detail": detail,
            "failures": failures,
        })
    );
    for healthy in DEPENDENCIES
        .into_iter()
        .filter(|dependency| !failing.contains(dependency))
    {
        assert!(!text.contains(healthy.failure_message()), "{text}");
        assert!(!text.contains(healthy.identifier()), "{text}");
    }
    assert!(!text.contains("stub probe failure"), "{text}");
}

#[tokio::test(start_paused = true)]
async fn a_hanging_probe_answers_the_problem_without_the_timeout() {
    let probes = vec![
        StubProbe::healthy(Dependency::EventStream),
        StubProbe::hanging(Dependency::Cache),
        StubProbe::healthy(Dependency::MessageBroker),
    ];

    let response = get_health(app(probes)).await;

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let text = String::from_utf8(body_bytes(response).await).unwrap();
    assert!(text.contains(r#""detail":"Cache is not working""#));
    assert!(!text.contains("timed out"));
}

#[test]
fn the_route_documents_the_health_operation() {
    let (_router, openapi) = router::<HealthCheckState>().split_for_parts();

    let document = serde_json::to_value(&openapi).unwrap();
    let operation = &document["paths"]["/health"]["get"];
    assert_eq!(operation["operationId"], "api_health_get");
    assert_eq!(operation["tags"], json!(["HealthCheck"]));
    assert_eq!(operation["summary"], "Checks the health of the service.");
    assert_eq!(
        operation["description"],
        "Returns 204 when the cache and the message broker respond and the event stream of the node is ready. Returns a 500 problem document that lists every failing dependency otherwise."
    );
    let responses = operation["responses"].as_object().unwrap();
    assert_eq!(responses.len(), 3);
    assert_eq!(
        responses["204"]["description"],
        "Health check passed with no content to return."
    );
    assert!(responses["204"].get("content").is_none());
    assert_eq!(responses["500"]["description"], "Health check failed.");
    assert_eq!(responses["default"]["description"], "Problem response.");
    for status in ["500", "default"] {
        assert_eq!(
            responses[status]["content"]["application/problem+json"]["schema"]["$ref"],
            "#/components/schemas/Problem"
        );
    }
}
