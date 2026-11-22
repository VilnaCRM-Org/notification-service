Feature: GraphQL subscription over an event stream
  In order to follow live events through the GraphQL schema with a plain HTTP client or an EventSource
  As a frontend developer
  I want to subscribe over SSE with a POST or a GET of the GraphQL path and resume after a disconnect

  # Every scenario runs in both forms: POST sends the operation as a JSON body, GET sends it in the
  # query parameter query, as an EventSource does; both send Accept: text/event-stream.

  Scenario Outline: A GraphQL subscriber receives only the events of its type
    Given a ready node with a topic of its own
    And a GraphQL subscription of "user.created" is open in the <form> form
    # The last "user.created" event is written after the "order.paid" one, so a delivered
    # "order.paid" result would arrive before it and break the comparison.
    When these events are produced:
      | type         |
      | user.created |
      | order.paid   |
      | user.created |
    Then the GraphQL subscriber receives the produced "user.created" events as next blocks with their id

    Examples:
      | form |
      | POST |
      | GET  |

  Scenario Outline: A GraphQL subscriber resumes with the last id it received
    Given a ready node with a topic of its own
    And a GraphQL subscription of "user.created" is open in the <form> form
    When these events are produced:
      | type         |
      | user.created |
      | user.created |
    Then the GraphQL subscriber receives the produced "user.created" events as next blocks with their id
    When the GraphQL subscriber disconnects
    And these events are produced:
      | type         |
      | user.created |
      | user.created |
    And the node has read every produced event
    And the GraphQL subscriber of "user.created" resubscribes in the <form> form with the last id it received in the <carrier>
    # Event 5 arrives live, right after the retained events 3 and 4: an id at or below the cursor,
    # a live event before a retained one or a 3 or a 4 delivered twice breaks the first comparison.
    # Event 6 is produced once 5 was read and must be the very next result: a 3, 4 or 5 delivered
    # again, such as event 5 both replayed from the window and taken from the live queue at the
    # hand-over, comes before it and breaks the second comparison.
    And a "user.created" event is produced
    Then the GraphQL subscriber receives the produced "user.created" events as next blocks with their id
    When a "user.created" event is produced
    Then the GraphQL subscriber receives the produced "user.created" events as next blocks with their id

    Examples:
      | form | carrier              |
      | POST | argument lastEventId |
      | POST | header Last-Event-ID |
      | GET  | argument lastEventId |
      | GET  | header Last-Event-ID |

  Scenario Outline: An invalid subscription is answered inside the stream with the validation error
    Given a ready node with a topic of its own
    And the status of the node reports 0 subscribers
    When a GraphQL subscription with "<case>" is sent in the <form> form
    Then the GraphQL answer is an event stream
    And the GraphQL stream reads a comment
    And the GraphQL stream reads one next block whose error is VALIDATION_FAILED on "<path>" with the code "<code>"
    And the GraphQL stream reads the complete block with a bare data line
    And the GraphQL stream ends
    And the status of the node reports 0 subscribers

    Examples:
      | form | case                                                 | path          | code              |
      | POST | an invalid type name                                 | types         | pattern           |
      | POST | 101 distinct type names                              | types         | max_items         |
      | POST | an empty list of types                               | types         | required          |
      | POST | a malformed lastEventId                              | lastEventId   | format            |
      | POST | a malformed Last-Event-ID                            | Last-Event-ID | format            |
      | POST | a malformed lastEventId beside a valid Last-Event-ID | lastEventId   | format            |
      | POST | two root fields                                      | query         | single_root_field |
      | GET  | an invalid type name                                 | types         | pattern           |
      | GET  | 101 distinct type names                              | types         | max_items         |
      | GET  | an empty list of types                               | types         | required          |
      | GET  | a malformed lastEventId                              | lastEventId   | format            |
      | GET  | a malformed Last-Event-ID                            | Last-Event-ID | format            |
      | GET  | a malformed lastEventId beside a valid Last-Event-ID | lastEventId   | format            |
      | GET  | two root fields                                      | query         | single_root_field |

  Scenario Outline: A node whose topic does not exist yet answers a GraphQL subscription with the 503 problem
    Given a node whose topic does not exist yet
    # The node has looked for its topic, so the answer comes from the missing topic, not from a
    # node that has not read the broker yet.
    And the status of the node reports the state starting with the reason "topic-not-found"
    When a GraphQL subscription of "user.created" is sent in the <form> form
    Then the response status code should be 503
    And the response header "content-type" should contain "application/problem+json"
    And the JSON field "detail" should equal "the node is starting"
    And the GraphQL answer has no event-stream head

    Examples:
      | form |
      | POST |
      | GET  |

  Scenario Outline: An open GraphQL subscription without events writes a comment every keep-alive interval
    Given the environment variable "STREAM_KEEP_ALIVE" is set to "1s"
    And a ready node with a topic of its own
    When a GraphQL subscription of "user.created" is sent in the <form> form
    Then the GraphQL answer is an event stream
    And the GraphQL stream reads a comment
    # The second comment is due 1 s after the body started, which is after the request was sent.
    # The upper bound leaves 4 s for scheduling, well below the default interval of 15 s, so a body
    # that does not use STREAM_KEEP_ALIVE fails within 5 s.
    And the GraphQL stream reads a comment between 1 and 5 seconds after the request was sent

    Examples:
      | form |
      | POST |
      | GET  |

  Scenario Outline: A query sent as an event stream is answered with one result, then complete
    Given the event types "user.created,order.paid" are configured
    And a ready node with a topic of its own
    When the GraphQL operation is sent in the <form> form as an event stream:
      """
      { eventTypes { name } }
      """
    Then the GraphQL answer is an event stream
    And the GraphQL stream reads a comment
    And the GraphQL stream reads one next block with the result:
      """
      {"data": {"eventTypes": [{"name": "order.paid"}, {"name": "user.created"}]}}
      """
    And the GraphQL stream reads the complete block with a bare data line
    And the GraphQL stream ends

    Examples:
      | form |
      | POST |
      | GET  |

  Scenario Outline: A subscription from an allowed origin carries that origin on its event-stream head
    Given the environment variable "CORS_ALLOWED_ORIGINS" is set to "http://app.example"
    And a ready node with a topic of its own
    And the GraphQL event-stream requests come from the origin "http://app.example"
    When a GraphQL subscription of "user.created" is sent in the <form> form
    Then the GraphQL answer is an event stream
    And the response header "access-control-allow-origin" should contain "http://app.example"

    Examples:
      | form |
      | POST |
      | GET  |
