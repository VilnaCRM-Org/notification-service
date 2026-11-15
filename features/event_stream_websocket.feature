Feature: Event stream over WebSocket
  In order to change the events I follow without reconnecting
  As a frontend developer
  I want to subscribe and unsubscribe event types on an open WebSocket

  Scenario: A WebSocket without types subscribes at run time and unsubscribes
    Given a ready node with a topic of its own
    And a WebSocket without types
    When the WebSocket subscribes to "order.paid"
    And an "order.paid" event is produced
    Then the WebSocket receives the produced "order.paid" events as JSON text frames
    When the WebSocket unsubscribes from "order.paid"
    And the WebSocket subscribes to "user.created"
    # The "user.created" event is written after the "order.paid" one, so a delivered
    # "order.paid" frame would arrive before it and break the comparison.
    And these events are produced:
      | type         |
      | order.paid   |
      | user.created |
    Then the WebSocket receives the produced "user.created" events as JSON text frames
