# Load testing

The load-test kit is in `tests/Load`. It runs [K6](https://k6.io) from the pinned image `grafana/k6` (nothing is built) against the production image of the service. This document describes the profiles, the budgets and the rule that derives them, how to add a scenario, the isolated environment the runs use, and the runner for a temporary AWS instance.

## Profiles

Each scenario runs up to four profiles. The settings of every profile are declared per endpoint in `tests/Load/config.json.dist`.

| Profile | Shape | Requests per second | Virtual users | Duration | Where it runs |
| --- | --- | --- | --- | --- | --- |
| `smoke` | constant arrival rate | 10 | 5 | 10 seconds | every pull request and every push to `main` (`make smoke-load-tests`) |
| `average` | ramp up, plateau, ramp down | peak 25 | 25 | 3 seconds rise, 10 seconds plateau, 3 seconds fall | on demand (`make average-load-tests`) |
| `stress` | ramp up, plateau, ramp down | peak 150 | 150 | 3 seconds rise, 10 seconds plateau, 3 seconds fall | on demand (`make stress-load-tests`) |
| `spike` | ramp up, ramp down | peak 200 | 200 | 5 seconds rise, 5 seconds fall | on demand (`make spike-load-tests`) |

`make load-tests` runs all four profiles one after the other, with the pause `delayBetweenScenarios` (60 seconds) between them. Only the smoke profile runs in a pipeline. The table gives the profiles of the `health` scenario; the `subscriptions` scenario has its own (see [The subscriptions scenario](#the-subscriptions-scenario)). The runners take the scenarios in sorted order, `health` then `subscriptions`; their smoke profiles take about 15 and 20 seconds once the environment is up.

A run fails when the p99 of the request duration of a profile exceeds its budget, or when the success rate of its checks is not above 99%; the `subscriptions` scenario also fails when the p99 of its delivery latency exceeds its `deliveryThreshold`. The runners fail with `Error: No load test scenarios found.` when `tests/Load/scripts` holds no scenario.

## Budgets

`threshold` in `tests/Load/config.json.dist` is the p99 budget in milliseconds. It must be a positive number; any other value is rejected with `Invalid threshold value` before the run starts. A local `tests/Load/config.json` (git-ignored) overrides the file, and the variables `API_HOST` and `API_PORT` override the target.

The budgets of the `health` scenario (`GET /api/health`, checked for status 204):

| Profile | Budget (p99, milliseconds) | Ceiling (milliseconds) | Where the budget comes from |
| --- | --- | --- | --- |
| `smoke` | 50 | 500 | the floor of the budget rule: 3 x 1.17 ms (worst p99 of three local runs, 2026-11-22) and 3 x 7.25 ms (4-vCPU GitHub-hosted runner, 2026-11-11) are both below 50 ms |
| `average` | 50 | 200 | the floor of the budget rule: 3 x 1.00 ms (worst p99 of three local runs, 2026-11-22) = 3.00 ms, rounded up to 10 ms |
| `stress` | 50 | 1000 | the floor of the budget rule: 3 x 1.29 ms (worst p99 of three local runs, 2026-11-22) = 3.87 ms, rounded up to 10 ms |
| `spike` | 50 | 5000 | the floor of the budget rule: 3 x 0.89 ms (worst p99 of three local runs, 2026-11-22) = 2.67 ms, rounded up to 10 ms |

**The measurements of 2026-11-22.** Since the removal of PostgreSQL and of SQS ([ADR 0023](adr/0023-sqs-is-removed.md)) a health request no longer calls SQS or LocalStack, nor any other backing service: it reads in-memory state only (the cache and event-stream checks run in memory), so the scenario measures the service itself. Before this change the health scenario collapsed at about 200 to 400 requests per second, and the stress and spike profiles of the `subscriptions` scenario failed with `Too many open files (os error 24)`, because the soft `nofile` limit of the service container was 1024 and every held subscriber holds one socket. The service of `docker-compose.prod.yml` now sets `nofile` to 1048576 (soft and hard), and both failures are gone. The budgets of both scenarios were then derived from three consecutive runs of `make load-tests BUILD_IMAGE=0` on the production image of the branch, on a local 4-core, 8-thread Intel Core i7-1165G7 laptop with Docker 29.10: every check passed in every run (`health` 3373, 3374 and 3373 checks; `subscriptions` 12040, 12040 and 12035 checks), and every p99 is in the table under [Measured values](#measured-values). The maintainer chose to derive the budgets from these local measurements instead of waiting for hosted-runner measurements of the larger profiles, and kept the floor of 50 ms. Every `health` p99 stayed below 1.3 ms, so all four `health` budgets are the floor of 50 ms.

**The earlier measurements.** When the measurements of 2026-11-11 were taken, each health request made one `SELECT 1` and one `ListQueues` call to LocalStack. The smoke profile ran on a 4-vCPU GitHub-hosted runner in the job `Smoke load tests` of commit `db7fe08` (2026-11-11): p99 7.25 ms, 101 of 101 checks passed. The rule gives 3 x 7.25 = 21.75 ms, rounded up to 30 ms, raised to the floor of 50 ms, so the smoke budget is 50 ms. A second hosted run, job `Smoke load tests` of commit `14d0d18` on the same day, measured a p99 of 6.45 ms with 101 of 101 checks: the rule gives 3 x 6.45 = 19.35 ms, rounded up to 20 ms, raised to the floor of 50 ms, so the budget stays 50 ms. CI runs the smoke profile only. The average, stress and spike profiles run in no pipeline (each of them, and `make load-tests`, was run locally on 2026-11-11; see the table below), so they had no hosted-runner measurement and stayed at their ceilings until the measurements of 2026-11-22; their local measurements are in the table below and are not hosted-runner values. On that machine, while other work shared it, a second stress run measured 1.48 s and failed the 1000 ms budget (load average 9.3), and one smoke run during a concurrent container compile measured 66.41 ms and failed the 50 ms budget (load average 11); every other local run stayed within its budget. The budgets and ceilings were not changed then, because the rule used the hosted-runner value only; a busy developer machine can fail `make smoke-load-tests`.

The four budget values are protected configuration: changing one needs the label `approved-config-change` on the pull request. The rest of the file changes freely. The success-rate threshold of the checks (above 99%) lives in `tests/Load/utils/thresholdsBuilder.js`, which is a protected file in its entirety.

### The budget rule

Each budget is three times the measured p99, rounded up to a multiple of 10 milliseconds, and it is not below 50 milliseconds (so that machine and runner noise does not fail the gate) and not above its ceiling. The measured p99 is the worst of the p99 values of three consecutive local runs of the production image (`make load-tests BUILD_IMAGE=0`) and of the p99 values measured on a 4-vCPU GitHub-hosted runner, where one exists:

```text
budget = min(ceiling, max(50, 3 x worst measured p99 rounded up to 10))
```

Ceilings: smoke 500 ms, average 200 ms, stress 1000 ms, spike 5000 ms for the request budgets `threshold` of both scenarios (the PHP template used 9000 ms for smoke), and 2000 ms for every `deliveryThreshold` of the `subscriptions` scenario.

Procedure:

1. Run `make load-tests BUILD_IMAGE=0` three times in a row on the production image of the change, on a machine that runs nothing else, and read the p99 of every profile from the end-of-run summaries or from the dashboards in `tests/Load/results/`. Every check must pass in every run. For the smoke profile, also read the p99 of the hosted runner from the job log or from the `load-test-results` artifact.
2. Take the worst p99 of each profile, compute the budget with the rule and record every measurement, its date and the budget in the table below. Add rows; never overwrite old ones.
3. Change the budgets in `tests/Load/config.json.dist` in a pull request with the label `approved-config-change`.
4. The average, stress and spike profiles run in no pipeline, so their budgets come from the local runs until a maintainer measures them on comparable hardware (the default AWS instance has 4 vCPU) and applies the same rule.

## The subscriptions scenario

`tests/Load/scripts/subscriptions.js` loads the event stream: it holds SSE, GraphQL and WebSocket subscribers of the event type `load.test` while a publisher of the load-test stack writes events of that type to the topic, and it measures how long an event takes from the publisher to a subscriber.

**The publisher.** The service has no publishing route and no Rust code publishes. The service `load-test-publisher` of `docker-compose.load_test.override.yml` uses the image of the broker: it waits until `/opt/kafka/bin/kafka-topics.sh --describe --topic notifications` succeeds, then pipes one envelope every 100 milliseconds (about 10 events per second, fixed in the stack and not driven by the profile) into one `/opt/kafka/bin/kafka-console-producer.sh` with `linger.ms=0` and `acks=all`. Each envelope has a new `id`, the type `load.test`, the current time as `occurredAt` and the data `{"sentAtMs":<epoch milliseconds>}`. Only the isolated load-test project runs it, never the development stack.

**One iteration.** `setup` asks `GET /api/events/status` once a second until `state` is `ready` and `windowLast` is not `null` (the node holds at least one event of the publisher), so the start of the stack and the JVM starts of the publisher stay out of the measured iterations; it fails the run after 25 attempts. Each iteration then:

1. requests the catalog `GET /api/event-types` and checks status 200; this request alone carries the profile's `test_type` tag, so it alone is measured against the p99 budget `threshold`;
2. holds one WebSocket subscriber (`k6/websockets`, `/api/events/ws?types=load.test`), one SSE subscriber (`http.asyncRequest` on `/api/events/stream?types=load.test` with the time-out `hold`) and one GraphQL subscriber (`http.asyncRequest` with a POST on `/api/graphql`, the headers `Accept: text/event-stream` and `Content-Type: application/json`, the body `{"query":"subscription { events(types: [\"load.test\"]) { id type data } }"}` and the time-out `hold`) for `hold` seconds, then closes the WebSocket while the SSE and GraphQL requests end by their own time-outs;
3. checks that the WebSocket opened (the open event follows the server's 101 answer only), that it received at least one event, that the held SSE response received at least one event (its body holds a frame `event: load.test`), and that the held GraphQL response received at least one subscription result (its body holds a block `event: next` whose `data:` line is a result `{"data":{"events":{...}}}` of the type `load.test`; only the blocks ended by a blank line are read, because the time-out can cut the last one short);
4. records, for every WebSocket frame, `Date.now() - data.data.sentAtMs` (`data` is the event object of the frame) in the trend `event_delivery_latency`.

A profile starts `rps` iterations per second and each one lasts `hold` seconds, so once it has ramped up it holds `rps` x `hold` subscribers of each kind: 10 x 5 = 50 SSE, 50 GraphQL and 50 WebSocket subscribers in the smoke profile (`GET /api/events/status` reports `subscribers` 150 then: each GraphQL subscription registers one subscriber of the node, like the other two kinds). `vus` leaves room above the number of iterations held at once, since one virtual user holds the three subscribers of its iteration. The SSE and GraphQL requests carry the tag `test_type:held_subscription` instead of the profile's, so their held durations never enter the p99 budget of the catalog; the GraphQL request is named `POST /api/graphql (subscription)` in the output.

**Which form of the SSE check the scenario uses, and why.** An SSE response never ends by itself, so the held request always ends by its K6 time-out. Before the scenario was written, one `http.asyncRequest` with `timeout: '3s'` was sent against the SSE route of a running service while events were written to its topic, with the pinned image `grafana/k6:2.3.0` (2026-11-15): K6 answered status 0, the error `request timeout` (code 1050) and the body received before the time-out (4 676 bytes, 28 frames). Because the pinned K6 returns that body, the scenario uses the direct form: the held SSE response must have received at least one event. The fallback form, which counts `subscribers` in `GET /api/events/status` while the SSE requests are held and measures delivery on the WebSocket only, is not used; it becomes the check if a later K6 stops returning the body of a request ended by its own time-out. Delivery latency is measured on the WebSocket in both forms.

**Which form of the GraphQL check the scenario uses, and why.** The GraphQL subscriber is held the same way as the SSE subscriber: its response is an event stream that never ends by itself, so its `http.asyncRequest` also ends by its own K6 time-out. What K6 returns for such a request does not depend on the route, and the probe above showed that the pinned K6 returns the body received before the time-out, so the scenario uses the direct form for the GraphQL subscriber too: the held GraphQL response must have received at least one `event: next` whose data carries the type `load.test`. The fallback form is the same as for SSE, the count of `subscribers` in `GET /api/events/status` while the requests are held, and it becomes the check of both subscribers if a later K6 stops returning that body. The GraphQL subscriber is not used for the delivery latency, which the WebSocket measures.

**What the output shows.** Every held SSE and GraphQL request ends with K6's warning `Request Failed error="request timeout"` and counts in `http_req_failed` (about two thirds of the scenario's HTTP requests: of the three requests of an iteration, only the catalog ends by itself). Both are expected and fail nothing: the success of the SSE and of the GraphQL subscriber is its check on the body.

**Configuration.** `endpoints.subscriptions` in `tests/Load/config.json.dist` declares the four profiles in the shapes of the table under [Profiles](#profiles), each with two fields that only this scenario reads: `deliveryThreshold`, the p99 budget of `event_delivery_latency` in milliseconds (a positive number; any other value is rejected with `Invalid deliveryThreshold value` before the run starts), and `hold`, the seconds each iteration holds its three subscribers.

| Profile | `rps` | `vus` | Duration (seconds) | `hold` (seconds) | Subscribers of each kind once ramped up | Catalog budget `threshold` (p99, milliseconds) | `deliveryThreshold` (p99, milliseconds) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `smoke` | 10 | 60 | 10 | 5 | 50 | 50 | 50 |
| `average` | 20 | 120 | 3 rise, 10 plateau, 3 fall | 5 | 100 | 50 | 130 |
| `stress` | 100 | 600 | 3 rise, 10 plateau, 3 fall | 5 | 500 | 240 | 310 |
| `spike` | 150 | 800 | 5 rise, 5 fall | 5 | about 560 at the peak | 650 | 830 |

Both numbers are budgets derived with the [budget rule](#the-budget-rule) from the worst p99 of the three local runs of 2026-11-22 (no hosted-runner measurement of this scenario exists yet):

| Profile | Worst catalog p99 (ms) | Catalog budget (ms) | Worst delivery p99 (ms) | Delivery budget (ms) |
| --- | --- | --- | --- | --- |
| `smoke` | 1.14 | 50 (3 x 1.14 = 3.42, rounded up to 10, floor 50) | 10 | 50 (3 x 10 = 30, floor 50) |
| `average` | 7.54 | 50 (3 x 7.54 = 22.62, rounded up to 30, floor 50) | 42 | 130 (3 x 42 = 126, rounded up to 130) |
| `stress` | 77.35 | 240 (3 x 77.35 = 232.05, rounded up to 240) | 102 | 310 (3 x 102 = 306, rounded up to 310) |
| `spike` | 213.98 | 650 (3 x 213.98 = 641.94, rounded up to 650) | 275 | 830 (3 x 275 = 825, rounded up to 830) |

The catalog ceilings are those of the `health` scenario; the delivery ceiling of 2 000 milliseconds is the one the requirement sets for the smoke profile, used for every profile because the event rate of the publisher does not change with the profile. No budget reaches its ceiling. The stress and spike p99 values vary much more between runs than the others (catalog stress 9.08 to 77.35 ms, spike 6.40 to 213.98 ms), so their budgets carry the worst run. The delivery interval includes the console producer, the broker, the service's read of the topic and the WebSocket write, so it can neither prove nor disprove the fan-out design target of the service. Unlike the four `health` budgets, the values of this entry are not protected configuration; like them, they are never raised to make a failing run pass, and a change of a budget records its measurement in the table below.

## Measured values

One row per measurement. Add a row for every new measurement; do not overwrite old rows. Until 2026-11-22 only a hosted-runner row derived a budget and a local row recorded what the profile measured on that machine; since then the rows of three consecutive local runs of the production image derive budgets too (see [The budget rule](#the-budget-rule)). An empty cell means that nothing has been measured yet.

| Scenario | Profile | Environment | Date | Measured p99 (ms) | Budget derived (ms) | Budget in `config.json.dist` (ms) |
| --- | --- | --- | --- | --- | --- | --- |
| `health` | `smoke` | local, 4 vCPU virtual machine, inside `make ci` after a clean clone, production image, `BUILD_IMAGE=0` (details in `docs/operations.md`) | 2026-11-11 | 14.79 | not derived; the rule uses the hosted-runner value | 500 |
| `health` | `smoke` | GitHub-hosted runner, 4 vCPU, `ubuntu-24.04`, job `Smoke load tests` of commit `db7fe08`, production image | 2026-11-11 | 7.25 | 50 (3 x 7.25 = 21.75, rounded up to 30, floor 50) | 50 |
| `health` | `smoke` | GitHub-hosted runner `ubuntu-24.04`, job `Smoke load tests` (job id 130769689140, run 43091554513) of commit `14d0d18`, production image built in the job; that commit still held the budget of 500 ms (101 of 101 checks) | 2026-11-11 | 6.45 | 50 (3 x 6.45 = 19.35, rounded up to 20, floor 50) | 50 |
| `health` | `average` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make average-load-tests` (324 requests, checks 100%) | 2026-11-11 | 14.34 | not derived; the rule uses the hosted-runner value | 200 |
| `health` | `stress` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make stress-load-tests` (1949 requests, checks 100%) | 2026-11-11 | 16.71 | not derived; the rule uses the hosted-runner value | 1000 |
| `health` | `spike` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make spike-load-tests` (999 requests, checks 100%) | 2026-11-11 | 219.98 | not derived; the rule uses the hosted-runner value | 5000 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make load-tests` (all four profiles in one run, 3374 checks, 100%) | 2026-11-11 | 25.37 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `average` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 19.13 | not derived; the rule uses the hosted-runner value | 200 |
| `health` | `stress` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 57.23 | not derived; the rule uses the hosted-runner value | 1000 |
| `health` | `spike` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 91.77 | not derived; the rule uses the hosted-runner value | 5000 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make smoke-load-tests`, three runs on an idle machine (101 checks each, 100%) | 2026-11-11 | 25.88, 23.51, 35.22 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine, production image, `BUILD_IMAGE=0`, `make smoke-load-tests`, three runs with all four cores busy (load average 9 to 12; 101 checks each, 100%) | 2026-11-11 | 24.51, 23.71, 30.43 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make smoke-load-tests`, three runs as the machine was (100 checks each, 100%) | 2026-11-11 | 18.9, 22.55, 16.66 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make smoke-load-tests`, three runs with four busy loops (100 checks each, 100%) | 2026-11-11 | 33.63, 20.03, 17.66 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make smoke-load-tests` during a concurrent container compile (load average 11; 100 of 100 checks); **crossed the budget, exit 2** | 2026-11-11 | 66.41 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `average` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make average-load-tests` (325 of 325 checks) | 2026-11-11 | 32.04 | not derived; the rule uses the hosted-runner value | 200 |
| `health` | `stress` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make stress-load-tests` (load average 9.3; 1873 of 1873 checks, 76 dropped iterations); **crossed the budget, exit 2** | 2026-11-11 | 1480 | not derived; the rule uses the hosted-runner value | 1000 |
| `health` | `spike` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make spike-load-tests` (999 of 999 checks) | 2026-11-11 | 144.85 | not derived; the rule uses the hosted-runner value | 5000 |
| `health` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make load-tests` (all four profiles in one run, 3374 of 3374 checks) | 2026-11-11 | 38.86 | not derived; the rule uses the hosted-runner value | 50 |
| `health` | `average` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 35.63 | not derived; the rule uses the hosted-runner value | 200 |
| `health` | `stress` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 399.22 | not derived; the rule uses the hosted-runner value | 1000 |
| `health` | `spike` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work, production image, `BUILD_IMAGE=0`, `make load-tests` | 2026-11-11 | 394.73 | not derived; the rule uses the hosted-runner value | 5000 |
| `subscriptions` | `smoke` | local, 4 vCPU and 15 GB Linux virtual machine shared with other work; **not the production image** (it could not be built on that machine) but a debug build of the service, a topic of its own on the development broker fed by the loop of `load-test-publisher`, and `tests/Load/execute-load-test.sh` called with `API_HOST` and `API_PORT`; two runs (404 and 400 checks, 100%; `subscribers` 100 while the subscribers were held) | 2026-11-15 | catalog 3.19 and 3.65; delivery 8 and 8 | not derived; the rule uses the hosted-runner value | catalog 500, delivery 2000 |
| `health` | `smoke` | local, 4-core, 8-thread Intel Core i7-1165G7 laptop, Docker 29.10, production image of the branch, `BUILD_IMAGE=0`, `make load-tests`, three consecutive runs (3373, 3374 and 3373 checks, 100%) | 2026-11-22 | 1.17, 0.76, 0.82 | 50 (3 x 1.17 = 3.51, rounded up to 10, floor 50) | 50 |
| `health` | `average` | local, 4-core, 8-thread Intel Core i7-1165G7 laptop, Docker 29.10, production image of the branch, `BUILD_IMAGE=0`, `make load-tests`, three consecutive runs (3373, 3374 and 3373 checks, 100%) | 2026-11-22 | 0.92, 0.93, 1.00 | 50 (3 x 1.00 = 3.00, rounded up to 10, floor 50) | 50 |
| `health` | `stress` | local, 4-core, 8-thread Intel Core i7-1165G7 laptop, Docker 29.10, production image of the branch, `BUILD_IMAGE=0`, `make load-tests`, three consecutive runs (3373, 3374 and 3373 checks, 100%) | 2026-11-22 | 0.89, 1.26, 1.29 | 50 (3 x 1.29 = 3.87, rounded up to 10, floor 50) | 50 |
| `health` | `spike` | local, 4-core, 8-thread Intel Core i7-1165G7 laptop, Docker 29.10, production image of the branch, `BUILD_IMAGE=0`, `make load-tests`, three consecutive runs (3373, 3374 and 3373 checks, 100%) | 2026-11-22 | 0.89, 0.86, 0.89 | 50 (3 x 0.89 = 2.67, rounded up to 10, floor 50) | 50 |
| `subscriptions` | `smoke` | local, 4-core, 8-thread Intel Core i7-1165G7 laptop, Docker 29.10, production image of the branch, `BUILD_IMAGE=0`, `make load-tests`, three consecutive runs (12040, 12040 and 12035 checks, 100%) | 2026-11-22 | catalog 1.06, 0.89, 1.14; delivery 7, 4, 10 | catalog 50 (3 x 1.14 = 3.42, rounded up to 10, floor 50), delivery 50 (3 x 10 = 30, floor 50) | catalog 50, delivery 50 |
| `subscriptions` | `average` | local, 4-core, 8-thread Intel Core i7-1165G7 laptop, Docker 29.10, production image of the branch, `BUILD_IMAGE=0`, `make load-tests`, three consecutive runs (12040, 12040 and 12035 checks, 100%) | 2026-11-22 | catalog 1.20, 1.08, 7.54; delivery 6, 6, 42 | catalog 50 (3 x 7.54 = 22.62, rounded up to 30, floor 50), delivery 130 (3 x 42 = 126, rounded up to 130) | catalog 50, delivery 130 |
| `subscriptions` | `stress` | local, 4-core, 8-thread Intel Core i7-1165G7 laptop, Docker 29.10, production image of the branch, `BUILD_IMAGE=0`, `make load-tests`, three consecutive runs (12040, 12040 and 12035 checks, 100%) | 2026-11-22 | catalog 9.08, 77.35, 48.49; delivery 26, 102, 89 | catalog 240 (3 x 77.35 = 232.05, rounded up to 240), delivery 310 (3 x 102 = 306, rounded up to 310) | catalog 240, delivery 310 |
| `subscriptions` | `spike` | local, 4-core, 8-thread Intel Core i7-1165G7 laptop, Docker 29.10, production image of the branch, `BUILD_IMAGE=0`, `make load-tests`, three consecutive runs (12040, 12040 and 12035 checks, 100%) | 2026-11-22 | catalog 213.98, 8.45, 6.40; delivery 275, 36, 27 | catalog 650 (3 x 213.98 = 641.94, rounded up to 650), delivery 830 (3 x 275 = 825, rounded up to 830) | catalog 650, delivery 830 |

## Running the tests

| Target | What it does |
| --- | --- |
| `make smoke-load-tests` | every scenario, smoke profile |
| `make average-load-tests` | every scenario, average profile |
| `make stress-load-tests` | every scenario, stress profile |
| `make spike-load-tests` | every scenario, spike profile |
| `make load-tests` | every scenario, all four profiles |
| `make execute-load-tests-script scenario=health runAverage=false runStress=false runSpike=false` | one scenario; every profile runs unless its switch (`runSmoke`, `runAverage`, `runStress`, `runSpike`) is set to `false`, so this example runs the smoke profile only; the selected profiles run one after the other with the pause `delayBetweenScenarios` between them, also when a profile in the middle is switched off |
| `make lint-load-tests` | ESLint and Prettier on the load-test JavaScript |

Every run writes one self-contained HTML dashboard to `tests/Load/results/<prefix><scenario>.html` and prints K6's end-of-run summary with the statistics `avg,min,med,max,p(95),p(99)`. The prefixes are `smoke-`, `average-`, `stress-`, `spike-` and `all-`; a run of `make execute-load-tests-script` has none. The directory is git-ignored apart from its `.gitignore`. CI uploads it as the artifact `load-test-results` (14 days).

The scenario script `execute-load-test.sh` in `tests/Load` is the single entry point of every runner. It exits with status 2 and prints its usage when the scenario is empty or unknown.

## Adding a scenario

A scenario is one script and one configuration entry; no runner needs an edit.

1. Create `tests/Load/scripts/<scenario>.js`. Copy `tests/Load/scripts/health.js` and change the request and the checks. The script builds its options from the configuration through the helpers in `tests/Load/utils`.
2. Add `endpoints.<scenario>` to `tests/Load/config.json.dist` with the four profiles (`smoke`, `average`, `stress`, `spike`), each with `threshold`, `rps`, `vus` and `duration`, in the shapes of the table above. A field that only your script reads, such as `deliveryThreshold` and `hold` of the `subscriptions` scenario, goes into the same profile objects.
3. Start with ceilings you can defend, then derive the budgets with the budget rule once a measurement exists.
4. Run `make execute-load-tests-script scenario=<scenario> runAverage=false runStress=false runSpike=false` (the smoke profile only), then `make smoke-load-tests`, which now includes the scenario.
5. Run `make lint-load-tests`. The JavaScript runs in K6, not in Node: only the K6 globals (`__ENV`, `__VU`, `__ITER`, `open`) exist, and `console` is forbidden.
6. Add the measurement to the table above.

Only the four budget values of the shipped configuration are protected in `config.json.dist`, and `utils/thresholdsBuilder.js` is protected as a file: run `make validate-configuration` when you change either.

The assistant skill `load-testing` lists the same steps, with the `health` scenario as its illustration.

## The isolated environment

Schemathesis and K6 run against the production image in a compose project of their own, `<project>-load`, never against the development stack. The project has its own network and volumes, publishes no broker port, and publishes the service on `LOAD_TEST_APP_PORT` (default `18081`). It also runs `load-test-publisher`, which writes the events of the `subscriptions` scenario to its broker from the start of the project to its removal. The development service on port 8081 is a debug build and keeps running next to it.

`scripts/with-load-test-stack.sh` builds the production image (unless `BUILD_IMAGE=0`), starts the project and waits until it is healthy, runs the given command with `API_HOST=localhost` and `API_PORT=$LOAD_TEST_APP_PORT`, and always removes the project with its volumes, also when the command fails. When the command fails (a crossed K6 threshold, a failed Schemathesis check), it prints a line with the exit status and the logs of the project, the service's included, to standard error before it removes the project, and exits with the status of the command. It exits with status 2 and prints the logs when the environment does not become healthy.

To test a service that already runs elsewhere, call the runners directly with `API_HOST` and `API_PORT` set; no environment is started then.

## The AWS runner

`make aws-load-tests` runs the load tests on a temporary EC2 instance and `make aws-load-tests-cleanup` removes everything it created. Use it to measure on hardware that is not your own. The host needs Docker and git only (the dry run, git only): the AWS CLI runs from a pinned image, the run id comes from `/dev/urandom`, and the policy files are rendered with `sed`.

```sh
make aws-load-tests region=eu-west-1 instance_type=c6i.xlarge load_target=smoke-load-tests
make aws-load-tests-cleanup
```

Parameters (all optional): `region`, `ami`, `instance_type`, `instance_tag`, `role`, `branch`, `repo_url`, `security_group`, `load_target`. Defaults: region `us-east-1`; instance type `c6i.xlarge` (4 vCPU, 8 GiB; a cold release build of the image takes about 5 minutes, which a `t2.micro` cannot do); the AMI is Ubuntu 24.04, resolved for the chosen region from the public SSM parameter `/aws/service/canonical/ubuntu/server/24.04/stable/current/amd64/hvm/ebs-gp3/ami-id`; the repository is `repo_url` (an http or https URL; credentials are removed) or else the `origin` remote of the checkout, converted to HTTPS (a remote that is a local path is refused with exit status 2: give `repo_url=https://…`); the branch is the checked-out branch, which must be pushed; the load target is `smoke-load-tests`.

### Credentials

The runner reads the AWS credentials from the environment of the host and never from a file of the repository:

| Variable | Meaning |
| --- | --- |
| `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY` | the access key of a user or role allowed to manage the resources listed below |
| `AWS_SESSION_TOKEN` | only with temporary credentials |
| `AWS_REGION` | the region, when the parameter `region` (option `-r`) is not given |

The region is taken from the `region` Make parameter (option `-r`) first, then from the `AWS_REGION` variable of the caller's environment, then `us-east-1`; a value in `.env` is not read for it. The cleanup uses the region recorded in `tests/Load/aws-run.env`, whatever the parameter or the environment say.

Without them (and without local mode) the script stops and names the variables. Export them in the shell, or let a profile tool export them; do not put them in `.env`. The identity needs permissions for EC2 (run and terminate instances, security groups), S3 (bucket, objects, bucket policy), IAM (role, instance profile, managed policy) and the SSM parameter read; use a sandbox account.

### What it does

1. Creates a security group, an S3 bucket, a role with a bucket policy, a managed policy rendered from `s3-access-policy.json`, and an instance profile. Their names carry a run id of 8 hexadecimal characters and are written to `tests/Load/aws-run.env`, which `make aws-load-tests-cleanup` reads. IAM changes need time to propagate: the script retries up to 53 times, 5 seconds apart.
2. Starts the instance with a 30 GB root volume and the shutdown behaviour `terminate`. On the instance, `user-data.sh` installs Docker, git, make and the AWS CLI from Ubuntu packages and snap, clones the repository and branch, runs `make <load_target>`, and in an exit trap uploads `tests/Load/results/` and the cloud-init log, writes `DONE` or `FAILED` to the bucket and shuts the instance down, so it terminates on success and on failure.
3. Waits for `<run-id>/DONE` or `<run-id>/FAILED`, polling every 30 seconds for at most 45 minutes (`wait=false` skips the wait), downloads the results to `tests/Load/results/aws/<run-id>/`, prints `Launched instance: <id>` and `You can access the S3 bucket here: <console URL>`, and exits with status 1 on `FAILED` or on timeout.

Run `make aws-load-tests-cleanup` afterwards, also after a failure or a timeout. It skips resources that no longer exist and removes `aws-run.env`.

### Costs

A smoke run keeps one `c6i.xlarge` instance for the image build (about 5 minutes), the environment start and the profiles, in all roughly 15 minutes of instance time, plus a 30 GB volume and a few megabytes in S3. At the on-demand price of that instance type, which is of the order of 0.15 to 0.20 US dollars per hour in `us-east-1` and differs by region, a run costs a few cents. A run whose `FAILED` marker or timeout leaves the instance up costs by the hour until it is terminated: check the console after a timeout. Prices change; look up the current price of the instance type in your region before you run `average`, `stress` or `spike` profiles for a long time. The security group, bucket and role cost next to nothing, but the cleanup target removes them so that nothing is left behind.

### What is verified

The AWS flow is verified only by its dry run: with `LOCAL_MODE_ENV=true` every call goes to the stub AWS CLI `tests/Load/stub-aws.sh` instead of the CLI image, so the dry run needs neither credentials, an emulator nor Docker. The stub answers each call of the runner and of its cleanup with a canned value (the AMI of the SSM parameter, a VPC, a security group, an instance id, ARNs that carry the run id) and starts nothing, so the wait is skipped. With `AWS_STUB_LOG=<file>` it appends every call to that file. The Bats suite asserts the two output markers, the calls made and a clean `make aws-load-tests-cleanup` in that mode. Nothing in CI starts a real instance. The first real run is yours: expect to adjust the permissions of the identity, the instance type or the region, and report what you had to change.
