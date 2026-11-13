use async_graphql::Request;

use super::*;

const EXPECTED_SDL: &str = r#""Input of the create mutation."
input CreateExampleApiResourceInput {
  "Description."
  description: String!
  "Name."
  name: String!
}

"Template example resource."
type ExampleApiResource {
  "Description."
  description: String!
  "Identifier."
  id: ID!
  "Name."
  name: String!
}

"Mutation root."
type Mutation {
  "Creates an example API resource."
  createExampleApiResource(input: CreateExampleApiResourceInput!): ExampleApiResource!
}

"Query root."
type Query {
  "Retrieves an example API resource by id."
  exampleApiResource(id: ID!): ExampleApiResource
  "Retrieves one page of example API resources, newest first."
  exampleApiResources(
    "Number of items per page, from 1 to 100."
    itemsPerPage: Int! = 30,
    "Page number, from 1."
    page: Int! = 1
  ): [ExampleApiResource!]!
}

"Directs the executor to include this field or fragment only when the `if` argument is true."
directive @include(if: Boolean!) on FIELD | FRAGMENT_SPREAD | INLINE_FRAGMENT
"Directs the executor to skip this field or fragment when the `if` argument is true."
directive @skip(if: Boolean!) on FIELD | FRAGMENT_SPREAD | INLINE_FRAGMENT
schema {
  query: Query
  mutation: Mutation
}
"#;

#[test]
fn the_sdl_is_the_documented_schema() {
    assert_eq!(sdl(), EXPECTED_SDL);
}

#[test]
fn the_sdl_has_the_three_operations_and_nothing_else_to_call() {
    let text = sdl();

    assert!(text.contains("exampleApiResource(id: ID!)"));
    assert!(text.contains("exampleApiResources("));
    assert!(text.contains("createExampleApiResource(input: CreateExampleApiResourceInput!)"));
    for absent in [
        "node",
        "Connection",
        "update",
        "delete",
        "type Subscription",
    ] {
        assert!(!text.contains(absent), "the SDL contains `{absent}`");
    }
}

#[test]
fn twenty_generations_are_byte_identical() {
    let first = sdl();

    for _ in 0..20 {
        assert_eq!(sdl(), first);
    }
}

#[tokio::test]
async fn the_builder_gives_a_schema_with_the_two_roots() {
    let schema: ApiSchema = builder().finish();

    let response = schema.execute(Request::new("{ query: __typename }")).await;
    let mutation = schema
        .execute(Request::new("mutation { mutation: __typename }"))
        .await;

    assert_eq!(
        response.data.into_json().unwrap(),
        serde_json::json!({ "query": "Query" })
    );
    assert_eq!(
        mutation.data.into_json().unwrap(),
        serde_json::json!({ "mutation": "Mutation" })
    );
}
