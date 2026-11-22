workspace "Service architecture" "C4 model of the service: system context, containers and components." {
    !identifiers hierarchical
    model {
        properties {
            "structurizr.groupSeparator" "/"
        }
        apiClient = person "API client" "A person or program that calls the REST and GraphQL API and subscribes to events over Server-Sent Events, WebSocket, or GraphQL subscriptions over Server-Sent Events."
        kafka = softwareSystem "Kafka" "The VilnaCRM event bus; the notifications topic." {
            tags "External"
        }
        system = softwareSystem "Service" "A VilnaCRM service built from the Rust service template." {
            service = container "Service binary" "Serves REST under /api, GraphQL at /api/graphql, with subscriptions over Server-Sent Events, the SSE and WebSocket event streams at /api/events/stream and /api/events/ws, /api/health and /ping; runs the supervised tail of the Kafka topic; also exports the API contracts." "Rust, axum, tokio" {
                compositionRoot = component "service" "Composition root: CLI, configuration, wiring, router, middleware, OpenAPI document, GraphQL schema and its Server-Sent Events transport, the supervised tail loop and the readiness probe of the event stream." "Rust crate"
                group "Shared" {
                    group "Shared/Domain" {
                        sharedDomain = component "shared-domain" "Aggregate root, domain event, bus ports, technical ports, UUID and page value objects." "Rust crate" {
                            tags "Domain"
                        }
                    }
                    group "Shared/Application" {
                        sharedApplication = component "shared-application" "Problem document, problem-aware extractors, fallbacks, request timeout, GraphQL error helpers." "Rust crate" {
                            tags "Application"
                        }
                    }
                    group "Shared/Infrastructure" {
                        sharedInfrastructure = component "shared-infrastructure" "In-process buses, cache adapter, clock, UUID generator." "Rust crate" {
                            tags "Infrastructure"
                        }
                    }
                }
                group "Internal/HealthCheck" {
                    group "Internal/HealthCheck/Domain" {
                        healthCheckDomain = component "health-check-domain" "HealthProbe port, Dependency and HealthReport value objects." "Rust crate" {
                            tags "Domain"
                        }
                    }
                    group "Internal/HealthCheck/Application" {
                        healthCheckApplication = component "health-check-application" "HealthService and the GET /api/health handler." "Rust crate" {
                            tags "Application"
                        }
                    }
                    group "Internal/HealthCheck/Infrastructure" {
                        healthCheckInfrastructure = component "health-check-infrastructure" "The cache probe." "Rust crate" {
                            tags "Infrastructure"
                        }
                    }
                }
                group "Notification/EventStream" {
                    group "Notification/EventStream/Domain" {
                        eventStreamDomain = component "event-stream-domain" "Event, cursor and offset value objects, the domain errors and the topic ports of the event stream." "Rust crate" {
                            tags "Domain"
                        }
                    }
                    group "Notification/EventStream/Application" {
                        eventStreamApplication = component "event-stream-application" "Event catalog and the list event types query, the envelope parser, the memory window, the tail use case, the hub, the subscriptions with resume and replay, the node status, the GET /api/event-types and GET /api/events/status handlers, the SSE and WebSocket routes GET /api/events/stream and GET /api/events/ws, the GraphQL eventTypes and eventStreamStatus queries, and the GraphQL subscription field events." "Rust crate" {
                            tags "Application"
                        }
                    }
                    group "Notification/EventStream/Infrastructure" {
                        eventStreamInfrastructure = component "event-stream-infrastructure" "The Kafka adapter of the topic ports: metadata, watermarks, tail and replay readers, and the Kafka connection settings." "Rust crate" {
                            tags "Infrastructure"
                        }
                    }
                }
                compositionRoot -> sharedApplication "Uses fallbacks and middleware"
                compositionRoot -> sharedInfrastructure "Builds buses and cache"
                compositionRoot -> healthCheckApplication "Mounts the health route"
                compositionRoot -> healthCheckInfrastructure "Registers the probes"
                compositionRoot -> sharedDomain "Wires the bus, clock and UUID ports"
                compositionRoot -> healthCheckDomain "Collects the probes behind the HealthProbe port"
                compositionRoot -> eventStreamApplication "Builds the catalog, window, hub, tail use case, subscriptions and status; mounts the event-types, status, SSE and WebSocket routes and the GraphQL queries and subscription; supervises the tail loop and reads its state for readiness"
                compositionRoot -> eventStreamDomain "Parses EVENT_TYPES into event types and holds the topic readers behind the TopicProbe, TopicTail and TopicReplay ports"
                compositionRoot -> eventStreamInfrastructure "Parses the Kafka settings and builds the probe, tail and replay readers of the Kafka topic that feed the supervised tail and the replays"
                sharedApplication -> sharedDomain "Uses bus errors"
                sharedInfrastructure -> sharedDomain "Implements bus, cache, clock and UUID ports"
                healthCheckDomain -> sharedDomain "Uses the shared error type"
                healthCheckApplication -> healthCheckDomain "Runs probes through the HealthProbe port"
                healthCheckApplication -> sharedApplication "Returns problem documents"
                healthCheckApplication -> sharedDomain "Formats the error chain of a failed probe"
                healthCheckInfrastructure -> healthCheckDomain "Implements HealthProbe"
                healthCheckInfrastructure -> sharedDomain "Uses the Cache port"
                eventStreamDomain -> sharedDomain "Uses the shared error type in the topic ports"
                eventStreamApplication -> eventStreamDomain "Pages the catalog of event types and reads the topic through the TopicProbe, TopicTail and TopicReplay ports into stored events by cursor"
                eventStreamApplication -> sharedDomain "Answers through the query bus ports, pages by PageRequest and reads the time from the Clock port"
                eventStreamApplication -> sharedApplication "Uses extractors, problem documents and the GraphQL error helper"
                eventStreamInfrastructure -> eventStreamDomain "Implements the topic ports"
                eventStreamInfrastructure -> sharedDomain "Uses the shared error type"
            }
        }
        apiClient -> system.service "Calls the REST and GraphQL API and subscribes to events over SSE, WebSocket and GraphQL subscriptions over SSE" "HTTP, JSON, Server-Sent Events, WebSocket"
        system.service.eventStreamInfrastructure -> kafka "Reads the notifications topic from" "Kafka protocol, krafka"
    }
    views {
        systemContext system "SystemContext" "The service, its callers and the systems it depends on." {
            include *
            autolayout lr
        }
        container system "Containers" "The service binary." {
            include *
            autolayout lr
        }
        component system.service "Components" "One component per workspace crate, grouped by bounded context and layer." {
            include *
            autolayout tb
        }
        styles {
            element "Person" {
                shape Person
            }
            element "External" {
                background #999999
                color #ffffff
            }
            element "Domain" {
                background #2e7d32
                color #ffffff
            }
            element "Application" {
                background #1565c0
                color #ffffff
            }
            element "Infrastructure" {
                background #6a1b9a
                color #ffffff
            }
        }
    }
}
