# Service make targets

The targets that this service adds in `make/service.mk`, in the format of the table "All targets" of [the template's reference](make-targets.md). The `Makefile` loads that file with `-include`, and the documentation check requires a row here for each of its targets.

| Target | Class | Needs | In `make ci` | Help text |
| --- | --- | --- | --- | --- |
| `ensure-deps` | HOST | Docker | no | Start LocalStack and Kafka unless this shell is the development container; this file adds `ensure-kafka` as its prerequisite, so every target that starts the backing services also starts the broker |
| `ensure-kafka` | HOST | Docker | no | Start the Kafka broker with its host listener unless this shell is the development container |

`ensure-kafka` runs `docker compose -f docker-compose.yml -f docker-compose.override.yml -f docker-compose.kafka-host.yml up --detach --wait kafka`. The extra file `docker-compose.kafka-host.yml` publishes the broker's host listener on `127.0.0.1:9095`, which the tests on the host reach through `KAFKA_BOOTSTRAP_SERVERS=localhost:9095`; `make start` does not read that file, so the development stack publishes no Kafka port. The broker is healthy once its health check has created the topic `notifications` with one partition. When the development project already runs `kafka` without the host port, compose recreates the container; the topic and its records stay on the volume `kafka_data`. Inside the development container (`IN_DEV_CONTAINER=1`) the target does nothing, because the app reaches the broker at `kafka:9092`.
