use std::any::TypeId;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use axum::Router;
use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, LOCATION};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use http_body_util::BodyExt as _;
use rstest::rstest;
use serde_json::{Value, json};
use shared_domain::{
    CommandBus, CommandHandler as _, ErasedBusError, ErasedMessage, EventBus, QueryBus,
    QueryHandler as _, Uuid, UuidGenerator as _,
};
use some_module_domain::{ExampleApiResourceId, ExampleApiResourceRepository};
use test_support::bus::{FakeCommandBus, FakeQueryBus, RecordingEventBus};
use test_support::clock::FixedClock;
use test_support::id::SequenceUuidGenerator;
use test_support::repository::InMemoryExampleApiResourceRepository;
use tower::ServiceExt as _;

use crate::command::create_example_api_resource::{
    CreateExampleApiResource, CreateExampleApiResourceHandler,
};
use crate::error::ExampleApiResourceError;
use crate::http::{SomeModuleState, router};
use crate::query::ExampleApiResourceView;
use crate::query::get_example_api_resource::{GetExampleApiResource, GetExampleApiResourceHandler};
use crate::query::list_example_api_resources::ListExampleApiResources;

const FIRST_COUNTER: u64 = 7;
const PROBLEM: &str = "application/problem+json";
const CREATE_BODY: &str = r#"{"name":"Name","description":"Description"}"#;

fn generated_id() -> Uuid {
    SequenceUuidGenerator::starting_at(FIRST_COUNTER).generate()
}

fn stored_id() -> Uuid {
    Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90").unwrap()
}

fn view(id: Uuid, name: &str) -> ExampleApiResourceView {
    ExampleApiResourceView {
        id,
        name: name.to_owned(),
        description: format!("{name} description"),
    }
}

fn accepting_commands() -> Arc<FakeCommandBus> {
    Arc::new(FakeCommandBus::handling(
        |_command: CreateExampleApiResource| Ok(()),
    ))
}

fn state(command_bus: &Arc<FakeCommandBus>, query_bus: &Arc<FakeQueryBus>) -> SomeModuleState {
    SomeModuleState {
        command_bus: Arc::<FakeCommandBus>::clone(command_bus),
        query_bus: Arc::<FakeQueryBus>::clone(query_bus),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(FIRST_COUNTER)),
    }
}

fn app(state: SomeModuleState) -> Router {
    let (routes, _document) = router::<SomeModuleState>().split_for_parts();
    routes.with_state(state)
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Reply {
    fn content_type(&self) -> &str {
        self.headers[CONTENT_TYPE].to_str().unwrap()
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

async fn send(state: SomeModuleState, request: Request<Body>) -> Reply {
    let response = app(state).oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    Reply {
        status: parts.status,
        headers: parts.headers,
        body: body.collect().await.unwrap().to_bytes().to_vec(),
    }
}

fn post(body: &str) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/example_api_resources")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn assert_problem(reply: &Reply, status: StatusCode, problem_type: &str) -> Value {
    assert_eq!(reply.status, status);
    assert_eq!(reply.content_type(), PROBLEM);
    let body = reply.json();
    assert_eq!(body["type"], problem_type);
    assert_eq!(body["status"], status.as_u16());
    body
}

// ---- create ----

type Seen = Arc<Mutex<Vec<CreateExampleApiResource>>>;

fn recording_commands(seen: &Seen) -> Arc<FakeCommandBus> {
    let seen = Arc::clone(seen);
    Arc::new(FakeCommandBus::handling(
        move |command: CreateExampleApiResource| {
            seen.lock().unwrap().push(command);
            Ok(())
        },
    ))
}

fn reading_back() -> Arc<FakeQueryBus> {
    Arc::new(FakeQueryBus::answering(|query: GetExampleApiResource| {
        Ok(Some(view(query.id, "Created")))
    }))
}

#[tokio::test]
async fn create_answers_201_with_the_resource_and_its_location() {
    let seen = Seen::default();
    let commands = recording_commands(&seen);
    let queries = reading_back();

    let reply = send(
        state(&commands, &queries),
        post(r#"{"name":"Created","description":"Created description"}"#),
    )
    .await;

    let id = generated_id().to_string();
    assert_eq!(reply.status, StatusCode::CREATED);
    assert_eq!(reply.content_type(), "application/json");
    assert_eq!(
        reply.json(),
        json!({"id": id, "name": "Created", "description": "Created description"})
    );
    assert_eq!(
        reply.headers[LOCATION],
        format!("/api/example_api_resources/{id}").as_str()
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].id.to_string(), id);
    assert_eq!(seen[0].name, "Created");
    assert_eq!(seen[0].description, "Created description");
    assert_eq!(queries.asked(), vec!["GetExampleApiResource"]);
}

/// Dispatches a create command to the real handler, as the in-memory bus does.
#[derive(Debug)]
struct HandlerCommandBus(CreateExampleApiResourceHandler);

#[async_trait::async_trait]
impl CommandBus for HandlerCommandBus {
    async fn dispatch_erased(
        &self,
        _command_type: TypeId,
        name: &'static str,
        command: ErasedMessage,
    ) -> Result<(), ErasedBusError> {
        let command = command
            .downcast::<CreateExampleApiResource>()
            .map_err(|_other| ErasedBusError::NotRegistered { message: name })?;
        self.0
            .handle(*command)
            .await
            .map_err(|error| ErasedBusError::Handler(Box::new(error)))
    }
}

/// Asks the real get handler, as the in-memory bus does.
#[derive(Debug)]
struct HandlerQueryBus(GetExampleApiResourceHandler);

#[async_trait::async_trait]
impl QueryBus for HandlerQueryBus {
    async fn ask_erased(
        &self,
        _query_type: TypeId,
        name: &'static str,
        query: ErasedMessage,
    ) -> Result<ErasedMessage, ErasedBusError> {
        let query = query
            .downcast::<GetExampleApiResource>()
            .map_err(|_other| ErasedBusError::NotRegistered { message: name })?;
        match self.0.handle(*query).await {
            Ok(output) => Ok(Box::new(output)),
            Err(error) => Err(ErasedBusError::Handler(Box::new(error))),
        }
    }
}

#[tokio::test]
async fn create_answers_201_when_the_event_bus_failed_after_the_save() {
    let repository = Arc::new(InMemoryExampleApiResourceRepository::new());
    let events = Arc::new(RecordingEventBus::failing());
    let repository_port: Arc<dyn ExampleApiResourceRepository> =
        Arc::<InMemoryExampleApiResourceRepository>::clone(&repository);
    let events_port: Arc<dyn EventBus> = Arc::<RecordingEventBus>::clone(&events);
    let handler = CreateExampleApiResourceHandler::new(
        Arc::<dyn ExampleApiResourceRepository>::clone(&repository_port),
        events_port,
        Arc::new(FixedClock::at(SystemTime::UNIX_EPOCH)),
        Arc::new(SequenceUuidGenerator::starting_at(900)),
    );
    let state = SomeModuleState {
        command_bus: Arc::new(HandlerCommandBus(handler)),
        query_bus: Arc::new(HandlerQueryBus(GetExampleApiResourceHandler::new(
            repository_port,
        ))),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(FIRST_COUNTER)),
    };

    let reply = send(
        state,
        post(r#"{"name":"Stored","description":"Despite the bus"}"#),
    )
    .await;

    let id = generated_id().to_string();
    assert_eq!(reply.status, StatusCode::CREATED);
    assert_eq!(
        reply.json(),
        json!({"id": id, "name": "Stored", "description": "Despite the bus"})
    );
    assert_eq!(events.published().len(), 1);
}

#[tokio::test]
async fn create_answers_the_internal_problem_when_the_command_fails() {
    let commands = Arc::new(FakeCommandBus::handling(
        |command: CreateExampleApiResource| {
            Err(ExampleApiResourceError::AlreadyExists(
                ExampleApiResourceId::new(command.id),
            ))
        },
    ));
    let queries = reading_back();

    let reply = send(state(&commands, &queries), post(CREATE_BODY)).await;

    let body = assert_problem(&reply, StatusCode::INTERNAL_SERVER_ERROR, "about:blank");
    assert!(body.get("detail").is_none());
    assert_eq!(queries.asked(), Vec::<&str>::new());
}

#[tokio::test]
async fn create_answers_the_internal_problem_without_a_registered_handler() {
    let commands = Arc::new(FakeCommandBus::unregistered());
    let queries = reading_back();

    let reply = send(state(&commands, &queries), post(CREATE_BODY)).await;

    assert_problem(&reply, StatusCode::INTERNAL_SERVER_ERROR, "about:blank");
}

#[tokio::test]
async fn create_answers_the_internal_problem_when_the_resource_cannot_be_read_back() {
    let commands = accepting_commands();
    let queries = Arc::new(FakeQueryBus::answering(|_query: GetExampleApiResource| {
        Ok(None)
    }));

    let reply = send(state(&commands, &queries), post(CREATE_BODY)).await;

    let body = assert_problem(&reply, StatusCode::INTERNAL_SERVER_ERROR, "about:blank");
    assert!(body.get("detail").is_none());
}

#[tokio::test]
async fn create_answers_the_internal_problem_when_the_read_back_fails() {
    let commands = accepting_commands();
    let queries = Arc::new(FakeQueryBus::unregistered());

    let reply = send(state(&commands, &queries), post(CREATE_BODY)).await;

    assert_problem(&reply, StatusCode::INTERNAL_SERVER_ERROR, "about:blank");
}

#[tokio::test]
async fn create_reports_every_violation_sorted_by_property_path() {
    let commands = accepting_commands();
    let queries = reading_back();

    let reply = send(
        state(&commands, &queries),
        post(r#"{"name":"","description":""}"#),
    )
    .await;

    let body = assert_problem(
        &reply,
        StatusCode::UNPROCESSABLE_ENTITY,
        "/problems/validation-failed",
    );
    assert_eq!(body["detail"], "The request failed validation.");
    assert_eq!(
        body["violations"],
        json!([
            {
                "propertyPath": "description",
                "message": "This value must contain between 1 and 1024 characters.",
                "code": "length"
            },
            {
                "propertyPath": "name",
                "message": "This value must contain between 1 and 255 characters.",
                "code": "length"
            }
        ])
    );
    assert_eq!(commands.dispatched(), Vec::<&str>::new());
}

#[rstest]
#[case::unknown_field(r#"{"name":"n","description":"d","extra":1}"#)]
#[case::missing_field(r#"{"name":"n"}"#)]
#[case::wrong_type(r#"{"name":1,"description":"d"}"#)]
#[tokio::test]
async fn create_answers_the_blank_422_for_a_body_that_does_not_deserialize(#[case] body: &str) {
    let commands = accepting_commands();
    let queries = reading_back();

    let reply = send(state(&commands, &queries), post(body)).await;

    assert_problem(&reply, StatusCode::UNPROCESSABLE_ENTITY, "about:blank");
    assert_eq!(commands.dispatched(), Vec::<&str>::new());
}

#[rstest]
#[case::in_the_name(r#"{"name":"a\u0000b","description":"d"}"#, "name")]
#[case::in_the_description(r#"{"name":"n","description":"\u0000"}"#, "description")]
#[tokio::test]
async fn create_rejects_the_nul_character(#[case] body: &str, #[case] property: &str) {
    let commands = accepting_commands();
    let queries = reading_back();

    let reply = send(state(&commands, &queries), post(body)).await;

    let problem = assert_problem(
        &reply,
        StatusCode::UNPROCESSABLE_ENTITY,
        "/problems/validation-failed",
    );
    assert_eq!(
        problem["violations"],
        json!([{
            "propertyPath": property,
            "message": "This value must not contain the NUL character.",
            "code": "does_not_contain"
        }])
    );
    assert_eq!(commands.dispatched(), Vec::<&str>::new());
}

#[tokio::test]
async fn create_answers_400_for_malformed_json_and_415_for_another_media_type() {
    let commands = accepting_commands();
    let queries = reading_back();

    let malformed = send(state(&commands, &queries), post("{")).await;
    let other_type = send(
        state(&commands, &queries),
        Request::builder()
            .method(Method::POST)
            .uri("/example_api_resources")
            .header(CONTENT_TYPE, "text/plain")
            .body(Body::from(CREATE_BODY))
            .unwrap(),
    )
    .await;

    assert_problem(&malformed, StatusCode::BAD_REQUEST, "about:blank");
    assert_problem(
        &other_type,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "about:blank",
    );
    assert_eq!(commands.dispatched(), Vec::<&str>::new());
}

// ---- get ----

fn finding(found: Option<ExampleApiResourceView>) -> Arc<FakeQueryBus> {
    Arc::new(FakeQueryBus::answering(
        move |_query: GetExampleApiResource| Ok(found.clone()),
    ))
}

#[tokio::test]
async fn get_answers_200_with_the_stored_resource() {
    let queries = Arc::new(FakeQueryBus::answering(|query: GetExampleApiResource| {
        Ok((query.id == stored_id()).then(|| view(query.id, "Stored")))
    }));

    let reply = send(
        state(&accepting_commands(), &queries),
        get(&format!("/example_api_resources/{}", stored_id())),
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.content_type(), "application/json");
    assert_eq!(
        reply.json(),
        json!({
            "id": stored_id().to_string(),
            "name": "Stored",
            "description": "Stored description"
        })
    );
}

#[tokio::test]
async fn get_answers_the_404_problem_for_an_unknown_id() {
    let id = stored_id();

    let reply = send(
        state(&accepting_commands(), &finding(None)),
        get(&format!("/example_api_resources/{id}")),
    )
    .await;

    let body = assert_problem(&reply, StatusCode::NOT_FOUND, "about:blank");
    assert_eq!(
        body["detail"],
        format!("ExampleApiResource {id} was not found.")
    );
}

#[tokio::test]
async fn get_answers_400_for_an_id_that_is_not_a_uuid() {
    let queries = finding(Some(view(stored_id(), "Stored")));

    let reply = send(
        state(&accepting_commands(), &queries),
        get("/example_api_resources/not-a-uuid"),
    )
    .await;

    assert_problem(&reply, StatusCode::BAD_REQUEST, "about:blank");
    assert_eq!(queries.asked(), Vec::<&str>::new());
}

#[tokio::test]
async fn get_answers_the_internal_problem_when_the_query_fails() {
    let queries = Arc::new(FakeQueryBus::answering(|_query: GetExampleApiResource| {
        Err(ExampleApiResourceError::Storage("database down".into()))
    }));

    let reply = send(
        state(&accepting_commands(), &queries),
        get(&format!("/example_api_resources/{}", stored_id())),
    )
    .await;

    let body = assert_problem(&reply, StatusCode::INTERNAL_SERVER_ERROR, "about:blank");
    assert!(body.get("detail").is_none());
    assert!(
        !String::from_utf8(reply.body)
            .unwrap()
            .contains("database down")
    );
}

// ---- list ----

type Asked = Arc<Mutex<Vec<(u32, u32)>>>;

fn listing(asked: &Asked, page: Vec<ExampleApiResourceView>) -> Arc<FakeQueryBus> {
    let asked = Arc::clone(asked);
    Arc::new(FakeQueryBus::answering(
        move |query: ListExampleApiResources| {
            asked
                .lock()
                .unwrap()
                .push((query.page, query.items_per_page));
            Ok(page.clone())
        },
    ))
}

#[tokio::test]
async fn list_answers_the_page_in_the_order_of_the_query() {
    let asked = Asked::default();
    let newest = Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f92").unwrap();
    let oldest = Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f91").unwrap();
    let queries = listing(&asked, vec![view(newest, "Newest"), view(oldest, "Oldest")]);

    let reply = send(
        state(&accepting_commands(), &queries),
        get("/example_api_resources?page=2&itemsPerPage=5"),
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.content_type(), "application/json");
    assert_eq!(
        reply.json(),
        json!([
            {"id": newest.to_string(), "name": "Newest", "description": "Newest description"},
            {"id": oldest.to_string(), "name": "Oldest", "description": "Oldest description"}
        ])
    );
    assert_eq!(*asked.lock().unwrap(), vec![(2, 5)]);
}

#[rstest]
#[case::no_query_string("/example_api_resources", (1, 30))]
#[case::unknown_parameters_are_ignored("/example_api_resources?color=red", (1, 30))]
#[case::only_the_page("/example_api_resources?page=3", (3, 30))]
#[case::only_the_page_size("/example_api_resources?itemsPerPage=40", (1, 40))]
#[case::smallest_values("/example_api_resources?page=1&itemsPerPage=1", (1, 1))]
#[case::largest_page_size("/example_api_resources?itemsPerPage=100", (1, 100))]
#[case::largest_page("/example_api_resources?page=2147483647", (2_147_483_647, 30))]
#[tokio::test]
async fn list_passes_the_parameters_with_their_defaults(
    #[case] uri: &str,
    #[case] expected: (u32, u32),
) {
    let asked = Asked::default();
    let queries = listing(&asked, Vec::new());

    let reply = send(state(&accepting_commands(), &queries), get(uri)).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(*asked.lock().unwrap(), vec![expected]);
}

#[tokio::test]
async fn list_answers_an_empty_array_for_a_page_beyond_the_last() {
    let asked = Asked::default();
    let queries = listing(&asked, Vec::new());

    let reply = send(
        state(&accepting_commands(), &queries),
        get("/example_api_resources?page=1000"),
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body, b"[]");
}

#[rstest]
#[case::page_zero("page=0", "page", "This value must be 1 or greater.")]
#[case::page_negative("page=-1", "page", "This value must be 1 or greater.")]
#[case::no_items(
    "itemsPerPage=0",
    "itemsPerPage",
    "This value must be between 1 and 100."
)]
#[case::too_many_items(
    "itemsPerPage=101",
    "itemsPerPage",
    "This value must be between 1 and 100."
)]
#[tokio::test]
async fn list_answers_422_with_a_range_violation(
    #[case] query: &str,
    #[case] property: &str,
    #[case] message: &str,
) {
    let asked = Asked::default();
    let queries = listing(&asked, Vec::new());

    let reply = send(
        state(&accepting_commands(), &queries),
        get(&format!("/example_api_resources?{query}")),
    )
    .await;

    let body = assert_problem(
        &reply,
        StatusCode::UNPROCESSABLE_ENTITY,
        "/problems/validation-failed",
    );
    assert_eq!(
        body["violations"],
        json!([{"propertyPath": property, "message": message, "code": "range"}])
    );
    assert_eq!(*asked.lock().unwrap(), Vec::<(u32, u32)>::new());
}

#[tokio::test]
async fn list_reports_both_parameters_sorted_by_property_path() {
    let queries = listing(&Asked::default(), Vec::new());

    let reply = send(
        state(&accepting_commands(), &queries),
        get("/example_api_resources?page=0&itemsPerPage=0"),
    )
    .await;

    let body = assert_problem(
        &reply,
        StatusCode::UNPROCESSABLE_ENTITY,
        "/problems/validation-failed",
    );
    let paths: Vec<&str> = body["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|violation| violation["propertyPath"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["itemsPerPage", "page"]);
}

#[rstest]
#[case::text("page=abc")]
#[case::fraction("page=1.5")]
#[case::beyond_32_bits("page=4294967296")]
#[case::items_text("itemsPerPage=many")]
#[tokio::test]
async fn list_answers_400_for_a_value_that_is_not_a_32_bit_integer(#[case] query: &str) {
    let asked = Asked::default();
    let queries = listing(&asked, Vec::new());

    let reply = send(
        state(&accepting_commands(), &queries),
        get(&format!("/example_api_resources?{query}")),
    )
    .await;

    assert_problem(&reply, StatusCode::BAD_REQUEST, "about:blank");
    assert_eq!(*asked.lock().unwrap(), Vec::<(u32, u32)>::new());
}

#[tokio::test]
async fn list_answers_the_internal_problem_when_the_query_fails() {
    let queries = Arc::new(FakeQueryBus::unregistered());

    let reply = send(
        state(&accepting_commands(), &queries),
        get("/example_api_resources"),
    )
    .await;

    assert_problem(&reply, StatusCode::INTERNAL_SERVER_ERROR, "about:blank");
}

// ---- routing and documentation ----

#[tokio::test]
async fn the_router_serves_only_the_registered_methods_and_paths() {
    let commands = accepting_commands();
    let queries = reading_back();
    let state = state(&commands, &queries);

    let wrong_method = send(
        state.clone(),
        Request::builder()
            .method(Method::DELETE)
            .uri("/example_api_resources")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let unknown = send(state, get("/example_api_resources/a/b")).await;

    assert_eq!(wrong_method.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}

fn document() -> Value {
    serde_json::to_value(router::<SomeModuleState>().into_openapi()).unwrap()
}

fn problem_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": {"application/problem+json": {"schema": {"$ref": "#/components/schemas/Problem"}}}
    })
}

fn default_problem() -> Value {
    problem_response("Problem response.")
}

fn keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

#[test]
fn the_document_has_the_three_operations_and_nothing_else() {
    let document = document();

    let paths = &document["paths"];
    assert_eq!(
        keys(paths),
        vec!["/example_api_resources", "/example_api_resources/{id}"]
    );
    assert_eq!(keys(&paths["/example_api_resources"]), vec!["get", "post"]);
    assert_eq!(keys(&paths["/example_api_resources/{id}"]), vec!["get"]);
}

#[test]
fn the_list_operation_matches_the_contract() {
    let operation = &document()["paths"]["/example_api_resources"]["get"];

    assert_eq!(operation["operationId"], "api_example_api_resources_get");
    assert_eq!(operation["tags"], json!(["ExampleApiResource"]));
    assert_eq!(
        operation["summary"],
        "Retrieves the collection of ExampleApiResource resources."
    );
    assert_eq!(
        operation["description"],
        "Returns one page of resources, newest first."
    );
    assert_eq!(
        operation["parameters"],
        json!([
            {
                "name": "page",
                "in": "query",
                "description": "Page number, starting at 1.",
                "required": false,
                "schema": {"type": "integer", "format": "int32", "default": 1, "minimum": 1}
            },
            {
                "name": "itemsPerPage",
                "in": "query",
                "description": "Number of items per page.",
                "required": false,
                "schema": {
                    "type": "integer", "format": "int32", "default": 30, "minimum": 1, "maximum": 100
                }
            }
        ])
    );
    assert_eq!(
        operation["responses"],
        json!({
            "200": {
                "description": "ExampleApiResource collection",
                "content": {"application/json": {"schema": {
                    "type": "array",
                    "items": {"$ref": "#/components/schemas/ExampleApiResource"}
                }}}
            },
            "400": problem_response("Invalid query parameter."),
            "422": problem_response("Validation failed."),
            "default": default_problem()
        })
    );
    assert!(operation.get("requestBody").is_none());
}

#[test]
fn the_create_operation_matches_the_contract() {
    let operation = &document()["paths"]["/example_api_resources"]["post"];

    assert_eq!(operation["operationId"], "api_example_api_resources_post");
    assert_eq!(operation["tags"], json!(["ExampleApiResource"]));
    assert_eq!(
        operation["summary"],
        "Creates an ExampleApiResource resource."
    );
    assert_eq!(
        operation["description"],
        "Validates the input, stores the resource and returns it."
    );
    assert_eq!(
        operation["requestBody"],
        json!({
            "content": {"application/json": {"schema": {
                "$ref": "#/components/schemas/CreateExampleApiResourceRequest"
            }}},
            "required": true
        })
    );
    assert!(operation.get("parameters").is_none());
    assert_eq!(
        operation["responses"],
        json!({
            "201": {
                "description": "ExampleApiResource resource created",
                "headers": {"location": {
                    "description": "Path of the created resource.",
                    "schema": {"type": "string"}
                }},
                "content": {"application/json": {"schema": {
                    "$ref": "#/components/schemas/ExampleApiResource"
                }}}
            },
            "400": problem_response("Invalid input."),
            "413": problem_response("Request body too large."),
            "415": problem_response("Unsupported media type."),
            "422": problem_response("Validation failed."),
            "default": default_problem()
        })
    );
}

#[test]
fn the_get_operation_matches_the_contract() {
    let operation = &document()["paths"]["/example_api_resources/{id}"]["get"];

    assert_eq!(operation["operationId"], "api_example_api_resources_id_get");
    assert_eq!(operation["tags"], json!(["ExampleApiResource"]));
    assert_eq!(
        operation["summary"],
        "Retrieves an ExampleApiResource resource."
    );
    assert_eq!(
        operation["description"],
        "Returns the resource identified by id."
    );
    assert_eq!(
        operation["parameters"],
        json!([{
            "name": "id",
            "in": "path",
            "description": "ExampleApiResource identifier",
            "required": true,
            "schema": {"type": "string", "format": "uuid"}
        }])
    );
    assert_eq!(
        operation["responses"],
        json!({
            "200": {
                "description": "ExampleApiResource resource",
                "content": {"application/json": {"schema": {
                    "$ref": "#/components/schemas/ExampleApiResource"
                }}}
            },
            "400": problem_response("Invalid identifier."),
            "404": problem_response("Resource not found."),
            "default": default_problem()
        })
    );
}

#[test]
fn the_document_carries_the_schemas_of_the_operations() {
    let document = document();

    let schemas = &document["components"]["schemas"];
    for name in [
        "CreateExampleApiResourceRequest",
        "ExampleApiResource",
        "Problem",
        "Violation",
    ] {
        assert!(schemas.get(name).is_some(), "{name} is missing");
    }
}

/// The extractor types a handler of this module may take.
const ALLOWED_EXTRACTORS: [&str; 4] = [
    "State<SomeModuleState>",
    "ApiPath<",
    "ValidatedQuery<",
    "ValidatedJson<",
];

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
    assert_eq!(names, ["create", "get_one", "list"]);
    assert!(source.contains("ApiPath<uuid::Uuid>"));
    assert!(source.contains("ValidatedQuery<ListExampleApiResourcesParams>"));
    assert!(source.contains("ValidatedJson<CreateExampleApiResourceRequest>"));
}

#[rstest]
#[case::qualified_json("Json(b): axum::Json<T>")]
#[case::fully_qualified_json("axum::extract::Json(b): axum::extract::Json<T>")]
#[case::bare_json("Json(b): Json<T>")]
#[case::query("Query(p): Query<T>")]
#[case::path("Path(id): Path<uuid::Uuid>")]
#[case::form("Form(b): Form<T>")]
#[case::raw_query("RawQuery(q): RawQuery")]
#[case::raw_body("body: String")]
fn the_extractor_check_reports_a_planted_violation(#[case] parameter: &str) {
    let source = format!(
        "pub async fn ok(State(state): State<SomeModuleState>, {parameter}) -> Result<(), Problem> {{}}"
    );

    assert_eq!(disallowed_parameters(&source), [format!("ok: {parameter}")]);
}

#[test]
fn the_extractor_check_accepts_the_allowed_extractors() {
    let source = "pub async fn ok(State(state): State<SomeModuleState>, \
        ApiPath(id): ApiPath<uuid::Uuid>, ValidatedQuery(q): ValidatedQuery<Q>, \
        ValidatedJson(b): ValidatedJson<B>) -> Result<(), Problem> {}";

    assert_eq!(disallowed_parameters(source), Vec::<String>::new());
}
