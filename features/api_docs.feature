Feature: API documentation
  In order to explore the API
  As a developer
  I want the service to serve its own documentation

  Scenario: The documentation page is served
    When GET request is sent to "/api/docs"
    Then the response status code should be 200
    And the response header "content-type" should contain "text/html"
    And the response body should contain "swagger-ui"

  Scenario: The OpenAPI document is served
    When GET request is sent to "/api/docs/openapi.json"
    Then the response status code should be 200
    And the response body should contain "3.1.0"
    And the response body should contain "/api/event-types"

  Scenario: The OpenAPI document holds every operation the service serves
    When GET request is sent to "/api/docs/openapi.json"
    Then the response status code should be 200
    And the OpenAPI document should hold exactly these operations:
      | method | path                   | operationId               | tag           |
      | GET    | /api/docs              | api_docs_get              | Documentation |
      | GET    | /api/docs/openapi.json | api_docs_openapi_json_get | Documentation |
      | GET    | /api/docs/{file}       | api_docs_file_get         | Documentation |
      | GET    | /api/event-types       | api_event_types_get       | EventStream   |
      | GET    | /api/events/status     | api_events_status_get     | EventStream   |
      | GET    | /api/events/stream     | api_events_stream_get     | EventStream   |
      | GET    | /api/events/ws         | api_events_ws_get         | EventStream   |
      | GET    | /api/graphql           | api_graphql_get           | GraphQL       |
      | POST   | /api/graphql           | api_graphql_post          | GraphQL       |
      | GET    | /api/health            | api_health_get            | HealthCheck   |
      | GET    | /ping                  | ping_get                  | HealthCheck   |
