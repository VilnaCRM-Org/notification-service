Feature: GraphQL operations
  In order to use the same use cases over GraphQL
  As an API client
  I want to query and create example API resources

  Scenario: Creating a resource with the mutation
    When the GraphQL operation is sent:
      """
      mutation { createExampleApiResource(input: {name: "Created by GraphQL", description: "Same command"}) { id name description } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the response body should contain "Created by GraphQL"

  Scenario: Reading a resource with the query
    Given an example API resource exists with name "Queried" and description "Same query"
    When the GraphQL operation is sent:
      """
      { exampleApiResource(id: "{id}") { id name description } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the response body should contain "Queried"

  Scenario: Listing resources with the query
    Given an example API resource exists with name "Listed by GraphQL" and description "First page"
    When the GraphQL operation is sent:
      """
      { exampleApiResources(itemsPerPage: 100) { id name } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the response body should contain "{id}"

  Scenario: Creating and reading through GraphQL
    When the GraphQL operation is sent:
      """
      mutation { createExampleApiResource(input: {name: "Round trip", description: "Mutation and both queries"}) { id name description } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    When the GraphQL operation is sent:
      """
      { exampleApiResource(id: "{id}") { id name description } }
      """
    Then the GraphQL response should have no errors
    And the response body should contain "Round trip"
    When the GraphQL operation is sent:
      """
      { exampleApiResources { id name } }
      """
    Then the GraphQL response should have no errors
    And the GraphQL field "exampleApiResources" should contain exactly the created resources

  Scenario: The explorer is disabled outside development
    When GET request is sent to "/api/graphql"
    Then the response status code should be 405
    And the response header "content-type" should contain "application/problem+json"
