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
    And the response body should contain "/api/example_api_resources"
