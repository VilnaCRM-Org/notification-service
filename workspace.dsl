workspace "Service architecture" "C4 model of the service: system context, containers and components." {
    !identifiers hierarchical
    model {
        properties {
            "structurizr.groupSeparator" "/"
        }
        apiClient = person "API client" "A person or program that calls the REST and GraphQL API."
        messageBroker = softwareSystem "Message broker" "Amazon SQS, or LocalStack in development." {
            tags "External"
        }
        kafka = softwareSystem "Kafka" "The VilnaCRM event bus; the notifications topic." {
            tags "External"
        }
        system = softwareSystem "Service" "A VilnaCRM service built from the Rust service template." {
            service = container "Service binary" "Serves REST under /api, GraphQL at /api/graphql, /api/health and /ping; also exports the API contracts." "Rust, axum, tokio" {
                compositionRoot = component "service" "Composition root: CLI, configuration, wiring, router, middleware, OpenAPI document, GraphQL schema." "Rust crate"
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
                        sharedInfrastructure = component "shared-infrastructure" "In-process buses, cache adapter, clock, UUID generator, SQS client factory." "Rust crate" {
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
                        healthCheckInfrastructure = component "health-check-infrastructure" "Cache and SQS probes." "Rust crate" {
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
                        eventStreamApplication = component "event-stream-application" "Event catalog, the list event types query and the GET /api/event-types handler." "Rust crate" {
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
                compositionRoot -> sharedInfrastructure "Builds buses, cache and SQS client"
                compositionRoot -> healthCheckApplication "Mounts the health route"
                compositionRoot -> healthCheckInfrastructure "Registers the probes"
                compositionRoot -> sharedDomain "Wires the bus, clock and UUID ports"
                compositionRoot -> healthCheckDomain "Collects the probes behind the HealthProbe port"
                compositionRoot -> eventStreamApplication "Mounts the event catalog route and registers its handler"
                compositionRoot -> eventStreamDomain "Parses EVENT_TYPES into event types"
                compositionRoot -> eventStreamInfrastructure "Parses the Kafka settings"
                sharedApplication -> sharedDomain "Uses bus errors"
                sharedInfrastructure -> sharedDomain "Implements bus, cache, clock and UUID ports"
                healthCheckDomain -> sharedDomain "Uses the shared error type"
                healthCheckApplication -> healthCheckDomain "Runs probes through the HealthProbe port"
                healthCheckApplication -> sharedApplication "Returns problem documents"
                healthCheckApplication -> sharedDomain "Formats the error chain of a failed probe"
                healthCheckInfrastructure -> healthCheckDomain "Implements HealthProbe"
                healthCheckInfrastructure -> sharedDomain "Uses the Cache port"
                eventStreamDomain -> sharedDomain "Uses the shared error type in the topic ports"
                eventStreamApplication -> eventStreamDomain "Pages the catalog of event types"
                eventStreamApplication -> sharedDomain "Answers through the query bus ports"
                eventStreamApplication -> sharedApplication "Uses extractors and problem documents"
                eventStreamInfrastructure -> eventStreamDomain "Implements the topic ports"
                eventStreamInfrastructure -> sharedDomain "Uses the shared error type"
            }
        }
        apiClient -> system.service "Calls the REST and GraphQL API" "HTTP, JSON"
        system.service.healthCheckInfrastructure -> messageBroker "Calls ListQueues" "HTTPS, AWS SDK"
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
