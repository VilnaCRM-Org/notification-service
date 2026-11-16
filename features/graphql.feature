Feature: GraphQL operations
  In order to explore the GraphQL schema of the event stream safely
  As an API client
  I want the explorer to be served only in development

  Scenario: The explorer is disabled outside development
    When GET request is sent to "/api/graphql"
    Then the response status code should be 405
    And the response header "content-type" should contain "application/problem+json"
