Feature: Event stream status
  In order to know whether the event stream can serve my subscriptions
  As an operator
  I want to read the status of the event stream over REST and GraphQL

  Scenario: A service without a running reader reports that it is starting
    When GET request is sent to "/api/events/status"
    Then the response status code should be 200
    And the response header "content-type" should contain "application/json"
    And the JSON response should be:
      """
      {
        "state": "starting",
        "reason": "catching-up",
        "brokerReachable": false,
        "position": null,
        "endOffset": null,
        "lag": null,
        "windowFirst": null,
        "windowLast": null,
        "subscribers": 0,
        "rejectedRecords": "0",
        "retentionLosses": "0",
        "replayCapped": "0"
      }
      """

  Scenario: The GraphQL query returns the values of the status route
    When the GraphQL operation is sent:
      """
      { eventStreamStatus { state reason brokerReachable position endOffset lag windowFirst windowLast subscribers rejectedRecords retentionLosses replayCapped } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the JSON response should be:
      """
      {"data": {"eventStreamStatus": {"state": "starting", "reason": "catching-up", "brokerReachable": false, "position": null, "endOffset": null, "lag": null, "windowFirst": null, "windowLast": null, "subscribers": 0, "rejectedRecords": "0", "retentionLosses": "0", "replayCapped": "0"}}}
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
