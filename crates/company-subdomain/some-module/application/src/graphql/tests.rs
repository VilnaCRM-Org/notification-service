use std::sync::{Arc, Mutex};

use async_graphql::{EmptySubscription, Request, SDLExportOptions, Schema};
use rstest::rstest;
use serde_json::{Value, json};
use shared_domain::Uuid;
use test_support::bus::{FakeCommandBus, FakeQueryBus};
use test_support::id::SequenceUuidGenerator;

use super::{
    CreateExampleApiResourceInput, ExampleApiResourceMutation, ExampleApiResourceObject,
    ExampleApiResourceQuery,
};
use crate::command::create_example_api_resource::CreateExampleApiResource;
use crate::error::ExampleApiResourceError;
use crate::http::SomeModuleState;
use crate::query::ExampleApiResourceView;
use crate::query::get_example_api_resource::GetExampleApiResource;
use crate::query::list_example_api_resources::ListExampleApiResources;

const FIRST_COUNTER: u64 = 7;
const STORED: &str = "0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90";

type TestSchema = Schema<ExampleApiResourceQuery, ExampleApiResourceMutation, EmptySubscription>;

fn stored_id() -> Uuid {
    Uuid::parse(STORED).unwrap()
}

fn view(id: Uuid, name: &str) -> ExampleApiResourceView {
    ExampleApiResourceView {
        id,
        name: name.to_owned(),
        description: format!("{name} description"),
    }
}

fn state(command_bus: FakeCommandBus, query_bus: FakeQueryBus) -> SomeModuleState {
    SomeModuleState {
        command_bus: Arc::new(command_bus),
        query_bus: Arc::new(query_bus),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(FIRST_COUNTER)),
    }
}

fn schema(state: SomeModuleState) -> TestSchema {
    Schema::build(
        ExampleApiResourceQuery,
        ExampleApiResourceMutation,
        EmptySubscription,
    )
    .data(state)
    .finish()
}

async fn run(schema: &TestSchema, query: &str) -> Value {
    serde_json::to_value(schema.execute(Request::new(query)).await).unwrap()
}

fn lookup(found: Option<ExampleApiResourceView>) -> FakeQueryBus {
    FakeQueryBus::answering(move |_query: GetExampleApiResource| Ok(found.clone()))
}

fn idle_commands() -> FakeCommandBus {
    FakeCommandBus::handling(|_command: CreateExampleApiResource| Ok(()))
}

fn pages(page: Vec<ExampleApiResourceView>) -> FakeQueryBus {
    FakeQueryBus::answering(move |_query: ListExampleApiResources| Ok(page.clone()))
}

fn error_of(response: &Value) -> &Value {
    &response["errors"][0]
}

fn assert_validation_failed(response: &Value, violations: &Value) {
    assert_eq!(
        error_of(response)["message"],
        "The request failed validation."
    );
    assert_eq!(
        error_of(response)["extensions"]["code"],
        "VALIDATION_FAILED"
    );
    assert_eq!(error_of(response)["extensions"]["violations"], *violations);
}

fn assert_internal(response: &Value) {
    assert_eq!(error_of(response)["message"], "Internal server error");
    assert_eq!(
        error_of(response)["extensions"],
        json!({"code": "INTERNAL_SERVER_ERROR"})
    );
    assert_eq!(response["errors"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn the_mutation_creates_and_returns_the_object() {
    let seen: Arc<Mutex<Vec<CreateExampleApiResource>>> = Arc::default();
    let recorded = Arc::clone(&seen);
    let commands = FakeCommandBus::handling(move |command: CreateExampleApiResource| {
        recorded.lock().unwrap().push(command);
        Ok(())
    });
    let queries =
        FakeQueryBus::answering(|query: GetExampleApiResource| Ok(Some(view(query.id, "Created"))));

    let response = run(
        &schema(state(commands, queries)),
        r#"mutation { createExampleApiResource(input: {name: "Created", description: "Created description"}) { id name description } }"#,
    )
    .await;

    let id = SequenceUuidGenerator::starting_at(FIRST_COUNTER);
    let id = shared_domain::UuidGenerator::generate(&id).to_string();
    assert_eq!(
        response,
        json!({"data": {"createExampleApiResource": {
            "id": id, "name": "Created", "description": "Created description"
        }}})
    );
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].id.to_string(), id);
    assert_eq!(seen[0].name, "Created");
    assert_eq!(seen[0].description, "Created description");
}

#[tokio::test]
async fn the_query_returns_a_stored_object() {
    let queries = FakeQueryBus::answering(|query: GetExampleApiResource| {
        Ok((query.id == stored_id()).then(|| view(query.id, "Stored")))
    });

    let response = run(
        &schema(state(idle_commands(), queries)),
        &format!(r#"{{ exampleApiResource(id: "{STORED}") {{ id name description }} }}"#),
    )
    .await;

    assert_eq!(
        response,
        json!({"data": {"exampleApiResource": {
            "id": STORED, "name": "Stored", "description": "Stored description"
        }}})
    );
}

#[tokio::test]
async fn the_query_returns_null_without_an_error_for_an_unknown_id() {
    let response = run(
        &schema(state(idle_commands(), lookup(None))),
        &format!(r#"{{ exampleApiResource(id: "{STORED}") {{ id }} }}"#),
    )
    .await;

    assert_eq!(response, json!({"data": {"exampleApiResource": null}}));
}

#[tokio::test]
async fn the_list_returns_the_page_newest_first() {
    let newest = Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f92").unwrap();
    let oldest = Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f91").unwrap();
    let asked: Arc<Mutex<Vec<(u32, u32)>>> = Arc::default();
    let recorded = Arc::clone(&asked);
    let page = vec![view(newest, "Newest"), view(oldest, "Oldest")];
    let queries = FakeQueryBus::answering(move |query: ListExampleApiResources| {
        recorded
            .lock()
            .unwrap()
            .push((query.page, query.items_per_page));
        Ok(page.clone())
    });

    let response = run(
        &schema(state(idle_commands(), queries)),
        "{ exampleApiResources(page: 2, itemsPerPage: 5) { id name } }",
    )
    .await;

    assert_eq!(
        response,
        json!({"data": {"exampleApiResources": [
            {"id": newest.to_string(), "name": "Newest"},
            {"id": oldest.to_string(), "name": "Oldest"}
        ]}})
    );
    assert_eq!(*asked.lock().unwrap(), vec![(2, 5)]);
}

#[rstest]
#[case::defaults("{ exampleApiResources { id } }", (1, 30))]
#[case::smallest("{ exampleApiResources(page: 1, itemsPerPage: 1) { id } }", (1, 1))]
#[case::largest_page_size("{ exampleApiResources(itemsPerPage: 100) { id } }", (1, 100))]
#[tokio::test]
async fn the_list_uses_the_defaults_of_the_contract(
    #[case] query: &str,
    #[case] expected: (u32, u32),
) {
    let asked: Arc<Mutex<Vec<(u32, u32)>>> = Arc::default();
    let recorded = Arc::clone(&asked);
    let queries = FakeQueryBus::answering(move |query: ListExampleApiResources| {
        recorded
            .lock()
            .unwrap()
            .push((query.page, query.items_per_page));
        Ok(Vec::new())
    });

    let response = run(&schema(state(idle_commands(), queries)), query).await;

    assert_eq!(response, json!({"data": {"exampleApiResources": []}}));
    assert_eq!(*asked.lock().unwrap(), vec![expected]);
}

#[rstest]
#[case::negative_page(
    "{ exampleApiResources(page: -1) { id } }",
    "page",
    "This value must be 1 or greater."
)]
#[case::zero_page(
    "{ exampleApiResources(page: 0) { id } }",
    "page",
    "This value must be 1 or greater."
)]
#[case::too_many_items(
    "{ exampleApiResources(itemsPerPage: 101) { id } }",
    "itemsPerPage",
    "This value must be between 1 and 100."
)]
#[case::no_items(
    "{ exampleApiResources(itemsPerPage: 0) { id } }",
    "itemsPerPage",
    "This value must be between 1 and 100."
)]
#[tokio::test]
async fn a_page_argument_outside_its_bounds_is_a_validation_failure(
    #[case] query: &str,
    #[case] property: &str,
    #[case] message: &str,
) {
    let response = run(&schema(state(idle_commands(), pages(Vec::new()))), query).await;

    assert_validation_failed(
        &response,
        &json!([{"propertyPath": property, "message": message, "code": "range"}]),
    );
    assert_eq!(response["data"], Value::Null);
}

#[tokio::test]
async fn an_id_that_is_not_a_uuid_is_a_validation_failure() {
    let queries = lookup(Some(view(stored_id(), "Stored")));
    let bus = Arc::new(queries);
    let state = SomeModuleState {
        command_bus: Arc::new(idle_commands()),
        query_bus: Arc::<FakeQueryBus>::clone(&bus),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(FIRST_COUNTER)),
    };

    let response = run(
        &schema(state),
        r#"{ exampleApiResource(id: "not-a-uuid") { id } }"#,
    )
    .await;

    assert_validation_failed(
        &response,
        &json!([{
            "propertyPath": "id",
            "message": "This value is not a valid UUID.",
            "code": "uuid"
        }]),
    );
    assert_eq!(bus.asked(), Vec::<&str>::new());
}

#[tokio::test]
async fn an_input_with_an_empty_name_is_a_validation_failure_and_dispatches_nothing() {
    let commands = Arc::new(idle_commands());
    let state = SomeModuleState {
        command_bus: Arc::<FakeCommandBus>::clone(&commands),
        query_bus: Arc::new(lookup(None)),
        uuid_generator: Arc::new(SequenceUuidGenerator::starting_at(FIRST_COUNTER)),
    };

    let response = run(
        &schema(state),
        r#"mutation { createExampleApiResource(input: {name: "", description: "d"}) { id } }"#,
    )
    .await;

    assert_validation_failed(
        &response,
        &json!([{
            "propertyPath": "name",
            "message": "This value must contain between 1 and 255 characters.",
            "code": "length"
        }]),
    );
    assert_eq!(commands.dispatched(), Vec::<&str>::new());
}

#[tokio::test]
async fn an_input_with_a_nul_character_reports_every_violation_sorted() {
    let response = run(
        &schema(state(idle_commands(), lookup(None))),
        r#"mutation { createExampleApiResource(input: {name: "a\u0000", description: ""}) { id } }"#,
    )
    .await;

    let violations = &error_of(&response)["extensions"]["violations"];
    let codes: Vec<(&str, &str)> = violations
        .as_array()
        .unwrap()
        .iter()
        .map(|violation| {
            (
                violation["propertyPath"].as_str().unwrap(),
                violation["code"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        codes,
        vec![("description", "length"), ("name", "does_not_contain")]
    );
}

#[tokio::test]
async fn a_storage_failure_of_a_query_is_an_internal_error_without_detail() {
    let queries = FakeQueryBus::answering(|_query: GetExampleApiResource| {
        Err(ExampleApiResourceError::Storage("database down".into()))
    });

    let response = run(
        &schema(state(idle_commands(), queries)),
        &format!(r#"{{ exampleApiResource(id: "{STORED}") {{ id }} }}"#),
    )
    .await;

    assert_internal(&response);
    assert!(!response.to_string().contains("database down"));
}

#[tokio::test]
async fn a_failing_list_query_is_an_internal_error() {
    let response = run(
        &schema(state(idle_commands(), FakeQueryBus::unregistered())),
        "{ exampleApiResources { id } }",
    )
    .await;

    assert_internal(&response);
}

#[tokio::test]
async fn a_failing_command_is_an_internal_error() {
    let commands = FakeCommandBus::unregistered();

    let response = run(
        &schema(state(commands, lookup(None))),
        r#"mutation { createExampleApiResource(input: {name: "n", description: "d"}) { id } }"#,
    )
    .await;

    assert_internal(&response);
}

#[tokio::test]
async fn a_resource_that_cannot_be_read_back_is_an_internal_error() {
    let response = run(
        &schema(state(idle_commands(), lookup(None))),
        r#"mutation { createExampleApiResource(input: {name: "n", description: "d"}) { id } }"#,
    )
    .await;

    assert_internal(&response);
}

#[tokio::test]
async fn a_schema_without_the_state_answers_an_internal_error() {
    let schema = Schema::build(
        ExampleApiResourceQuery,
        ExampleApiResourceMutation,
        EmptySubscription,
    )
    .finish();

    let query = run(&schema, "{ exampleApiResources { id } }").await;
    let mutation = run(
        &schema,
        r#"mutation { createExampleApiResource(input: {name: "n", description: "d"}) { id } }"#,
    )
    .await;
    let single = run(
        &schema,
        &format!(r#"{{ exampleApiResource(id: "{STORED}") {{ id }} }}"#),
    )
    .await;

    assert_internal(&query);
    assert_internal(&mutation);
    assert_internal(&single);
}

#[tokio::test]
async fn an_int_outside_32_bits_is_rejected_by_the_library() {
    let response = run(
        &schema(state(idle_commands(), pages(Vec::new()))),
        "{ exampleApiResources(page: 4294967296) { id } }",
    )
    .await;

    assert!(error_of(&response).get("extensions").is_none());
}

#[test]
fn the_types_are_documented_in_the_sdl() {
    let sdl = schema(state(idle_commands(), lookup(None))).sdl_with_options(
        SDLExportOptions::new()
            .sorted_fields()
            .sorted_arguments()
            .prefer_single_line_descriptions()
            .use_space_ident()
            .indent_width(2),
    );

    for line in [
        "\"Template example resource.\"\ntype ExampleApiResource {",
        "\"Input of the create mutation.\"\ninput CreateExampleApiResourceInput {",
        "\"Retrieves an example API resource by id.\"",
        "\"Retrieves one page of example API resources, newest first.\"",
        "\"Creates an example API resource.\"",
        "\"Page number, from 1.\"",
        "\"Number of items per page, from 1 to 100.\"",
    ] {
        assert!(sdl.contains(line), "the SDL lacks {line}:\n{sdl}");
    }
}

#[test]
fn the_types_can_be_printed_for_diagnostics() {
    let input = CreateExampleApiResourceInput {
        name: "n".to_owned(),
        description: "d".to_owned(),
    };
    let object = ExampleApiResourceObject::from(view(stored_id(), "Stored"));

    assert_eq!(
        format!("{input:?}"),
        r#"CreateExampleApiResourceInput { name: "n", description: "d" }"#
    );
    assert!(format!("{object:?}").starts_with("ExampleApiResourceObject { id: ID("));
    assert_eq!(
        format!("{ExampleApiResourceQuery:?}"),
        "ExampleApiResourceQuery"
    );
    assert_eq!(
        format!("{ExampleApiResourceMutation:?}"),
        "ExampleApiResourceMutation"
    );
}

#[test]
fn the_input_is_read_from_a_graphql_object_and_rejects_other_values() {
    use async_graphql::{InputType as _, Name, Value};

    let object = Value::Object(
        [
            (Name::new("name"), Value::String("n".to_owned())),
            (Name::new("description"), Value::String("d".to_owned())),
        ]
        .into_iter()
        .collect(),
    );

    let parsed = CreateExampleApiResourceInput::parse(Some(object)).unwrap();

    assert_eq!(
        (parsed.name.as_str(), parsed.description.as_str()),
        ("n", "d")
    );
    assert!(CreateExampleApiResourceInput::parse(Some(Value::Boolean(true))).is_err());
    let wrong_type = Value::Object(
        [
            (Name::new("name"), Value::Boolean(true)),
            (Name::new("description"), Value::String("d".to_owned())),
        ]
        .into_iter()
        .collect(),
    );
    assert!(CreateExampleApiResourceInput::parse(Some(wrong_type)).is_err());
    let wrong_description = Value::Object(
        [
            (Name::new("name"), Value::String("n".to_owned())),
            (Name::new("description"), Value::Boolean(true)),
        ]
        .into_iter()
        .collect(),
    );
    assert!(CreateExampleApiResourceInput::parse(Some(wrong_description)).is_err());
}
