# Targets of this service. The Makefile loads this file with `-include`; the template ships none, so a template
# sync never changes it. Every target here has a row in docs/make-targets-service.md.

.PHONY: ensure-kafka

ensure-kafka: ## Start the Kafka broker with its host listener unless this shell is the development container
	[ "$(IN_DEV_CONTAINER)" = 1 ] || $(DOCKER_COMPOSE) -f docker-compose.yml -f docker-compose.override.yml -f docker-compose.kafka-host.yml up --detach --wait kafka

# The test targets that need services (integration, coverage, end-to-end, mutation) start the broker first.
ensure-deps: ensure-kafka
