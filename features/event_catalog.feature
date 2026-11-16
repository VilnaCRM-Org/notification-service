Feature: Event catalog
  In order to know which events I can subscribe to
  As a frontend developer
  I want to read the catalog of subscribable event types

  Scenario: An unset catalog is empty
    When GET request is sent to "/api/event-types"
    Then the response status code should be 200
    And the JSON response should be:
      """
      []
      """

  Scenario: The catalog lists each configured name once in byte order
    Given the event types "user.created,order.paid,order-paid,order_paid,order.paid" are configured
    When GET request is sent to "/api/event-types"
    Then the response status code should be 200
    And the response header "content-type" should contain "application/json"
    And the JSON response should be:
      """
      [{"name": "order-paid"}, {"name": "order.paid"}, {"name": "order_paid"}, {"name": "user.created"}]
      """

  Scenario: The catalog is paged
    Given the event types "order.paid,user.created,user.deleted" are configured
    When GET request is sent to "/api/event-types?itemsPerPage=1&page=2"
    Then the response status code should be 200
    And the JSON response should be:
      """
      [{"name": "user.created"}]
      """

  Scenario: A page beyond the last is empty
    Given the event types "order.paid,user.created,user.deleted" are configured
    When GET request is sent to "/api/event-types?page=4&itemsPerPage=1"
    Then the response status code should be 200
    And the JSON response should be:
      """
      []
      """

  Scenario: A page parameter that is not a number is a bad request
    When GET request is sent to "/api/event-types?page=abc"
    Then the response status code should be 400
    And the response header "content-type" should contain "application/problem+json"
    And the response body should contain "about:blank"
    And the response body should contain "Failed to deserialize query string: page: invalid digit found in string"

  Scenario Outline: A page parameter outside its bounds fails validation
    When GET request is sent to "/api/event-types?<query>"
    Then the response status code should be 422
    And the response body should contain "/problems/validation-failed"
    And the response body should contain "<message>"

    Examples:
      | query            | message                                |
      | page=0           | page must be 1 or greater              |
      | itemsPerPage=0   | itemsPerPage must be between 1 and 100 |
      | itemsPerPage=101 | itemsPerPage must be between 1 and 100 |
