use std::any::TypeId;
use std::collections::BTreeSet;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Method, Request, StatusCode};
use event_stream_domain::{Cursor, EventType};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde_json::{Value, json};
use shared_domain::{ErasedBusError, ErasedMessage, QueryBus, QueryHandler as _};
use test_support::bus::FakeQueryBus;
use test_support::fixture::stored_event;
use tokio::sync::watch;
use tower::ServiceExt as _;

use crate::catalog::EventCatalog;
use crate::http::hub::{Hub, HubSettings};
use crate::http::{EventStreamState, router};
use crate::query::list_event_types::{ListEventTypes, ListEventTypesHandler};
use crate::status::StatusSource;
use crate::tail::{TailState, TopicFault};
use crate::window::Window;

const PROBLEM: &str = "application/problem+json";

/// Asks the real handler, as the in-memory bus does.
#[derive(Debug)]
struct HandlerQueryBus(ListEventTypesHandler);

#[async_trait::async_trait]
impl QueryBus for HandlerQueryBus {
    async fn ask_erased(
        &self,
        _query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError> {
        let query = query
            .downcast::<ListEventTypes>()
            .map_err(|_other| ErasedBusError::NotRegistered { message: name })?;
        match self.0.handle(*query).await {
            Ok(output) => Ok(Box::new(output)),
            Err(error) => Err(ErasedBusError::Handler(Box::new(error))),
        }
    }
}

const SETTINGS: HubSettings = HubSettings {
    max_subscribers: 4,
    buffer: 2,
};

/// A state whose status reads `tail`, `window` and `hub`.
fn state_with(
    query_bus: Arc<dyn QueryBus>,
    tail: TailState,
    window: Arc<Window>,
    hub: Arc<Hub>,
) -> EventStreamState {
    let (_sender, receiver) = watch::channel(tail);
    EventStreamState {
        query_bus,
        status: Arc::new(StatusSource::new(receiver, window, Arc::<Hub>::clone(&hub))),
        hub,
    }
}

/// A state of a node that has not started, over `query_bus`.
fn starting(query_bus: Arc<dyn QueryBus>) -> EventStreamState {
    state_with(
        query_bus,
        TailState::default(),
        Arc::new(Window::new(10, usize::MAX)),
        Hub::new(SETTINGS),
    )
}

fn state(names: &[&str]) -> EventStreamState {
    let types = names
        .iter()
        .map(|name| EventType::new((*name).to_owned()).unwrap())
        .collect::<BTreeSet<_>>();
    starting(Arc::new(HandlerQueryBus(ListEventTypesHandler::new(
        Arc::new(EventCatalog::new(types)),
    ))))
}

fn three() -> EventStreamState {
    state(&["user.deleted", "order.paid", "user.created"])
}

fn app(state: EventStreamState) -> Router {
    let (routes, _document) = router::<EventStreamState>().split_for_parts();
    routes.with_state(state)
}

struct Reply {
    status: StatusCode,
    content_type: String,
    body: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

async fn send_request(state: EventStreamState, request: Request<Body>) -> Reply {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let response = app(state).oneshot(request).await.unwrap();
        let (parts, body) = response.into_parts();
        let content_type = parts
            .headers
            .get(CONTENT_TYPE)
            .map(|value| value.to_str().unwrap().to_owned())
            .unwrap_or_default();
        Reply {
            status: parts.status,
            content_type,
            body: body.collect().await.unwrap().to_bytes().to_vec(),
        }
    })
    .await
    .expect("the request answers in time")
}

async fn send(state: EventStreamState, uri: &str) -> Reply {
    send_request(
        state,
        Request::builder().uri(uri).body(Body::empty()).unwrap(),
    )
    .await
}

#[tokio::test]
async fn an_empty_catalog_answers_an_empty_array() {
    let reply = send(state(&[]), "/event-types").await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.content_type, "application/json");
    assert_eq!(reply.body, b"[]");
}

#[tokio::test]
async fn the_catalog_answers_each_name_once_in_byte_order() {
    let reply = send(
        state(&[
            "user.created",
            "order.paid",
            "order-paid",
            "order_paid",
            "order.paid",
        ]),
        "/event-types",
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.json(),
        json!([
            {"name": "order-paid"},
            {"name": "order.paid"},
            {"name": "order_paid"},
            {"name": "user.created"}
        ])
    );
}

#[rstest]
#[case::second_of_one("itemsPerPage=1&page=2", json!([{"name": "user.created"}]))]
#[case::beyond_the_last("page=4&itemsPerPage=1", json!([]))]
#[case::smallest_size("itemsPerPage=1", json!([{"name": "order.paid"}]))]
#[case::largest_size(
    "itemsPerPage=100",
    json!([{"name": "order.paid"}, {"name": "user.created"}, {"name": "user.deleted"}])
)]
#[case::unknown_parameters_are_ignored(
    "color=red",
    json!([{"name": "order.paid"}, {"name": "user.created"}, {"name": "user.deleted"}])
)]
#[tokio::test]
async fn the_page_parameters_select_the_page(#[case] query: &str, #[case] expected: Value) {
    let reply = send(three(), &format!("/event-types?{query}")).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json(), expected);
}

#[rstest]
#[case::page_zero("page=0", "page", "page must be 1 or greater")]
#[case::page_negative("page=-1", "page", "page must be 1 or greater")]
#[case::no_items(
    "itemsPerPage=0",
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
#[case::negative_items(
    "itemsPerPage=-3",
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
#[case::too_many_items(
    "itemsPerPage=101",
    "itemsPerPage",
    "itemsPerPage must be between 1 and 100"
)]
#[case::both_reports_the_page("page=0&itemsPerPage=0", "page", "page must be 1 or greater")]
#[tokio::test]
async fn a_page_outside_its_bounds_is_422_with_one_range_violation(
    #[case] query: &str,
    #[case] property: &str,
    #[case] message: &str,
) {
    let reply = send(three(), &format!("/event-types?{query}")).await;

    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.content_type, PROBLEM);
    let body = reply.json();
    assert_eq!(body["type"], "/problems/validation-failed");
    assert_eq!(body["status"], 422);
    assert_eq!(
        body["violations"],
        json!([{"propertyPath": property, "message": message, "code": "range"}])
    );
}

#[rstest]
#[case::text("page=abc", "page: invalid digit found in string")]
#[case::fraction("itemsPerPage=1.5", "itemsPerPage: invalid digit found in string")]
#[case::beyond_32_bits("page=4294967296", "page: number too large to fit in target type")]
#[tokio::test]
async fn a_value_that_is_not_a_32_bit_integer_is_400_with_the_parser_text(
    #[case] query: &str,
    #[case] parser_text: &str,
) {
    let queries = Arc::new(FakeQueryBus::unregistered());
    let state = starting(Arc::<FakeQueryBus>::clone(&queries));

    let reply = send(state, &format!("/event-types?{query}")).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.content_type, PROBLEM);
    let body = reply.json();
    assert_eq!(body["type"], "about:blank");
    assert_eq!(
        body["detail"],
        format!("Failed to deserialize query string: {parser_text}")
    );
    assert_eq!(queries.asked(), Vec::<&str>::new());
}

#[tokio::test]
async fn the_handler_asks_the_list_query_once() {
    let queries = Arc::new(FakeQueryBus::answering(|query: ListEventTypes| {
        assert_eq!((query.page, query.items_per_page), (3, 7));
        Ok(Vec::new())
    }));
    let state = starting(Arc::<FakeQueryBus>::clone(&queries));

    let reply = send(state, "/event-types?page=3&itemsPerPage=7").await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(queries.asked(), vec!["ListEventTypes"]);
}

#[tokio::test]
async fn the_router_serves_only_the_registered_method_and_path() {
    let wrong_method = send_request(
        three(),
        Request::builder()
            .method(Method::POST)
            .uri("/event-types")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let unknown = send(three(), "/event-types/a").await;

    assert_eq!(wrong_method.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

// ---- status ----

const STATUS_FIELDS: [&str; 12] = [
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
    "windowLast",
];

fn idle_bus() -> Arc<dyn QueryBus> {
    Arc::new(FakeQueryBus::unregistered())
}

fn field_names(body: &Value) -> Vec<String> {
    let mut names: Vec<String> = body.as_object().unwrap().keys().cloned().collect();
    names.sort_unstable();
    names
}

#[tokio::test]
async fn a_starting_node_answers_200_with_the_nulls_of_an_unknown_position() {
    let queries = Arc::new(FakeQueryBus::unregistered());

    let reply = send(
        starting(Arc::<FakeQueryBus>::clone(&queries)),
        "/events/status",
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.content_type, "application/json");
    assert_eq!(
        reply.json(),
        json!({
            "state": "starting",
            "reason": "catching-up",
            "brokerReachable": false,
            "position": null,
            "endOffset": null,
            "lag": null,
            "windowFirst": null,
            "windowLast": null,
            "subscribers": 0,
            "rejectedRecords": "0",
            "retentionLosses": "0",
            "replayCapped": "0"
        })
    );
    assert_eq!(queries.asked(), Vec::<&str>::new());
}

#[tokio::test]
async fn a_ready_node_answers_its_position_window_and_subscribers() {
    let window = Arc::new(Window::new(10, usize::MAX));
    window.restart(Cursor::new(30));
    window.push(Arc::new(stored_event(35, "user.created", "{}")));
    let hub = Hub::new(SETTINGS);
    hub.start(Cursor::new(35));
    let types = BTreeSet::from([EventType::new("user.created".to_owned()).unwrap()]);
    let _first = hub.register(&types).unwrap();
    let _second = hub.register(&types).unwrap();
    let tail = TailState {
        started: true,
        broker_reachable: true,
        position: Some(Cursor::new(u64::MAX)),
        end: Some(Cursor::new(u64::MAX)),
        rejected: 3,
        retention_losses: 2,
        ..TailState::default()
    };

    let reply = send(state_with(idle_bus(), tail, window, hub), "/events/status").await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(
        reply.json(),
        json!({
            "state": "ready",
            "reason": null,
            "brokerReachable": true,
            "position": "18446744073709551615",
            "endOffset": "18446744073709551615",
            "lag": "0",
            "windowFirst": "30",
            "windowLast": "35",
            "subscribers": 2,
            "rejectedRecords": "3",
            "retentionLosses": "2",
            "replayCapped": "0"
        })
    );
}

#[rstest]
#[case::unreachable(Some(TopicFault::Unreachable), "broker-unreachable")]
#[case::missing(Some(TopicFault::Missing), "topic-not-found")]
#[case::partitions(Some(TopicFault::Partitions(2)), "partition-count")]
#[case::catching_up(None, "catching-up")]
#[tokio::test]
async fn a_starting_node_names_its_reason(#[case] fault: Option<TopicFault>, #[case] reason: &str) {
    let tail = TailState {
        fault,
        ..TailState::default()
    };
    let state = state_with(
        idle_bus(),
        tail,
        Arc::new(Window::new(10, usize::MAX)),
        Hub::new(SETTINGS),
    );

    let reply = send(state, "/events/status").await;

    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.json();
    assert_eq!(body["state"], "starting");
    assert_eq!(body["reason"], reason);
    assert_eq!(field_names(&body), STATUS_FIELDS);
}

#[tokio::test]
async fn a_closed_hub_answers_200_shutting_down_and_not_a_503() {
    let hub = Hub::new(SETTINGS);
    hub.close();
    let tail = TailState {
        started: true,
        ..TailState::default()
    };
    let state = state_with(idle_bus(), tail, Arc::new(Window::new(10, usize::MAX)), hub);

    let reply = send(state, "/events/status").await;

    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.json();
    assert_eq!(body["state"], "shutting-down");
    assert_eq!(body["reason"], Value::Null);
    assert_eq!(field_names(&body), STATUS_FIELDS);
}

#[tokio::test]
async fn the_status_reads_the_tail_state_at_each_request() {
    let (sender, receiver) = watch::channel(TailState::default());
    let hub = Hub::new(SETTINGS);
    let state = EventStreamState {
        query_bus: idle_bus(),
        status: Arc::new(StatusSource::new(
            receiver,
            Arc::new(Window::new(10, usize::MAX)),
            Arc::<Hub>::clone(&hub),
        )),
        hub,
    };

    let before = send(state.clone(), "/events/status").await;
    sender.send_replace(TailState {
        started: true,
        ..TailState::default()
    });
    let after = send(state, "/events/status").await;

    assert_eq!(before.json()["state"], "starting");
    assert_eq!(after.json()["state"], "ready");
}

#[tokio::test]
async fn the_status_route_serves_only_get() {
    let reply = send_request(
        three(),
        Request::builder()
            .method(Method::POST)
            .uri("/events/status")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(reply.status, StatusCode::METHOD_NOT_ALLOWED);
}

// ---- documentation ----

fn document() -> Value {
    serde_json::to_value(router::<EventStreamState>().into_openapi()).unwrap()
}

fn problem_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": {"application/problem+json": {"schema": {"$ref": "#/components/schemas/Problem"}}}
    })
}

#[test]
fn the_document_has_the_catalog_and_the_status_operations_only() {
    let document = document();

    let paths = document["paths"].as_object().unwrap();
    let mut names: Vec<&str> = paths.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, ["/event-types", "/events/status"]);
    for path in names {
        assert_eq!(
            paths[path].as_object().unwrap().keys().collect::<Vec<_>>(),
            ["get"],
            "{path}"
        );
    }
}

#[test]
fn the_status_operation_matches_the_contract() {
    let document = document();

    let operation = &document["paths"]["/events/status"]["get"];
    assert_eq!(operation["operationId"], "api_events_status_get");
    assert_eq!(operation["tags"], json!(["EventStream"]));
    assert_eq!(operation["summary"], "Retrieves the event stream status.");
    assert!(
        operation["description"]
            .as_str()
            .unwrap()
            .contains("answers in every life stage")
    );
    assert!(operation.get("parameters").is_none());
    let responses = &operation["responses"];
    assert_eq!(
        responses["200"]["description"],
        "EventStreamStatus resource"
    );
    assert_eq!(
        responses["200"]["content"]["application/json"]["schema"],
        json!({"$ref": "#/components/schemas/EventStreamStatus"})
    );
    assert_eq!(responses["default"], problem_response("Problem response."));
    assert_eq!(
        responses.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["200", "default"]
    );
}

#[test]
fn the_catalog_operation_matches_the_contract() {
    let document = document();

    let operation = &document["paths"]["/event-types"]["get"];
    assert_eq!(operation["operationId"], "api_event_types_get");
    assert_eq!(operation["tags"], json!(["EventStream"]));
    assert_eq!(
        operation["summary"],
        "Retrieves the collection of EventType resources."
    );
    let parameters = operation["parameters"].as_array().unwrap();
    let names: Vec<&str> = parameters
        .iter()
        .map(|parameter| parameter["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["page", "itemsPerPage"]);
    assert_eq!(parameters[0]["in"], "query");
    assert_eq!(parameters[0]["schema"]["minimum"], 1);
    assert_eq!(parameters[0]["schema"]["default"], 1);
    assert_eq!(parameters[1]["schema"]["minimum"], 1);
    assert_eq!(parameters[1]["schema"]["maximum"], 100);
    assert_eq!(parameters[1]["schema"]["default"], 30);
    let responses = &operation["responses"];
    assert_eq!(
        responses["200"]["content"]["application/json"]["schema"],
        json!({"type": "array", "items": {"$ref": "#/components/schemas/EventType"}})
    );
    assert_eq!(
        responses["400"],
        problem_response("Invalid query parameter.")
    );
    assert_eq!(responses["422"], problem_response("Validation failed."));
    assert_eq!(responses["default"], problem_response("Problem response."));
    assert_eq!(
        responses.as_object().unwrap().keys().collect::<Vec<_>>(),
        ["200", "400", "422", "default"]
    );
}

#[test]
fn the_document_carries_the_event_type_schema() {
    let document = document();

    let schemas = &document["components"]["schemas"];
    for name in ["EventStreamStatus", "EventType", "Problem"] {
        assert!(schemas.get(name).is_some(), "{name} is missing");
    }
}

// ---- extractor check ----

/// The extractor types a handler of this module may take.
const ALLOWED_EXTRACTORS: [&str; 2] = ["State<EventStreamState>", "ValidatedQuery<"];

/// Splits `text` at every `separator` that is not nested in brackets.
fn split_top_level(text: &str, separator: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0_usize;
    let mut skip = 0_usize;
    for (index, character) in text.char_indices() {
        if skip > 0 {
            skip = skip.saturating_sub(1);
            continue;
        }
        let at_separator = depth == 0
            && text
                .get(index..)
                .is_some_and(|rest| rest.starts_with(separator));
        if at_separator {
            parts.push(std::mem::take(&mut current));
            skip = separator.chars().count().saturating_sub(1);
            continue;
        }
        match character {
            '(' | '[' | '<' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '>' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        current.push(character);
    }
    parts.push(current);
    parts
}

/// Returns the name and the parameter list of every `pub async fn` of `source`.
fn handler_parameters(source: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for chunk in source.split("pub async fn ").skip(1) {
        let (name, after) = chunk
            .split_once('(')
            .expect("a handler has a parameter list");
        let mut depth = 1_usize;
        let mut parameters = String::new();
        for character in after.chars() {
            match character {
                '(' => depth = depth.saturating_add(1),
                ')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            if depth == 0 {
                break;
            }
            parameters.push(character);
        }
        found.push((name.trim().to_owned(), parameters));
    }
    found
}

/// Returns every handler parameter whose type is not an allowed extractor.
fn disallowed_parameters(source: &str) -> Vec<String> {
    let mut disallowed = Vec::new();
    for (name, parameters) in handler_parameters(source) {
        for parameter in split_top_level(&parameters, ",") {
            let parameter = parameter.trim();
            if parameter.is_empty() {
                continue;
            }
            let parts = split_top_level(parameter, ": ");
            let kind = parts.iter().skip(1).cloned().collect::<Vec<_>>().join(": ");
            if !ALLOWED_EXTRACTORS
                .iter()
                .any(|allowed| kind.trim().starts_with(allowed))
            {
                disallowed.push(format!("{name}: {parameter}"));
            }
        }
    }
    disallowed
}

#[test]
fn handlers_use_only_the_problem_aware_extractors() {
    let source = include_str!("../handler.rs");

    assert_eq!(disallowed_parameters(source), Vec::<String>::new());
    let names: Vec<String> = handler_parameters(source)
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, ["list_event_types", "event_stream_status"]);
    assert!(source.contains("ValidatedQuery<ListEventTypesParams>"));
}

#[rstest]
#[case::qualified_json("Json(b): axum::Json<T>")]
#[case::bare_json("Json(b): Json<T>")]
#[case::query("Query(p): Query<T>")]
#[case::path("Path(id): Path<String>")]
#[case::raw_query("RawQuery(q): RawQuery")]
#[case::raw_body("body: String")]
#[case::other_state("State(state): State<OtherState>")]
fn the_extractor_check_reports_a_planted_violation(#[case] parameter: &str) {
    let source = format!(
        "pub async fn ok(State(state): State<EventStreamState>, {parameter}) -> Result<(), Problem> {{}}"
    );

    assert_eq!(disallowed_parameters(&source), [format!("ok: {parameter}")]);
}

#[test]
fn the_extractor_check_accepts_the_allowed_extractors() {
    let source = "pub async fn ok(State(state): State<EventStreamState>, \
        ValidatedQuery(q): ValidatedQuery<Q>) -> Result<(), Problem> {}";

    assert_eq!(disallowed_parameters(source), Vec::<String>::new());
}
