Feature: Event stream status
  In order to know whether the event stream can serve my subscriptions
  As an operator
  I want to read the status of the event stream over REST and GraphQL

  Scenario: A ready node reports its state and a reachable broker
    When GET request is sent to "/api/events/status"
    Then the response status code should be 200
    And the response header "content-type" should contain "application/json"
    And the JSON field "state" should equal "ready"
    And the response body should contain '"reason":null'
    And the response body should contain '"brokerReachable":true'
    And the response body should contain '"subscribers":0'

  Scenario: The GraphQL query returns the values of the status route
    When the GraphQL operation is sent:
      """
      { eventStreamStatus { state reason brokerReachable subscribers } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the JSON response should be:
      """
      {"data": {"eventStreamStatus": {"state": "ready", "reason": null, "brokerReachable": true, "subscribers": 0}}}
      """

  Scenario: The GraphQL catalog query returns the page of the REST operation
    Given the event types "order.paid,user.created,user.deleted" are configured
    When the GraphQL operation is sent:
      """
      { eventTypes(page: 2, itemsPerPage: 1) { name } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the JSON response should be:
      """
      {"data": {"eventTypes": [{"name": "user.created"}]}}
      """

  Scenario: The GraphQL catalog query without arguments returns the first page of 30
    Given the event types "order.paid,user.created,user.deleted" are configured
    When the GraphQL operation is sent:
      """
      { eventTypes { name } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the JSON response should be:
      """
      {"data": {"eventTypes": [{"name": "order.paid"}, {"name": "user.created"}, {"name": "user.deleted"}]}}
      """

  Scenario: A GraphQL catalog page size above 100 is the validation error
    When the GraphQL operation is sent:
      """
      { eventTypes(itemsPerPage: 101) { name } }
      """
    Then the response status code should be 200
    And the response body should contain "VALIDATION_FAILED"
    And the response body should contain "itemsPerPage must be between 1 and 100"
