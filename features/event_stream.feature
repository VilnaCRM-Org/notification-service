Feature: Event stream
  In order to show live notifications without polling
  As a frontend developer
  I want to subscribe to the events of the topic over SSE and resume after a disconnect

  Scenario: The catalog lists the configured names in byte order over REST and GraphQL
    Given the event types "user.created,order.paid" are configured
    And a ready node with a topic of its own
    When GET request is sent to "/api/event-types"
    Then the response status code should be 200
    And the JSON response should be:
      """
      [{"name": "order.paid"}, {"name": "user.created"}]
      """
    When the GraphQL operation is sent:
      """
      { eventTypes { name } }
      """
    Then the response status code should be 200
    And the GraphQL response should have no errors
    And the JSON response should be:
      """
      {"data": {"eventTypes": [{"name": "order.paid"}, {"name": "user.created"}]}}
      """

  Scenario: A node on a topic of its own reports ready, a reachable broker and its position
    # Only a topic of its own makes the position known: the status feature reads the shared topic of
    # the stack, where other scenarios write.
    Given a ready node with a topic of its own
    When a "user.created" event is produced
    And the node has read every produced event
    And GET request is sent to "/api/events/status"
    Then the response status code should be 200
    And the JSON field "state" should equal "ready"
    And the response body should contain '"brokerReachable":true'
    And the JSON field "position" should equal "1"
    And the JSON field "windowFirst" should equal "0"
    And the JSON field "windowLast" should equal "1"

  Scenario: An SSE subscriber receives only the events of its type
    Given a ready node with a topic of its own
    And an SSE subscriber of "user.created"
    # The last "user.created" event is written after the "order.paid" one, so a delivered
    # "order.paid" frame would arrive before it and break the comparison.
    When these events are produced:
      | type         |
      | user.created |
      | order.paid   |
      | user.created |
    Then the SSE subscriber receives the produced "user.created" events with their id, event and data fields

  Scenario: A client resumes from the window with the last id it received
    Given a ready node with a topic of its own
    And an SSE subscriber of "user.created"
    When these events are produced:
      | type         |
      | user.created |
      | user.created |
    Then the SSE subscriber receives the produced "user.created" events with their id, event and data fields
    When the SSE subscriber disconnects
    And these events are produced:
      | type         |
      | user.created |
      | user.created |
    And the node has read every produced event
    And the SSE subscriber reconnects with the header Last-Event-ID of the last id it received
    And a "user.created" event is produced
    Then the SSE subscriber receives the produced "user.created" events with their id, event and data fields

  Scenario: A client below the window resumes from Kafka, then from the window
    # EVENT_WINDOW_MAX_BYTES cannot go below 2 MiB, so the count bound makes the window small.
    Given the environment variable "EVENT_WINDOW_MAX_EVENTS" is set to "10"
    And a ready node with a topic of its own
    When 25 "user.created" events are produced
    And the node has read every produced event
    Then the window of the node no longer holds the first produced event
    When an SSE subscriber of "user.created" resumes from the id "0" in the query parameter lastEventId
    # Event 26 arrives live, right after event 25: a gap or a duplicate between the replay and the
    # live events would break the comparison.
    And a "user.created" event is produced
    Then the SSE subscriber receives the produced "user.created" events with their id, event and data fields

  Scenario: An invalid record between two valid events is skipped and counted
    Given a ready node with a topic of its own
    And an SSE subscriber of "user.created"
    And the status of the node reports 0 rejected records
    When a "user.created" event is produced
    And a record that is not an event is produced
    And a "user.created" event is produced
    Then the SSE subscriber receives the produced "user.created" events with their id, event and data fields
    And the status of the node reports 1 rejected record

  Scenario: A node whose topic does not exist yet answers a subscription with the 503 problem
    Given a node whose topic does not exist yet
    # The node has looked for its topic, so the answer comes from the missing topic, not from a
    # node that has not read the broker yet.
    And the status of the node reports the state starting with the reason "topic-not-found"
    When GET request is sent to "/api/events/stream?types=user.created"
    Then the response status code should be 503
    And the response header "content-type" should contain "application/problem+json"
    And the response body should contain "the node is starting"
