Feature: Example API resource
  In order to learn the patterns of the template
  As an API client
  I want to create, read and list example API resources

  Scenario: Creating a resource
    When POST request is sent to "/api/example_api_resources" with body:
      """
      {"name": "Created by a scenario", "description": "Stored through the command path"}
      """
    Then the response status code should be 201
    And the response header "location" should contain "/api/example_api_resources/"
    And the JSON field "name" should equal "Created by a scenario"
    And the JSON field "description" should equal "Stored through the command path"

  Scenario: Reading a resource by id
    Given an example API resource exists with name "Readable" and description "Read through the query path"
    When GET request is sent to "/api/example_api_resources/{id}"
    Then the response status code should be 200
    And the JSON field "name" should equal "Readable"

  Scenario: Listing resources
    Given an example API resource exists with name "First listed" and description "Created first"
    And an example API resource exists with name "Second listed" and description "Created second"
    When GET request is sent to "/api/example_api_resources"
    Then the response status code should be 200
    And the JSON array should contain exactly the created resources

  Scenario: Rejecting invalid input
    When POST request is sent to "/api/example_api_resources" with body:
      """
      {"name": "", "description": ""}
      """
    Then the response status code should be 422
    And the response body should contain "/problems/validation-failed"
    And the response body should contain "This value must contain between 1 and 255 characters."
    And the response body should contain "This value must contain between 1 and 1024 characters."

  Scenario: Reading an unknown id
    When GET request is sent to "/api/example_api_resources/00000000-0000-7000-8000-000000000000"
    Then the response status code should be 404
    And the response body should contain "was not found"

  Scenario: Reading a malformed id
    When GET request is sent to "/api/example_api_resources/not-a-uuid"
    Then the response status code should be 400
    And the response header "content-type" should contain "application/problem+json"

  Scenario: Requesting a page size above the maximum
    When GET request is sent to "/api/example_api_resources?itemsPerPage=101"
    Then the response status code should be 422
    And the response body should contain "itemsPerPage"
