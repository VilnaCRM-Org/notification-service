Feature: HTTP surface
  In order to rely on uniform behaviour
  As an API client
  I want liveness, error documents and response headers to follow one contract

  Scenario: Liveness
    When GET request is sent to "/ping"
    Then the response status code should be 200
    And the response body should contain "pong"

  Scenario: Unknown route
    When GET request is sent to "/api/unknown"
    Then the response status code should be 404
    And the response header "content-type" should contain "application/problem+json"

  Scenario: Unsupported method
    When DELETE request is sent to "/api/example_api_resources"
    Then the response status code should be 405
    And the response header "allow" should contain "POST"

  Scenario: Unsupported media type
    When POST request is sent to "/api/example_api_resources" with content type "text/plain" and body:
      """
      name=plain
      """
    Then the response status code should be 415
    And the response header "content-type" should contain "application/problem+json"

  Scenario: Malformed JSON
    When POST request is sent to "/api/example_api_resources" with body:
      """
      {not json
      """
    Then the response status code should be 400
    And the response header "content-type" should contain "application/problem+json"

  Scenario: Request id and security headers
    When GET request is sent to "/ping"
    Then the response header "x-request-id" should be present
    And the response header "x-content-type-options" should contain "nosniff"
    And the response header "x-frame-options" should contain "DENY"
    And the response header "referrer-policy" should contain "no-referrer"
    And the response header "content-security-policy" should contain "frame-ancestors 'none'"
