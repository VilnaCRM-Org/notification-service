# Capability Catalog: PHP Service Template to Rust Service Template

- **BMAD phase:** 1 (Analysis), `analyst` command, autonomous run. **Date:** 2026-11-10.
- **Purpose:** one traceable list of every capability the PHP template has, with a binding disposition for the Rust template. Later planning stages (brief, PRD, architecture, epics, readiness) use this file instead of re-reading the five PHP inventories.
- **Inputs:** `scope-decisions.md` (binding), `research/php-app-code.md`, `research/php-tests-and-quality.md`, `research/php-docker-make-load.md`, `research/php-ci-github-contracts.md`, `research/php-ai-docs-audit.md`, `research/rust-http-api-stack.md`, `research/rust-data-cache-messaging.md`.
- **Not available when this was written:** `research/rust-quality-gates.md` and `research/rust-testing-delivery.md`. Every value those reports are expected to supply is marked "architect" below.

## How to read this catalog

- **Ids.** Capability ids are copied unchanged from the inventories: `CAP-nn` (application code), `CAP-TQ-nn` (tests and quality), `CAP-DM-nn` (Docker, Make, load), `CAP-CI-nn` (CI, GitHub, contracts), `CAP-AD-nn` (AI guidance, documentation, audit). A row that lists several ids merges capabilities that state the same thing in different inventories.
- **Disposition.** Exactly one of:
  - `KEEP`: same contract, Rust mechanism.
  - `FIX`: the intent is kept and a named PHP defect is corrected.
  - `EXTEND`: `scope-decisions.md` asks for more than PHP has.
  - `DROP`: non-goal; the Note cites the scope clause.
- **Scope references.** "Scope 3.1" and similar point at sections of `scope-decisions.md`. "Analyst decision" marks a row where `scope-decisions.md` is silent; all such rows are listed again in "Decisions taken by the analyst".
- **Defect references.** `APP:D-nn` = `php-app-code.md`; `TQ:nn` = numbered item in `php-tests-and-quality.md`; `DM:D-nn` = `php-docker-make-load.md`; `CI:D-nn` = `php-ci-github-contracts.md`; `AD:D-AD-nn` = `php-ai-docs-audit.md`. `Pnn` = audit problem. `W-nn` = PHP workflow number in `php-ci-github-contracts.md`. `NEW-nn` = capability added beyond PHP (see that section).
- **"Architect"** in a Note means the architecture stage owns the choice and must record it with evidence from `research/`.

## Totals (verified by counting)

| Inventory | Id range | Capability ids |
| --- | --- | --- |
| `php-app-code.md` | `CAP-01` to `CAP-41` | 41 |
| `php-tests-and-quality.md` | `CAP-TQ-01` to `CAP-TQ-36` | 36 |
| `php-docker-make-load.md` | `CAP-DM-01` to `CAP-DM-50` | 50 |
| `php-ci-github-contracts.md` | `CAP-CI-01` to `CAP-CI-36` | 36 |
| `php-ai-docs-audit.md` | `CAP-AD-01` to `CAP-AD-33` | 33 |
| **Total** | | **196** |

- Source ids covered by the tables below: **196 of 196**, each exactly once (checked by extracting the first column of every table row and comparing with the id lists of the five inventories).
- Catalog rows: **161**. Thirty-one rows merge duplicates and absorb 35 ids (28 rows with two ids, 2 rows with three, 1 row with four).
- Rows by disposition: `KEEP` 71, `FIX` 73, `EXTEND` 8, `DROP` 9.
- Ids by disposition: `KEEP` 81, `FIX` 95, `EXTEND` 9, `DROP` 11.
- Capabilities added beyond PHP: **42** (`NEW-01` to `NEW-42`).
- Source defects digested: **188** (38 + 44 + 40 + 33 + 33) reduced to **150** deduplicated lines.

Rows per area:

| # | Area | Rows | Ids |
| --- | --- | --- | --- |
| 1 | Service runtime and HTTP surface | 16 | 16 |
| 2 | Health and liveness | 7 | 9 |
| 3 | Example bounded context | 1 | 1 |
| 4 | Shared kernel | 6 | 6 |
| 5 | Persistence, cache and messaging | 7 | 7 |
| 6 | Architecture and its enforcement | 4 | 5 |
| 7 | Formatting, static analysis and complexity | 8 | 14 |
| 8 | Dependency and security checks | 4 | 6 |
| 9 | Unit and integration tests | 9 | 12 |
| 10 | End-to-end tests | 4 | 5 |
| 11 | Coverage and mutation testing | 3 | 5 |
| 12 | Makefile command surface and its tests | 12 | 16 |
| 13 | Contract artifacts and contract checks | 8 | 12 |
| 14 | Load testing | 14 | 15 |
| 15 | Containers and compose stacks | 18 | 19 |
| 16 | CI workflows | 3 | 5 |
| 17 | Release, dependency updates and template sync | 4 | 6 |
| 18 | Git hooks and commit convention | 2 | 4 |
| 19 | AI-assistant guidance | 23 | 24 |
| 20 | Human documentation | 7 | 8 |
| 21 | Architecture model (Structurizr) | 1 | 1 |
| | **Total** | **161** | **196** |

## Catalog

### 1. Service runtime and HTTP surface

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-01 | One deployable HTTP service whose behaviour is selected by an environment mode (dev, test, prod) with per-mode settings. | Symfony kernel and Runtime, `when@<env>` blocks, `services_<env>.yaml` | KEEP | Scope 3.1: one service binary, plain HTTP, one port. `APP_ENV` keeps its name (scope 4). The only mode-dependent behaviour scope names is the GraphQL explorer (development only). Scope requires no separate debug flag. |
| CAP-02 | An operator CLI shares the service configuration and can run migrations, check configuration and export the API contracts. | `bin/console` | KEEP | Scope 3.1: explicit `migrate` command, documented rollback command, `check-config` style command, contract snapshots generated by CLI commands. Research verified export subcommands that need no database (`rust-http-api-stack.md` D3, D4). Cache-clearing console commands have no Rust counterpart. |
| CAP-03 | All deploy-specific settings come from environment variables, with committed non-secret defaults for local use. | Dotenv, tracked `.env` and `.env.test` | FIX | Defects APP:D-34, APP:D-35, DM:D-09: the env file is tracked although git-ignored and carries secret-shaped values; the test env file doubles as a Makefile include. Scope 4: ecosystem-standard names where a tool requires them, PHP names for compose-level settings, nothing secret-shaped committed, local-only defaults labelled. See NEW-05, NEW-35. |
| CAP-04 | Components are wired by constructor injection in one place, and every implementation of a contract (probes, handlers, subscribers) can be supplied as a collection. | Symfony autowire and autoconfigure, `_instanceof` tags, `!tagged` | KEEP | Analyst decision (scope silent). Rust mechanism: one explicit composition root that builds the collections. Automatic binding by reflection is not reproduced. |
| CAP-05 | The production build carries start-up optimisations and no debug artefacts. | `services_prod.yaml`, opcache preload, Doctrine production caches | KEEP | Analyst decision (scope silent). Met by construction by a release-profile binary in the production image target (CAP-DM-02). Preload and opcode-cache settings have no Rust meaning and create no work item. |
| CAP-06 | All API routes are served under the `/api` path prefix. | `config/routes/api_platform.yaml` `prefix: /api` | KEEP | Scope 3.1. Exception named by scope: `/ping` is outside the prefix (CAP-DM-20). |
| CAP-14 | The API is JSON-only: success bodies are `application/json` and every error is a problem document. | `api_platform.yaml` `formats`, `error_formats` | FIX | Defect APP:D-23: responses for unknown routes, wrong methods and unsupported media types are neither specified nor tested. Scope 3.1: all errors are `application/problem+json` (RFC 9457), including unknown routes, wrong methods, unsupported media types, validation failures, timeouts and panics. Research pitfall: framework-default extractor rejections are `text/plain` and must be replaced. |
| CAP-18 | Human-browsable API documentation is served at `/api/docs` with HTTP 200. | API Platform Swagger UI via Twig and Asset | KEEP | Scope 3.1: docs UI at `/api/docs`. Research pitfall: UI assets must be vendored at build time. Tested by CAP-TQ-13. |
| CAP-19 | A GraphQL endpoint is served at `/api/graphql`, its schema is exportable as SDL to a versioned file, and the browser IDE is disabled. | API Platform GraphQL, `api:graphql:export`, `graphiql: enabled: false` | FIX | Defects APP:D-19, CI:D-27: the PHP schema is an accidental Relay and CRUD projection with no code behind it. Scope 3.1: only intentionally implemented operations (NEW-10); the interactive explorer is enabled in the development environment only (P25), which replaces the PHP "always disabled" setting. |
| CAP-20 | A development-only error-page preview route exists. | `/_error` routes in `config/routes/framework.yaml` | DROP | Analyst decision. HTML error pages belong to HTML templating, a non-goal (scope 3.2, row "Mercure, Vulcain, HTML templating, sessions"). Symfony-specific; all errors are problem documents (CAP-14). |
| CAP-34 | A request input validation facility is available. | `symfony/validator`, `validator.yaml` (enabled, used by nothing) | EXTEND | Scope 3.1 asks for more than PHP has: the create operation validates its input (NEW-07) and validation failures are problem responses (CAP-14). |
| CAP-35 | A translation and locale facility with a default and fallback locale is available. | `symfony/translation`, `translation.yaml`, empty `translations/` | DROP | Scope 3.2, row "Internationalisation (P20)": declared non-goal; no empty scaffolding ships. |
| CAP-36 | Server-side HTML templating and cookie-session support are available. | Twig `templates/base.html.twig`, `framework.session` | DROP | Scope 3.2, row "Mercure, Vulcain, HTML templating, sessions": configured but unused in PHP. |
| CAP-DM-18 | A plain-HTTP listener on port 8081 serves tooling (health probe, load tests, API fuzzing, CLI tests). | Caddy site `:8081` with `env APP_ENV {$APP_RUNTIME_HTTP_ENV:test}` | KEEP | Scope 3.1: default published port `8081`. It becomes the only listener, so the PHP per-listener mode switch disappears (removes defect DM:D-14). The Rust HTTP research spike listened on `:8080`; `scope-decisions.md` prevails. |
| CAP-DM-19 | The public listener serves HTTPS and HTTP/3 with automatic certificates and response compression, and both listeners write structured access logs. | Caddy and FrankenPHP: ports 443/tcp and 443/udp, `encode zstd gzip`, `log` | DROP | Scope 3.1 ("TLS, HTTP/3 and compression are left to the platform edge; no Caddy or reverse proxy ships") and scope 3.2, row "Reverse proxy, in-process TLS, HTTP/3": platform edge. The access-log part is not lost: request logging moves into the service (NEW-01, NEW-02). |
| CAP-CI-15 | A check confirms that the runtime environment meets the service's requirements. | `symfony check:requirements` (`make check-requirements`, W-17, pre-commit) | KEEP | Scope 3.1: a `check-config` style command replaces Symfony's requirements check. Host prerequisites are a separate new capability (NEW-21). |

### 2. Health and liveness

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-07, CAP-TQ-10 | `GET /api/health` returns `204 No Content` with an empty body when the database, cache and message-broker probes all succeed. | `HealthCheckController` plus three event subscribers; `features/health_check.feature` scenario 1 | KEEP | Scope 3.1 keeps the wire contract. Asserted by the reused Gherkin feature and by the K6 health scenario (CAP-DM-46). |
| CAP-08 | When a probe fails, the health endpoint returns `500` with an `application/problem+json` body that contains the failure message. | Uncaught exception rendered by API Platform; message visible because the test environment runs with debug on | FIX | Defects APP:D-12, APP:D-13, TQ:10: the text is whatever the driver throws, the three asserted strings exist only in test mocks, the first failure hides the rest. Scope 3.1: `500` problem response; all failing dependencies are reported; the messages are owned by application code and are exactly `Database is not available`, `Cache is not working`, `Message broker is not available`. |
| CAP-09 | The database probe runs a trivial query (`SELECT 1`) on the primary PostgreSQL connection. | `DBCheckSubscriber` via Doctrine DBAL | KEEP | Scope 3.1: the database is one of the three probed dependencies. Runs under a per-probe timeout (CAP-12). |
| CAP-10 | The cache probe does a read-through fetch of key `health_check` (storing `ok` on a miss) on the application cache. | `CacheCheckSubscriber` via Symfony Cache contracts | FIX | Defect APP:D-15: after the first request the probe only reads a never-expiring key, so a cache that stops accepting writes still reports healthy. Scope principle 3. Analyst decision on the probe behaviour: it exercises a write and a read through the cache port on each call (research D6). With an in-process adapter the probe has little diagnostic value; it is kept for wire parity and for a later remote adapter. |
| CAP-11 | The message-broker probe calls the SQS-compatible endpoint. | `BrokerCheckSubscriber`: `SqsClient::createQueue('health-check-queue')` | FIX | Defects APP:D-14, TQ:13: the probe creates a queue on every request. Scope 3.1: the broker probe is non-mutating. The Rust data research spike kept `CreateQueue` for parity and lists a read-only probe as open; `scope-decisions.md` prevails and the architect chooses the read-only call. |
| CAP-12 | Health probes are pluggable: adding a probe adds one component and does not change the endpoint handler. | Framework event fan-out to subscribers extending `BaseHealthCheckSubscriber` | FIX | Defects APP:D-07, APP:D-11, APP:D-12: probes ride the framework dispatcher, sit in the wrong layer and run sequentially with no timeout. Scope 3.1: probes run concurrently, each under a timeout. Rust: one probe port, one adapter per dependency, registered in the composition root (`rust-data-cache-messaging.md` D6). |
| CAP-DM-20, CAP-CI-31 | A liveness path separate from the dependency health endpoint answers `GET /ping` with `200` and body `pong` without touching any dependency. | Caddy `respond "pong" 200` on both listeners | KEEP | Scope 3.1: served by the service itself, not part of the OpenAPI document, used by container healthchecks and CI wait loops. The Rust HTTP research spike used `/api/health/live`; `scope-decisions.md` prevails. |

### 3. Example bounded context

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-13 | An example resource endpoint `GET /api/example_api_resources` returns `200 application/json` with `name` and `description` and demonstrates the module layout. | `ExampleApiResource`, `ExampleApiResourceProvider`, `resources.yaml`; fixed placeholder values | EXTEND | Scope 3.1 replaces the inert placeholder with a real vertical slice at the same REST path: the `GET` becomes a list with a bounded page, and create, read by id, an event subscriber and GraphQL operations are added (NEW-07 to NEW-10, NEW-12). No update or delete (scope 3.2). Deliberate wire change: PHP returned one fixed object at the collection path (defect APP:D-21). |

### 4. Shared kernel

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-25 | An aggregate-root base records domain events and hands them over exactly once, in recording order. | `AggregateRoot::record` / `pullDomainEvents` (unused) | KEEP | Scope 3.1: aggregate root with recorded domain events. Used by the example aggregate, which removes the PHP "unused" state (APP:D-10). |
| CAP-26 | A domain-event contract defines event id, occurrence time, stable event name and conversion to and from primitive values. | abstract `DomainEvent` (unused) | FIX | Defects APP:D-09, TQ:18: the timestamp is an unvalidated string and its test compares wall-clock values. Scope 3.1 keeps the contract. Rust: typed timestamp, controllable clock in tests; primitive conversion ships only where the audit subscriber (NEW-09) uses it (scope principle 5). |
| CAP-27 | An event-bus port with an in-process synchronous adapter routes each event to its subscribers and surfaces a failing subscriber's error. | `EventBusInterface` + `InMemorySymfonyEventBus` on Symfony Messenger (unused) | FIX | Defects APP:D-02, APP:D-03, APP:D-04, APP:D-05, APP:D-06: duplicate port, one subscriber per event kept silently, zero subscribers treated as an error, reflection-based routing. Scope 3.1: an event may have zero or many subscribers. |
| CAP-28 | A command-bus port sends each command to exactly one handler, returns nothing, and raises a dedicated error when no handler is registered. | `CommandBusInterface`, marker interfaces, `CommandNotRegisteredException`; adapter class missing | FIX | Defect APP:D-01: no adapter exists. Scope 3.1: working in-process adapter; a missing command handler is an error. The query side is NEW-11. |
| CAP-29 | Handlers and subscribers are registered without editing the bus. | `_instanceof` tagging and `!tagged` injection | KEEP | Analyst decision (scope silent). Rust mechanism: explicit registration in the composition root (CAP-04); discovery by reflection is not reproduced. A missing registration is caught by the "missing handler is an error" rule and by tests. |
| CAP-30 | A UUID value object with a factory is available. | `Uuid`, `UuidFactoryInterface` / `UuidFactory`, `UuidTransformer` (unused) | FIX | Defects APP:D-08, TQ:23: accepts any string and cannot generate. Scope 3.1: validated UUID value object and generator. The 16-byte binary form and the transformer ship only if the slice uses them (scope principle 5). |

### 5. Persistence, cache and messaging

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-21 | PostgreSQL connectivity is configured from one URL variable, and tests use a separate database. | Doctrine DBAL, `DB_URL`, `DB_VERSION`, `dbname_suffix: _test` | KEEP | Scope 3.1 and 4: `DATABASE_URL`, default `DB_VERSION=16`. The PHP test-database rule is `<name>_test` with an optional per-worker suffix; the Rust isolation mechanism is the architect's decision (research verified per-test databases). Avoid APP:D-32 (host and port hard-coded in the URL, version configured twice). |
| CAP-22 | Versioned, reversible schema migrations are kept in the repository and are runnable from the CLI. | Doctrine Migrations; one sample migration | FIX | Defect APP:D-30: the only migration creates a foreign `test_user` table with an asymmetric down step. Scope 3.1: reversible SQL migrations, explicit `migrate` command, documented rollback command (NEW-13); the first migration belongs to the example slice (NEW-12). |
| CAP-DM-09 | A started stack has its pending migrations applied without a manual step. | `doctrine:migrations:migrate --no-interaction` in the container entrypoint | FIX | Analyst decision. Defect APP:D-30 (applied implicitly at every container start) and audit P14 (migration as a deliberate step). Scope 3.1 names an explicit `migrate` command: the stack start runs it as its own step and the service process does not migrate implicitly. How `make start` and CI invoke it is the architect's decision. |
| CAP-23 | Persistence mapping has a defined place and a snake_case naming convention. | Doctrine ORM mapping of `src/Shared/Domain/Entity` (empty), production query caches | FIX | Analyst decision. Defects APP:D-31, AD:D-AD-11: the mapping pointed into the Domain layer at an empty directory. Rust: mapping lives in the Infrastructure adapter behind the repository port (NEW-12); snake_case names in SQL migrations. ORM query and result caches have no counterpart. |
| CAP-31 | A configured client for an SQS-compatible broker points at a local emulator by default. | `Aws\Sqs\SqsClient` fed by `AWS_SQS_*` and `LOCALSTACK_PORT` | FIX | Defect APP:D-17: endpoint without scheme, API version `latest`, committed fake credentials, emulator-named variables. Scope 3.1 and 4: SQS client wiring against LocalStack, used by the broker probe; names `AWS_REGION`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_ENDPOINT_URL_SQS`. Explicit timeouts (NEW-06). |
| CAP-32 | An application cache abstraction lets the backend be swapped. | Symfony Cache `cache.app` (filesystem by default), `ArrayAdapter` in test | KEEP | Scope 3.1: cache port with an in-process adapter. Redis is a documented extension and is not shipped (scope 3.2, NEW-42). |
| CAP-33 | A configuration slot exists for an asynchronous message transport. | `messenger.yaml` (malformed, not functional) | DROP | Scope 3.2, row "Asynchronous message consumer / worker": PHP has no working one. Scope 3.1: no asynchronous consumer, no outbox. |

### 6. Architecture and its enforcement

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-37 | The source layout is mandatory: bounded context, module, then Application / Domain / Infrastructure, plus a Shared kernel with the same layers and an Internal context for non-business features. | `src/` tree; required-directory list in `scripts/validate-configuration.sh` | KEEP | Scope 3.1: `Shared`, `Internal/HealthCheck`, `CompanySubdomain/SomeModule`, each split into Domain / Application / Infrastructure. On-disk Rust naming is the architect's decision. |
| CAP-15 | Transport and framework metadata are declared outside domain types, so the Domain layer carries no framework code. | `config/api_platform/resources.yaml` instead of attributes | KEEP | Scope 3.1: Domain is framework-free. Rust: API schema, serialization and persistence code sits on Application or Infrastructure types; enforced by the architecture gate (CAP-38). |
| CAP-38, CAP-CI-08 | Automated dependency-rule enforcement between layers fails on violations and on code assigned to no layer, and Domain depends on nothing. | Deptrac `--report-uncovered --fail-on-uncovered` (`make deptrac`, W-06) | FIX | Defects APP:D-24, APP:D-25, APP:D-26, APP:D-27: 14 of 32 classes unassigned, placeholder names hard-coded in the rules, Application and Infrastructure may depend on each other. Scope 3.1: hard gate, zero unassigned code, structural rules by layer position, no rule-file edit for a new context, cross-context rule (NEW-14). Mechanism: architect, from `research/rust-quality-gates.md`. |
| CAP-39 | Source units that belong to no layer can be listed. | `make deptrac-debug` (`debug:unassigned`) | KEEP | Analyst decision (scope silent). The architecture gate's failure output names every unassigned unit; a separate target exists only if the chosen mechanism needs one. |

### 7. Formatting, static analysis and complexity

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-TQ-25 | Code is formatted automatically with a pinned rule set. | PHP-CS-Fixer (`@PSR12` plus ten overrides), `make phpcsfixer` | FIX | Defects TQ:4, TQ:5, CI:D-03: always fix mode, never fails, the file filter expands to nothing. Scope 3.1: formatting check that can fail, with a separate fix target. |
| CAP-TQ-20, CAP-CI-09 | Static correctness analysis runs over production and test code with zero findings. | Psalm over `src` and `tests` at `errorLevel="8"` (W-14) | FIX | Defect TQ:6 and audit P11: most lenient level. Scope 3.1: strict static analysis with warnings denied. |
| CAP-TQ-22, CAP-CI-11 | A quality gate applies numeric minimums for code quality, complexity, architecture and style. | PHPInsights 100 / 95 / 100 / 100 (W-13) | KEEP | Scope 3.1 expresses the same intent as separate gates: static analysis with warnings denied, complexity limits, architecture boundaries, formatting check. No composite percentage score exists for Rust; see "Thresholds". |
| CAP-TQ-23 | Structural code rules are enforced automatically. | Final-or-abstract classes, `declare(strict_types=1)`, no unused imports, sorted imports, 80-column lines, no Yoda conditions | KEEP | Analyst decision (scope silent on the individual rules). Carried: no unused imports, ordered imports, a line-width limit (formatter plus lints with warnings denied). No Rust meaning: final-or-abstract, strict-types declaration, Yoda rule. Line-width value: architect. |
| CAP-TQ-24 | Complexity and mess detection runs with a relaxed profile for tests. | PHPMD `codesize,design,cleancode`; `phpmd.tests.xml` | FIX | Defects TQ:1, DM:D-01: the tool is not installed, runs in no workflow and breaks `make ci`. Scope 3.1: complexity limits as a gate that is a Make target and a CI check. Whether tests get a relaxed profile is the architect's decision. |
| CAP-TQ-26, CAP-CI-12 | An automated modernisation check fails when machine-applicable rewrites are pending, and a separate mode applies them. | Rector `--dry-run` (`rector-ci`, W-15) and `rector-apply` | KEEP | Analyst decision (scope silent). Folded into the static-analysis gate: lints with machine-applicable fixes fail the check, and the apply mode is the lint auto-fix target. No separate tool. Avoid TQ:37, CI:D-08 (a mode variable that never reaches the tool). |
| CAP-CI-34 | Formatting and lint conventions exist for non-application files (YAML, Markdown, JavaScript), with generated contract files excluded. | `.prettierrc.yml`, `.prettierignore`, `.github/linters/.yaml-lint.yml` (no runner) | FIX | Defect CI:D-18: configuration that nothing runs. Scope 3.1: workflow, YAML, shell, Dockerfile and Markdown linting are gates (NEW-20 covers workflow, shell and Dockerfile). Generated snapshots stay excluded. |
| CAP-41, CAP-TQ-31, CAP-DM-33, CAP-CI-33 | A guard fails when a protected quality-configuration file is modified or a required directory is missing. | `scripts/validate-configuration.sh` (`make validate-configuration`): 5 locked files, 14 required directories | FIX | Defects TQ:42, DM:D-11, CI:D-05, AD:D-AD-02, AD:D-AD-03: no workflow runs it, it is blind on shallow checkouts, the lock is absolute with no sanctioned change path, and the list is incomplete and disagrees with the guidance. Scope 3.1: the protected-configuration guard runs in CI; architecture rules are structural so a new context needs no edit to a locked file. One protected list shared by guard, guidance and assistant settings (NEW-31). |

### 8. Dependency and security checks

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-CI-14 | The dependency manifest and lockfile are validated. | `composer validate` (`make composer-validate`, W-17) | KEEP | Analyst decision (scope silent). Rust: manifest and lockfile consistency checked by locked builds. Avoid CI:D-15, TQ:27 (a release-written field that makes validation warn, and a test that requires the warning). |
| CAP-TQ-30, CAP-CI-16 | Locked dependencies are audited for known vulnerabilities, and any advisory fails the build. | `symfony security:check` (`make check-security`, W-17, pre-commit) | EXTEND | Scope 3.1 asks for more: advisories plus licence policy plus source policy, an unused-dependency check (NEW-17) and secrets scanning (NEW-18). CAP-TQ-30 also names manifest validation (CAP-CI-14) and the platform check (CAP-CI-15). |
| CAP-TQ-21, CAP-CI-10 | Security-focused static analysis fails the build on findings and publishes them to the repository's code-scanning view. | `psalm --taint-analysis --report=results.sarif` + `github/codeql-action/upload-sarif` (W-14) | FIX | Analyst decision. Defect CI:D-04: the upload is skipped exactly when findings exist, and no `security-events` permission is declared. Scope names no taint tool; the intent is met by the scoped security gates (strict lints, advisories, secrets scan, image scan NEW-19). Publishing findings in SARIF, also on failure, is kept where a scoped gate can emit it; the architect names the emitter. |
| CAP-DM-05 | Third-party binaries and tool images are pinned by tag or digest and verified. | Pinned tags, two sha256 digests, GPG-verified Xdebug tag | FIX | Defects DM:D-31, DM:D-24: unpinned base image, unpinned load-generator core, `latest` downloads. Scope section 6, P26 (hygiene, in scope): base and tool images pinned; actions pinned by commit SHA (NEW-27). LocalStack stays on the tokenless community tag the research verified (scope 3.1). |

### 9. Unit and integration tests

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-TQ-02 | The unit suite runs with no database, broker, cache or network. | PHPUnit suite `Unit`, mocks | KEEP | Scope 3.1 gate "unit tests". |
| CAP-TQ-03 | Any collaborator can be replaced by a test double. | `DG\BypassFinals` strips `final` at load time; PHPUnit mocks of concrete classes | FIX | Defect TQ:24: substitutability depends on a load-time hack. Scope 3.1 puts every external dependency behind a port (repository, cache, broker client, buses); doubles replace ports in unit tests and at composition time. |
| CAP-TQ-04 | Each test level has shared test support with a random data generator. | `UnitTestCase`, `IntegrationTestCase`, unseeded Faker | FIX | Defect TQ:19 and audit P10. Scope 3.1: randomised test data is seeded and the seed is printed on failure. |
| CAP-TQ-05 | Every production unit has its tests at a predictable location. | `tests/<Level>/` mirrors `src/`, class `<Unit>Test` | KEEP | Analyst decision (scope silent). Rust idiom: unit tests beside the code; integration and end-to-end tests in a tree that follows the bounded-context paths. Exact layout: architect. Avoid TQ:21 (tests filed under the wrong layer). |
| CAP-TQ-06, CAP-CI-03 | The integration suite runs against real backing services, locally and in CI. | `KernelTestCase::getContainer()`, PostgreSQL 16 database `app_test`, LocalStack 3.4.0 (W-20) | KEEP | Scope 3.1: integration tests against real PostgreSQL and LocalStack. |
| CAP-TQ-07, CAP-DM-29 | One command drops, recreates and migrates the test database, and a second does the same for the load-test stack. | `make setup-test-db`, `make setup-load-test-db` | KEEP | Scope principle 2: capability-named targets keep their names. The load-test variant must act only on the isolated load-test stack (CAP-DM-38). |
| CAP-TQ-08 | Tests run under a dedicated configuration that never touches development data. | `APP_ENV=test`, `.env.test`, `services_test.yaml`, `when@test` | KEEP | Analyst decision (scope silent on a test profile). Rust: tests build their own configuration and substitute ports by composition, not through a runtime profile. The env-file leftovers (APP:D-34, TQ:38) are not carried. |
| CAP-TQ-14 | Each dependency probe is verified at unit level and at integration level. | Three `*CheckSubscriberTest` pairs | FIX | Defects TQ:12, TQ:15, TQ:16: integration assertions observe the test's own calls, the same assertion appears six times, the cache "integration" test is a unit test. Scope principle 4. Rust: unit tests per probe with fakes (including the timeout path) and integration tests for the real PostgreSQL and SQS adapters. |
| CAP-24, CAP-TQ-15 | A data-fixture loader and a command rebuild the schema and load fixtures. | DoctrineFixturesBundle (dev, test), `tests/DataFixtures/AppFixtures.php` (loads nothing), `make load-fixtures` | DROP | Analyst decision. Scope principles 5 and 7: no dead scaffolding and nothing outside the listed scope; the PHP fixture loads nothing (TQ:20). Tests create their own data. |

### 10. End-to-end tests

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-TQ-09, CAP-CI-05 | Acceptance tests written in Gherkin drive the HTTP entry point, locally and in CI. | Behat 3 + SymfonyExtension, in-process kernel requests (W-01) | FIX | Defect TQ:9: both suites run every feature and undefined steps pass. Scope 3.1: Gherkin end-to-end tests reusing the PHP health feature; P10 strict tests (undefined or pending steps fail). In-process or over a socket: architect. |
| CAP-TQ-11 | For each dependency a scenario injects a failure and expects `500` with that dependency's message. | Container service replaced by a throwing mock inside the Behat context | KEEP | Scope 3.1: `features/health_check.feature` is reused. Fault injection by composing the application with a failing port implementation, which avoids TQ:11. |
| CAP-TQ-12 | A fault injected in one scenario does not leak into the next. | Kernel rebuilt per scenario by the Behat extension (inferred) | KEEP | Required for the reused feature (scope 3.1). Each scenario builds its own application instance. |
| CAP-TQ-13 | A smoke test checks that the API documentation endpoint answers `200`. | `features/demo.feature` ("the response should be received") | FIX | Audit P10 (in scope): the placeholder scenario is replaced by a real assertion on `/api/docs` (status and content). |

### 11. Coverage and mutation testing

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-TQ-16, CAP-CI-04 | Coverage over all production source from unit and integration runs is reported in machine-readable and HTML form and published to an external coverage service. | Xdebug + PHPUnit clover and HTML, `processUncoveredFiles`, Codecov action with `fail_ci_if_error: true` (W-05, W-20) | FIX | Defects TQ:3, CI:D-07 and audit P9: no threshold in the repository; two workflows upload the same data. Scope 3.1: in-repository floor of 100% lines for testable code; one coverage job. Analyst decision: publication to the external service stays as a non-gating step that needs `CODECOV_TOKEN` (scope silent on the service). |
| CAP-DM-32 | Coverage commands write machine-readable and HTML reports to known paths. | `tests-with-coverage`, `coverage-xml`, `coverage-html` | FIX | Defects DM:D-15, DM:D-37, DM:D-39: two output conventions (one at the container root), outputs not git-ignored, root-owned files. Scope principle 2 keeps the target names. One output directory inside the project, ignored, owned by the invoking user. |
| CAP-TQ-17, CAP-CI-07 | Mutation testing mutates production source, judges it with the test suite in parallel and writes logs. | Infection 0.27, `--testsuite=Unit -j8` (W-09) | FIX | Defects TQ:2, CI:D-02 and audit P8: no minimum score, so the job cannot fail on escaped mutants. Scope 3.1: mutation testing fails on surviving mutants. |

### 12. Makefile command surface and its tests

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-DM-24 | The task runner documents itself: with no argument it prints usage and every documented target. | `.DEFAULT_GOAL = help`, `## ` comments | KEEP | Scope 3.1: `help` is the default goal. Avoid AD:D-AD-17, DM:D-28 (targets hidden from help, stale texts). |
| CAP-DM-25 | One named command exists per quality gate, test level, contract tool and maintenance action, and CI, hooks, documentation and assistant guidance all call it. | 68 Make targets | KEEP | Scope 3.1: the Makefile is the single command surface; capability-named targets keep their names, tool-named targets get capability names, the mapping is documented (NEW-36). See "Make target names" under "Contracts to preserve". |
| CAP-TQ-01 | One command exists per test level, plus an aggregate. | `unit-tests`, `integration-tests`, `behat`, `bats`, `all-tests` (alias `test`) | KEEP | Scope 3.1 naming rule: `behat` is tool-named and gets a capability name; the others are kept. |
| CAP-DM-23 | Stack lifecycle commands start, stop, remove and rebuild the stack, open a shell, and follow all logs or only new logs. | `start` / `up`, `stop`, `down`, `build`, `sh`, `logs`, `new-logs` | KEEP | Scope principle 2: names kept. Avoid DM:D-20 (no incremental rebuild). |
| CAP-DM-30 | Dependency management commands install from the lock, update, validate, check requirements and audit. | `install`, `update`, `composer-validate`, `check-requirements`, `check-security` | KEEP | Scope 3.1 naming rule: `composer-validate` is tool-named and gets a capability name. The checks behind the commands are CAP-CI-14, CAP-CI-15 and CAP-TQ-30. |
| CAP-DM-31 | Migration and maintenance commands exist. | `doctrine-migrations-migrate`, `doctrine-migrations-generate`, `cache-clear`, `cache-warmup`, `purge`, `load-fixtures`, `commands` | KEEP | Carried under capability names: apply migrations, create a migration, `purge` / `clean`, plus rollback (NEW-13). Analyst decision, not carried: `cache-clear` and `cache-warmup` (framework cache, no Rust meaning), `load-fixtures` (CAP-24), `commands` (the binary's own help output). |
| CAP-DM-27 | Quality and test commands run under the test configuration without the caller setting it. | `docker compose exec -e APP_ENV=test` | FIX | Defect DM:D-13: the Makefile includes the test env file wholesale, which mangles values and overrides the caller's environment. Scope principle 3. Rust: each target sets the configuration it needs; the Makefile imports no env file. |
| CAP-TQ-34, CAP-DM-26 | Every command works both by executing inside the development container and by running directly. | `EXEC_ENV`, `ifeq ($(CI),1)` | FIX | Defects TQ:39, DM:D-12: only `CI=1` switches, GitHub's `CI=true` does not, and 17 targets honour it. Scope 5: targets work in the development container, on a host with the pinned toolchain and on a runner; the switch is explicit, honours `CI=true` and is covered by tests. Default and variable name: architect. |
| CAP-TQ-32, CAP-DM-28, CAP-CI-32 | One aggregate local command runs all gates and prints a success line, and conventional aliases exist. | `make ci` (`all`, `ci-sequential`), `test`, `clean` | FIX | Defects TQ:1, DM:D-01, CI:D-24: it cannot pass at HEAD, nothing runs it, and it differs from hosted CI. Scope principle 4 and 3.1: `make ci` passes on a clean checkout, CI runs it or its parts, and it is the one canonical verification list. |
| CAP-TQ-18, CAP-CI-06 | Every developer-facing command is executed by an automated test that checks exit status and output, locally and in CI. | 38 positive Bats tests (W-03); helper libraries installed by an action | FIX | Defects TQ:7, TQ:25, TQ:26, TQ:28, TQ:29, TQ:30, TQ:31, TQ:32: helper libraries not obtainable (broken submodules), assertions on echoed recipe text, wrong artefact checked, order dependence. Scope 3.1 and 4: Bats tests of the Makefile; helper libraries provisioned without submodules. |
| CAP-TQ-19 | For each gate a negative test plants a bad artefact and proves that the gate fails. | 12 negative Bats tests with fixtures under `tests/CLI/bats/php/` | FIX | Defects TQ:14, TQ:30, TQ:2: duplicated test, no teardown, the mutation "should fail" test does not assert failure. Scope 3.1: negative tests included. Compiled-language redesign: fixtures quarantined from build and analysis, with guaranteed restore. |
| CAP-CI-36 | Ignore rules cover local environment files, tool caches and generated output. | `.gitignore`, `tests/Load/results/.gitignore` | FIX | Defects CI:D-32, TQ:40, DM:D-37: an ignored file is tracked; coverage, mutation, SARIF and load artefacts are not ignored. Scope section 6, P26. Every command that writes into the working tree gets a defined path and an ignore rule. |

### 13. Contract artifacts and contract checks

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-16, CAP-CI-19 | An OpenAPI 3.1 document is generated from the application's own definitions, carries API metadata and documented responses, and is exported by CLI to a versioned file. | API Platform OpenAPI factory, `api:openapi:export`, `.github/openapi-spec/spec.yaml` | FIX | Defects APP:D-20, APP:D-22, CI:D-26, CI:D-10: schema without properties, unused schema, no error schema, static `info.version`, snapshot can drift. Scope 3.1: code-first OpenAPI 3.1, same snapshot path, deterministic export, CI fails when the committed snapshot differs from the generated one. Scope 4: the `info.version` source is the architect's decision. Research: stay on 3.1.0. |
| CAP-17 | The generated OpenAPI document carries human-readable tag descriptions. | `OpenApiFactory` service decorator | KEEP | Analyst decision (scope silent). The Spectral ruleset requires tag descriptions. Rust: declared in the code-first document; no post-processing hook is needed. |
| CAP-DM-34 | Commands export the REST contract and the GraphQL schema to versioned files. | `make generate-openapi-spec`, `make generate-graphql-spec` | KEEP | Scope principle 2 and 3.1: target names and snapshot paths kept. Exports need no running stack (research). Both must appear in `make help` (AD:D-AD-17). |
| CAP-CI-23 | The GraphQL schema snapshot is generated from code and kept current at a fixed path. | `.github/graphql-spec/spec`; CI auto-commit to the PR branch (W-08) | FIX | Defect CI:D-09: a bot commit on the PR branch with a `feat` message, unsupported for fork PRs. Scope 3.1: no bot commits; CI fails when the committed snapshot differs from the generated one. |
| CAP-DM-35, CAP-CI-20 | The REST contract is linted against a house ruleset with zero tolerance. | Spectral 6.15.1, `.spectral.yaml`, `--fail-severity=hint` (`make validate-openapi-spec`, W-12) | KEEP | Scope 3.1: OpenAPI lint with the PHP Spectral ruleset. Research: the Rust document passes it unchanged. The ruleset weakness CI:D-28 (a bare `default` satisfies both response rules) is recorded here so it is not inherited unnoticed; the architect decides whether to tighten. |
| CAP-DM-36, CAP-CI-21 | The REST contract of a change is compared with the default branch for backward compatibility. | `openapitools/openapi-diff:2.1.2`, `make openapi-diff base_ref=` (W-11) | FIX | Defects DM:D-10, CI:D-01 and audit P24: no fail flag, so a breaking change passes. Scope 3.1: the breaking-change check fails unless an approval label is present (same label as GraphQL). |
| CAP-CI-24 | GraphQL breaking changes against the default branch are detected, and a label approves an intentional break. | GraphQL Inspector, label `approved-breaking-change` (W-08) | KEEP | Scope 3.1: GraphQL schema diff with the same label. Do not repeat CI:D-09 (deprecated action runtime, write permission on PR workflows, typos). |
| CAP-DM-37, CAP-CI-22 | Contract-driven conformance and fuzz tests run against the running service. | Schemathesis `--checks all --phases=examples,coverage` (`make schemathesis-validate`, W-16) | FIX | Defects DM:D-07, DM:D-14, CI:D-29: localhost detection misses `127.0.0.1`; the database it prepares is not the one the server uses. Scope 3.1 and 5: Schemathesis against the running service built from the production image. Research: the PHP flags pass on the Rust output. |

### 14. Load testing

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-DM-38 | Load tests run against a production-build stack that one command builds, starts, health-gates, initialises and always tears down. | `RUN_LOAD_TEST` wrapper over `docker-compose.prod.yml` + `docker-compose.load_test.override.yml` | FIX | Defect DM:D-03: the stack shares project name, volumes and ports with the development stack and drops the development database. Scope 3.1: an isolated load-test stack that cannot touch development data. |
| CAP-DM-39 | Four standard load profiles per endpoint (smoke, average, stress, spike) run one by one or in a single run. | K6 `constant-arrival-rate` / `ramping-arrival-rate`, `delayBetweenScenarios` | KEEP | Scope 3.1: K6 kit ported with the four profiles. |
| CAP-DM-40 | Per-endpoint load configuration is declarative in one JSON file, with a local override file and environment overrides for host and port. | `config.json.dist`, optional `config.json`, `API_HOST`, `API_PORT` | KEEP | Scope 3.1: kit ported from PHP. The local override file must be git-ignored (DM:D-37). |
| CAP-DM-41 | Performance gates pass or fail a run: p99 latency per profile and a check success rate above 99%. | K6 thresholds built from `config.json.dist` | FIX | Audit P19 and defect DM:D-25: the only CI-enforced budget is smoke p99 < 9000 ms, which cannot catch a regression. Scope 3.1: budgets suited to a compiled service. Values: architect. |
| CAP-DM-42 | Adding one scenario file adds an endpoint to every runner. | `get-load-test-scenarios.sh` | KEEP | Scope 3.1: kit ported from PHP. |
| CAP-DM-43 | A single scenario can be run with per-profile switches. | `make execute-load-tests-script scenario=.. runSmoke=..` | FIX | Defect DM:D-18: without `scenario=` the arguments shift, no stack is started, and its test asserts nothing. Scope principle 3; the fix is local to the scripts. |
| CAP-DM-44 | Each run writes one self-contained HTML report to an ignored results directory and prints a console summary. | xk6-dashboard export to `tests/Load/results/`, `--summary-trend-stats` | EXTEND | Scope 3.1 asks for more: results uploaded as CI artifacts (PHP uploads nothing, DM:D-40). |
| CAP-DM-45 | The load generator is a locally built container image, so no host installation is needed. | `tests/Load/Dockerfile`, image `k6` | FIX | Defect DM:D-31: unused extensions, unpinned core and base image, generic tag, rebuilt on every run. Scope principles 3 and 5: pin and trim. |
| CAP-DM-46 | A baseline scenario for the health endpoint asserts the documented success status. | `scripts/health.js`, `GET /api/health` expects `204` | KEEP | Scope 3.1: health contract unchanged (CAP-07). The probe behind it no longer creates a queue per request (CAP-11). |
| CAP-DM-47, CAP-CI-18 | The load-test scripts are statically analysed. | ESLint via pnpm, workflow `eslint.yaml` (path-filtered, cached) | FIX | Analyst decision. Defect DM:D-33: configuration copied from a frontend project, no Make target, CI-only. Scope's gate list does not name JavaScript lint, but scope keeps Dependabot for the K6 npm package; kept as a gate with a minimal configuration and a Make target (scope 3.1: each gate is a Make target and a CI check). |
| CAP-CI-17 | A smoke load test runs on pull requests against the production build with latency and success-rate thresholds. | W-10: `make smoke-load-tests`, `API_HOST=localhost`, `API_PORT=8081` | EXTEND | Scope 3.1 asks for more: smoke on pull requests plus uploaded results and realistic budgets (P19). |
| CAP-DM-48 | A cloud load run provisions temporary infrastructure with one command, runs the tests on an instance, uploads reports and terminates, and a second command removes everything. | `make aws-load-tests`, `make aws-load-tests-cleanup` | FIX | Defects DM:D-04, DM:D-05, DM:D-06, DM:D-23, DM:D-24: unrendered IAM policy, compose command mismatch, no failure path, clone URL hard-coded to the template. Scope 3.1: AWS runner ported; known AWS-flow defects fixed where the fix is local to the scripts. |
| CAP-DM-49 | The cloud run is parameterised by region, image, instance type, tag, role, branch and security group. | `getopts` in `tests/Load/config.sh` | FIX | Defect DM:D-22: options cannot be passed through Make and an advertised option does not exist. Scope 3.1: local script fix. |
| CAP-DM-50 | The cloud provisioning script can be dry-run offline against the local emulator, and a CLI test covers it. | `LOCAL_MODE_ENV=true`, LocalStack `ec2,s3,iam,sts` | KEEP | Scope 3.1: AWS runner and Bats tests ported. Requires those emulator services in the development stack. Correct DM:D-23 (cleanup ignores local mode). |

### 15. Containers and compose stacks

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-DM-01 | One multi-stage image definition has a shared base and separate development and production targets. | `Dockerfile` stages `frankenphp_base`, `frankenphp_dev`, `frankenphp_prod` | KEEP | Scope 3.1: multi-stage Dockerfile with a development target and a production target. |
| CAP-DM-02 | The production image is minimal and runs as a non-root user. | `USER www-data`, `composer install --no-dev`, `app.prod.ini` | EXTEND | Scope 3.1 asks for more: minimal, non-root, hardened production target (P6). Image vulnerability scan: NEW-19. |
| CAP-DM-03 | The development image carries the full toolchain needed for every gate and Make target. | Xdebug, `bash`, `bats`, `make`, Symfony CLI, Composer | KEEP | Scope 3.1 and 5: development target; targets run inside the development container. Toolchain pinned by `rust-toolchain.toml` (NEW-24). Avoid TQ:32 (a CLI test runner shipped where it cannot run). |
| CAP-DM-04 | Dependencies are cached in an image layer separate from source. | Copy manifests, install, copy source | KEEP | Implementation detail of the in-scope Dockerfile (scope 3.1; build caching, P17). Mechanism: architect. |
| CAP-DM-06 | The image build context is lean. | `.dockerignore` | KEEP | Implementation detail of the in-scope Dockerfile (scope 3.1). |
| CAP-TQ-35 | Test tooling and development files stay out of the production image. | `.dockerignore` excludes `tests/`, `.env.test`, `*.dist`; development-only stage | FIX | Defect DM:D-34: Makefile, scripts and tool configuration are present in the PHP production image. Scope 3.1: minimal production target. |
| CAP-DM-07 | An image-level health check calls the liveness path and allows a start grace period. | `HEALTHCHECK` and compose: `curl -fsS http://localhost:8081/ping`, 10 s / 3 s / 3 retries / 180 s | KEEP | Scope 3.1: `/ping` is the container healthcheck contract. Probe command and timings chosen deliberately for a minimal image and a compiled service (architect). |
| CAP-DM-08 | Start-up waits for the database with a bounded retry and exits non-zero with the error when it never becomes reachable. | `docker-entrypoint.sh`: 60 tries, 1 s apart | KEEP | Analyst decision (scope silent). A bounded wait with a clear error, placed in the migrate step or the connection set-up (architect); explicit timeouts (NEW-06). |
| CAP-DM-10 | The development container never runs with dependencies older than the lock file. | Timestamp comparison in the entrypoint, container-managed `vendor` volume | KEEP | Analyst decision (scope silent). Met by construction: the build tool resolves the lock file on every build; needs persistent dependency and build caches in the development stack. |
| CAP-DM-11 | Source changes reload the service automatically in local development, and not in production or CI. | FrankenPHP `--watch` unless `APP_ENV=prod` or `CI` is set | KEEP | Analyst decision (scope silent). A compiled service needs watch-and-rebuild in the development target; mechanism and cost are the architect's call, and if it is not delivered it must be recorded as a non-goal with a reason. |
| CAP-DM-12 | Arguments given to the container are passed to the service command. | First block of `docker-entrypoint.sh` | KEEP | Analyst decision (scope silent). Met by construction when the image entry point is the service binary; this is how `migrate` and `check-config` run in a container. |
| CAP-DM-13, CAP-CI-01 | One command boots the complete local stack in the background and returns only when it is healthy, and CI uses the same command. | `make start` -> `docker compose up --detach --wait` | KEEP | Scope principle 2: name kept. The "every check is a Make target" half of CAP-CI-01 is CAP-DM-25. |
| CAP-DM-14 | The local stack consists of the application, PostgreSQL with a persistent volume and readiness probe, an AWS-API emulator and an architecture-diagram viewer. | `php`, `database` (PostgreSQL 16), `localstack` 3.4.0, `structurizr` lite | KEEP | Scope 3.1: PostgreSQL 16, LocalStack pinned to the tokenless community tag the research verified, Structurizr Lite stays in the development stack. Avoid DM:D-35 (empty init hook) and DM:D-08. |
| CAP-DM-15 | The application starts only after the database is ready. | `depends_on: condition: service_healthy` | KEEP | Implementation detail of the in-scope compose files (scope 3.1). |
| CAP-DM-16 | Every published port and the database credentials are configurable through one committed env file with working defaults. | `.env` + `${VAR:-default}` | FIX | Defects DM:D-09, DM:D-08: the file is tracked and ignored at once and carries secret-shaped defaults; the database healthcheck ignores the configured names. Scope 4: `DB_VERSION`, `DB_NAME`, `DB_USER`, `DB_PASSWORD`, `DB_PORT`, `LOCALSTACK_PORT`, `STRUCTURIZR_PORT`, `APP_ENV` keep their names; local-only defaults are labelled. |
| CAP-DM-17 | Host source edits are visible in the development container without rebuilding the image. | Bind mount `./:/srv/app`, anonymous `vendor` volume | KEEP | Needed for scope 5 (targets run inside the development container). Build output and dependency caches stay container-managed. Avoid DM:D-39 (root-owned files on the host). |
| CAP-DM-21 | A production compose file forces production mode and requires real secrets. | `docker-compose.prod.yml` | EXTEND | Scope 3.1 asks for more: a production compose reference with resource limits, a restart policy, read-only root filesystem, dropped capabilities and no-new-privileges (P6, P15). |
| CAP-DM-22 | The debugger in the container can connect to the host IDE. | `extra_hosts: host.docker.internal:host-gateway`, `xdebug.client_host` | DROP | Analyst decision. Xdebug-specific call-back model with no Rust meaning; the dev container definition (NEW-40) covers in-container debugging. |

### 16. CI workflows

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-TQ-33, CAP-CI-02 | Each gate is a separately attributable status check on every pull request to the default branch. | 16 always-on PR workflows plus 1 path-filtered, one job each | KEEP | Scope 3.1: each gate is a Make target and a CI check. How jobs are grouped to share a build is the architect's decision; the hygiene additions are NEW-25 to NEW-29. CI:D-06 (names hard-coded to the template) is avoided through NEW-23. |
| CAP-TQ-27, CAP-CI-13 | Formatting and refactoring fixes are committed back to the default branch by a bot. | `code-lint-and-prettify.yml`, `GitHub Actions Bot`, optional GPG signing | DROP | Scope 3.1: "No bot commits to pull-request branches or to `main`, other than the release identity" (P12). Replaced by the failing format check (CAP-TQ-25). Secrets `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE` are no longer needed. |
| CAP-CI-30 | Automated AI code review is configured to request changes. | `.coderabbit.yaml` (`request_changes_workflow: true`) | KEEP | Scope 3.1: CodeRabbit config. Depends on an external App installation (CI:D-33). |

### 17. Release, dependency updates and template sync

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-CI-25, CAP-AD-29 | A merge to the default branch bumps the version from conventional commits, regenerates the changelog, writes the version to the manifest, pushes a release commit and a `v` tag, and creates a hosted release without a CI loop. | `autorelease.yml`: `TriPSs/conventional-changelog-action`, `actions/create-release`, GitHub App token, `[skip ci]` | KEEP | Scope 3.1: same flow and secrets as PHP. Rust specifics: version of record in `Cargo.toml`, mirrored in `Cargo.lock`. Do not repeat CI:D-14, AD:D-AD-27, AD:D-AD-31 (archived release action, silent five-release truncation, failure on every push when the App secrets are absent). |
| CAP-CI-28 | Dependency-update pull requests are opened automatically, grouped per ecosystem, with commit prefixes compatible with the release convention. | `.github/dependabot.yml`: composer weekly, npm in `/tests/Load` monthly | EXTEND | Scope 3.1 asks for more: cargo, GitHub Actions, Docker and the K6 npm package (P18). Also correct CI:D-12 (the `feat` prefix cuts a MINOR release per bump) and CI:D-25 (one person hard-coded as reviewer). |
| CAP-CI-27 | Downstream repositories receive template changes as scheduled, labelled, self-updating pull requests, with two authentication options. | `tempate-sync-pat.yml`, `template-sync-app.yml`, `AndreasAugustin/actions-template-sync`, label `template_sync` | FIX | Defect CI:D-13: file-name typo, no ignore list, runs in the template against itself, source hard-coded to the PHP template. Scope 3.1: both workflows with the typo fixed, a sync ignore list, and a guard so they do nothing in the template repository itself. |
| CAP-CI-35, CAP-AD-32 | Setup documentation covers the release and template-sync automation: credentials, permissions and how a downstream repository receives updates. | `.github/AUTORELEASE.md`, `.github/TEMPLATE_SYNC_APP.md`, `.github/TEMPLATE_SYNC_PAT.md`, README section | FIX | Defects CI:D-13, CI:D-14: wrong flavour label, options missing from the examples, App permission list incomplete for workflow files and broader than needed elsewhere. Scope 3.1 keeps release and both sync flows, so their guides ship. |

### 18. Git hooks and commit convention

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-TQ-28, CAP-CI-26, CAP-AD-27 | Commit messages follow `type(#issue): subject` with a fixed type list, and the convention drives semantic versioning and is documented for contributors. | CaptainHook `commit-msg` regex and `PrepareConventionalCommit` prompt; `.github/AUTORELEASE.md`, `CONTRIBUTING.md` | FIX | Defects TQ:8, CI:D-14, AD:D-AD-16: enforced only by a local hook; bots and history violate it; the prompt builds messages the regex rejects. Scope 3.1: the VilnaCRM form is kept and enforced by a local hook and by a pull-request title check in CI, bots exempt (P12). |
| CAP-TQ-29 | A pre-commit pipeline runs quality checks before a commit is accepted. | CaptainHook `pre-commit`: 11 actions inside the development container | FIX | Analyst decision (scope names only the commit-message hook). Defect TQ:44: integration and acceptance tests and a network audit on every commit, the stack must be running, the architecture check is missing. Kept as a fast subset of Make targets; content is the architect's decision. |

### 19. AI-assistant guidance

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-AD-01 | A cross-tool agent entry file at the repository root states project identity, the mandatory reading order, working rules, the verification baseline and the skill inventory. | `AGENTS.md` (64 lines) | FIX | Defects AD:D-AD-04, AD:D-AD-05: five diverging command lists; skill list kept in four places. Scope 3.1: `AGENTS.md` with one canonical verification command list equal to `make ci`. |
| CAP-AD-02 | A tool-specific entry file only delegates to the cross-tool file and adds a project snapshot and a command reference. | `CLAUDE.md` (49 lines) | KEEP | Scope 3.1: `CLAUDE.md`. Must coexist with the BMAD-generated content already in the Rust repository's `CLAUDE.md` (NEW-32). |
| CAP-AD-03 | A skills directory provides an index, a usage guide and a decision guide. | `.claude/skills/README.md`, `AI-AGENT-GUIDE.md`, `SKILL-DECISION-GUIDE.md` | KEEP | Scope 3.1: `.claude/skills/` with index, usage guide and decision guide. |
| CAP-AD-04 | Task skills are small Markdown files with `name` and `description` metadata that any assistant can use. | Ten `SKILL.md` files, 33 to 63 lines, two-key frontmatter | FIX | Defects AD:D-AD-06, AD:D-AD-07: descriptions say what a skill is, not when to use it; the template name is baked into files meant for downstream sync. Scope 3.1 keeps the skills. |
| CAP-AD-05 | A decision tree maps each task intent to one primary skill and lists multi-skill scenarios and non-negotiable rules. | `SKILL-DECISION-GUIDE.md`: ten intents, four scenarios, three rules | KEEP | Scope 3.1: decision guide. Intents follow the Rust skill set. Do not state a rule the repository itself breaks (AD:D-AD-10). |
| CAP-AD-06 | Standard flows (code change, API change, architecture change) tell an agent which skills to chain. | `AI-AGENT-GUIDE.md` "Suggested Flow" | KEEP | Scope 3.1: usage guide. |
| CAP-AD-07 | A skill covers adding or extending an API resource end to end with a framework-free domain and regenerated contracts. | `api-platform-crud` | FIX | Defects AD:D-AD-11, AD:D-AD-12: it points at a directory that does not exist and at patterns with no exemplar. Scope 3.1: a Rust counterpart named by capability; every pattern it names exists in the example slice. |
| CAP-AD-08 | A skill defines the full ordered verification stack, routes each failing check to a skill and defines an honest close-out. | `ci-workflow` | FIX | Defects AD:D-AD-04, AD:D-AD-08: its list is not the CI list and the close-out wording is ambiguous. Scope 3.1: one list, equal to `make ci`. |
| CAP-AD-09 | A skill quotes the numeric maintainability thresholds and gives refactoring guidance instead of threshold changes. | `complexity-management` (100 / 95 / 100 / 100) | KEEP | Scope 3.1: Rust counterpart for each of the ten skills. Thresholds re-derived from the Rust gates; numbers live in one place (AD:D-AD-13). |
| CAP-AD-10 | A skill explains how to fix architecture-boundary violations by moving code and catalogues common violation patterns. | `deptrac-fixer` | KEEP | Scope 3.1: Rust counterpart named by capability; patterns re-derived for the Rust gate. |
| CAP-AD-11 | A skill enumerates documentation surfaces, the triggers for updating them and typical sync patterns. | `documentation-sync` (eight surfaces, five triggers) | FIX | Defects AD:D-AD-09, AD:D-AD-15: a manual checklist with an incomplete surface list. Scope 3.1: backed by the documentation-drift check (NEW-30). |
| CAP-AD-12 | A skill covers adding and running load-test scenarios, smoke first. | `load-testing` | KEEP | Scope 3.1: Rust counterpart for each skill. Portable while the kit layout is kept. |
| CAP-AD-13 | A skill keeps the generated contract snapshots current and in the same change as the code. | `openapi-development` | FIX | Defect AD:D-AD-14: it covers export only. Scope 3.1 gates to cover: lint, breaking-change diff, conformance and snapshot equality. |
| CAP-AD-14 | A reference skill lists every protected quality gate with its value and command. | `quality-standards` | FIX | Defect AD:D-AD-13: incomplete, and it duplicates numbers kept elsewhere. Scope 3.1: Rust counterpart listing all gates. |
| CAP-AD-15 | A skill gives criteria for when the architecture model must be updated. | `structurizr-architecture-sync` | FIX | Defect AD:D-AD-09: no executable verification step. Scope 3.1: cites the model validation run in CI (NEW-16). |
| CAP-AD-16 | A skill maps test surfaces to commands and gives risk-tiered run orders, debugging notes and done criteria. | `testing-workflow` | KEEP | Scope 3.1: Rust counterpart for each skill. Suite and command names re-derived. |
| CAP-AD-17 | The command policy is wrapper-first: use the repository's commands, not raw tool invocations. | "Use `make` targets first ... `docker compose exec php ...`"; "never the host toolchain" | FIX | Defect AD:D-AD-10 and scope 5: targets also run on a host with the pinned toolchain, so the "never on the host" clause is replaced by "always through `make`". |
| CAP-AD-18, CAP-TQ-36 | Guidance tells assistants which gate commands to run and forbids weakening quality configuration to obtain a pass. | Repeated in `AGENTS.md`, `CLAUDE.md`, the guide and four skills; guard script local only | FIX | Defects AD:D-AD-01, AD:D-AD-02: the policy is unenforced and the protected lists disagree. Scope 3.1: backed by the guard in CI (row CAP-41) and the assistant settings guard (NEW-31). |
| CAP-AD-19 | The domain-purity policy is stated in guidance and backed by the automated architecture check. | "Keep Domain code framework-free" + `make deptrac` | KEEP | Scope 3.1: Domain is framework-free. Backed by CAP-38. |
| CAP-AD-20 | Contract snapshots and the architecture model change in the same commit as the code they describe. | `.github/openapi-spec/spec.yaml`, `.github/graphql-spec/spec`, `workspace.dsl`; discipline only for OpenAPI | FIX | Defects CI:D-10, AD:D-AD-09: nothing enforces it. Scope 3.1: enforced by snapshot equality in CI (CAP-16, CAP-CI-23) and model validation (NEW-16). |
| CAP-AD-21 | An agent states what was not run and why before claiming completion. | `ci-workflow` "Minimum Close-Out", `testing-workflow` "Done Criteria" | KEEP | Part of the verification skill counterpart (scope 3.1). Reword without the ambiguity of AD:D-AD-08. |
| CAP-AD-22 | Guidance avoids service-specific assumptions so it can be synced into downstream services. | `.claude/skills/README.md` "Scope" | FIX | Defect AD:D-AD-07: the template's own name appears in synced files. Scope 3.1 keeps template sync, so the guidance must stay template-safe. |
| CAP-AD-23 | A recommended prompt pattern tells humans how to instruct an assistant. | README "How to use them well" | KEEP | Analyst decision (scope silent); a few lines in the README. |

### 20. Human documentation

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-AD-24 | A README explains the purpose, the baseline, a one-command quick start with entry-point URLs, the AI workflow, local commands, the CI model, downstream sync, cloud load testing, documentation links and the licence. | `README.md` (171 lines) | FIX | Defects AD:D-AD-19, AD:D-AD-22, AD:D-AD-23: false promises, key documentation on an external wiki, first run under-documented. Scope 3.1: README plus in-repository documents (NEW-33 to NEW-38). |
| CAP-AD-25 | A badge row signals code health, coverage and analysis scores, preceded by a solidarity banner. | Eight badges (six static), StandWithUkraine banner | FIX | Analyst decision (scope silent). Defect AD:D-AD-20: hard-coded values that disagree with the gates. Only dynamic badges backed by something the repository configures; the banner is kept as an organisation convention. |
| CAP-AD-26 | A contributing guide covers issue, branch, commit convention, pull request, review and merge. | `CONTRIBUTING.md` (70 lines) | FIX | Defect AD:D-AD-25: generic boilerplate with errors and no project rules. Scope 3.1: CONTRIBUTING. |
| CAP-AD-28 | A security policy gives a private disclosure channel. | `SECURITY.md` (7 lines, `TBD`) | FIX | Defect AD:D-AD-26 and audit P13. Scope 3.1: a complete SECURITY policy. |
| CAP-AD-30 | A public-domain dedication is the licence and is stated consistently. | CC0 1.0 `LICENSE`; manifest says `proprietary` | FIX | Defects AD:D-AD-24, APP:D-33 and audit P26. Scope 3.1 and 4: one licence statement (`CC0-1.0`) in manifest, `LICENSE`, README and API description. |
| CAP-CI-29, CAP-AD-31 | Issue templates and a pull-request checklist carry the process rules. | `.github/ISSUE_TEMPLATE/*.md` (labels `bug`, `enhancement`), `.github/pull_request_template.md` | FIX | Defects CI:D-31, AD:D-AD-32: frontend-flavoured fields and links hard-coded to the PHP template. Scope 3.1: backend-appropriate issue templates. |
| CAP-AD-33 | A dated, evidence-based quality audit scores the template and reduces the gaps to tracked problems. | 2026-07-02 audit: 94 attributes, 28 problems, issues #176 to #203 | DROP | Analyst decision. No new audit document is a deliverable of this release (scope principle 7). The PHP audit is consumed as input: scope section 6 and "Audit problems P1-P28" below. |

### 21. Architecture model (Structurizr)

| Id(s) | Capability | PHP mechanism | Disposition | Note |
| --- | --- | --- | --- | --- |
| CAP-40 | The architecture is kept as code: a C4 model in the repository that is browsable locally. | `workspace.dsl` + Structurizr Lite on port 8080 | FIX | Defects APP:D-28, APP:D-29, DM:D-36: the model disagrees with the code, has one view, and the viewer's lock file is committed. Scope 3.1: `workspace.dsl` kept, matches the code, has context, container and component views, is validated in CI (NEW-16); Structurizr Lite stays in the development stack. |

## Contracts to preserve

Status values: **preserved** (must be identical in Rust), **changed** (deliberately different; reason given), **not carried**, **architect** (not fixed by `scope-decisions.md`).

### HTTP surface

| Contract | Exact value | Status | Source |
| --- | --- | --- | --- |
| Route prefix | `/api` | preserved | CAP-06; scope 3.1 |
| Health success | `GET /api/health` -> `204`, empty body | preserved | CAP-07; `features/health_check.feature`; scope 3.1 |
| Health failure | `GET /api/health` -> `500`, `application/problem+json`, body contains the message of the failing dependency | preserved; addition: every failing dependency is reported | CAP-08; scope 3.1 |
| Failure string, database | `Database is not available` | preserved exactly | `features/health_check.feature`; scope 3.1 |
| Failure string, cache | `Cache is not working` | preserved exactly | same |
| Failure string, broker | `Message broker is not available` | preserved exactly | same |
| Liveness | `GET /ping` -> `200`, body `pong`, no dependency access, absent from the OpenAPI document | preserved | CAP-DM-20; scope 3.1 |
| Docs UI | `GET /api/docs` -> `200` | preserved | CAP-18; `features/demo.feature`; scope 3.1 |
| GraphQL endpoint | `/api/graphql` | preserved; schema content changed (intentional operations only) | CAP-19; scope 3.1 |
| Example resource path | `/api/example_api_resources` | preserved; operations changed (list with bounded page, create, read by id) | CAP-13; scope 3.1 |
| Example resource fields | `name`, `description` (both `String!` in the PHP SDL) | preserved as fields; further fields (identifier) by PRD and architect | `php-ci-github-contracts.md` I-13 |
| Media types | success `application/json`; errors `application/problem+json` | preserved | CAP-14; scope 3.1 |
| OpenAPI version | `3.1.0` | preserved | I-12; scope 3.1; `rust-http-api-stack.md` |
| Database probe | `SELECT 1` | preserved | CAP-09 |
| Cache probe key and value | key `health_check`, value `ok` | preserved (probe also writes on each call) | CAP-10 |
| Broker probe | `CreateQueue` for `health-check-queue` | changed: non-mutating call; queue name no longer a contract | CAP-11; scope 3.1 |
| Published service port | `8081`, plain HTTP | preserved | CAP-DM-18; scope 3.1 |
| Ports 80, 443/tcp, 443/udp | Caddy listeners | not carried | CAP-DM-19; scope 3.2 |

Gherkin step phrases of the reused feature (`features/health_check.feature`, 4 scenarios; source `php-tests-and-quality.md` I-7):

- `When GET request is sent to "/api/health"` (pattern: `<method> request is sent to <path>`)
- `Then the response status code should be 204` and `Then the response status code should be 500`
- `And the response body should contain "<text>"`
- `Given the cache is not working`
- `Given the database is not available`
- `Given the message broker is not available`

The docs feature steps (`When a demo scenario sends a request to "https://localhost/api/docs"`, `Then the response should be received`) are **changed** (CAP-TQ-13).

### Paths and names

| Contract | Exact value | Status | Source |
| --- | --- | --- | --- |
| OpenAPI snapshot | `.github/openapi-spec/spec.yaml` | preserved | scope 3.1; I-12 |
| GraphQL snapshot | `.github/graphql-spec/spec` (no extension) | preserved | scope 3.1; I-13 |
| Spectral ruleset | `.spectral.yaml`, 10 custom rules plus `spectral:oas`, run with `--fail-severity=hint` | preserved | scope 3.1; I-14 |
| Architecture model | `workspace.dsl` at the repository root | preserved | scope 3.1; CAP-40 |
| Gherkin feature | `features/health_check.feature` | preserved (content reused) | scope 3.1 |
| Load kit | `tests/Load/` with `config.json.dist`, optional `config.json`, `scripts/<scenario>.js`, `results/<prefix><scenario>.html` (prefixes `smoke-`, `average-`, `stress-`, `spike-`, `all-`) | preserved | scope 3.1; `php-docker-make-load.md` I-10 |
| Guidance files | `AGENTS.md`, `CLAUDE.md`, `.claude/skills/` | preserved | scope 3.1 |
| Changelog | `CHANGELOG.md` | preserved | scope 3.1; I-10 |
| Review config | `.coderabbit.yaml` | preserved | scope 3.1 |
| Dependency updates | `.github/dependabot.yml` | preserved (ecosystems changed) | scope 3.1; I-7 |
| Sync workflows | `template-sync-pat.yml`, `template-sync-app.yml` | changed: PHP file `tempate-sync-pat.yml` has a typo | scope 3.1; W-18, W-19 |
| Sync ignore file | `.templatesyncignore` (the sync action's default path) | new file at the default path | scope 3.1; I-11 |
| Module names | `Shared`, `Internal/HealthCheck`, `CompanySubdomain/SomeModule`; layers `Domain`, `Application`, `Infrastructure` | preserved; on-disk Rust spelling: architect | scope 3.1 |
| Package identity | `rust-service-template`, licence `CC0-1.0` | set by scope | scope 4 |
| Bats suite path | `tests/CLI/bats/` | architect | `php-tests-and-quality.md` I-8 |
| Compose service names | `php`, `database`, `localstack`, `structurizr` | architect (`php` cannot stay; the other three are language-neutral) | `php-docker-make-load.md` I-4 |

### Ports

| Port | Use | Variable | Status | Source |
| --- | --- | --- | --- | --- |
| 8081 | Service HTTP (published) | PHP `APP_HTTP_TEST_PORT`; Rust name: architect | preserved | scope 3.1 |
| 5432 | PostgreSQL | `DB_PORT` | preserved | scope 4 |
| 4566 | LocalStack | `LOCALSTACK_PORT` | preserved | scope 4 |
| 8080 | Structurizr Lite (`http://localhost:8080/workspace/diagrams`) | `STRUCTURIZR_PORT` | preserved | scope 4 |
| 80, 443/tcp, 443/udp | Caddy HTTP, HTTPS, HTTP/3 | `HTTP_PORT`, `HTTPS_PORT`, `HTTP3_PORT` | not carried | scope 3.2 |

### Environment variables and command parameters

| Group | Names | Status | Source |
| --- | --- | --- | --- |
| Compose-level, PHP names kept | `DB_VERSION` (default `16`), `DB_NAME`, `DB_USER`, `DB_PASSWORD`, `DB_PORT`, `LOCALSTACK_PORT`, `STRUCTURIZR_PORT`, `APP_ENV` | preserved | scope 4 |
| Ecosystem-standard names | `DATABASE_URL` (PHP `DB_URL`), `AWS_REGION` (PHP `AWS_SQS_REGION`), `AWS_ACCESS_KEY_ID` (PHP `AWS_SQS_KEY`), `AWS_SECRET_ACCESS_KEY` (PHP `AWS_SQS_SECRET`), `AWS_ENDPOINT_URL_SQS` (PHP `AWS_SQS_ENDPOINT_BASE` plus `LOCALSTACK_PORT`) | changed by scope; mapping documented (NEW-35) | scope 4 |
| Load kit | `API_HOST`, `API_PORT` | preserved | I-10 |
| Make parameters | `scenario=`, `runSmoke=`, `runAverage=`, `runStress=`, `runSpike=`, `base_ref=` (default `origin/main`), `SCHEMATHESIS_URL` (default `http://localhost:8081`), `LOCAL_MODE_ENV=true`, `OPENAPI_DIFF_IMAGE` | preserved with their targets | `php-docker-make-load.md` I-8 |
| Execution switch | PHP `CI=1` and `EXEC_ENV` | changed: explicit switch that honours `CI=true`; name by the architect | scope 5 |
| Not carried | `APP_SECRET`, `AWS_SQS_VERSION`, `EMAIL_QUEUE_NAME`, `MESSENGER_TRANSPORT_DSN`, `CADDY_MERCURE_JWT_SECRET`, `SERVER_NAME`, `APP_RUNTIME_HTTP_ENV`, `XDEBUG_MODE`, `KERNEL_CLASS`, `PANTHER_*`, `SYMFONY_DEPRECATIONS_HELPER`, `LOAD_TEST_CONFIG` | not carried | scope 3.2; APP:D-34, APP:D-35 |

### Make target names

PHP has 68 targets (`php-docker-make-load.md` I-8). Scope 3.1: capability-named targets keep their names, tool-named targets get capability names, the mapping is documented.

| Class | Count | Targets | Source |
| --- | --- | --- | --- |
| Survive verbatim: named language-neutral by the research | 37 | `help`, `start`, `up`, `build`, `stop`, `down`, `sh`, `logs`, `new-logs`, `purge`, `clean`, `install`, `update`, `setup-test-db`, `validate-configuration`, `unit-tests`, `integration-tests`, `all-tests`, `test`, `bats`, `tests-with-coverage`, `coverage-html`, `coverage-xml`, `generate-openapi-spec`, `generate-graphql-spec`, `validate-openapi-spec`, `openapi-diff`, `schemathesis-validate`, `smoke-load-tests`, `average-load-tests`, `stress-load-tests`, `spike-load-tests`, `load-tests`, `execute-load-tests-script`, `aws-load-tests`, `aws-load-tests-cleanup`, `ci` | `php-docker-make-load.md` "Notes for the Rust port"; scope principle 2 |
| Survive verbatim: capability-named by analyst classification | 5 | `setup-load-test-db`, `all`, `ci-sequential`, `check-requirements`, `check-security` | scope 3.1 naming rule; architect confirms |
| Tool-named: capability name to be fixed by the architect | 18 | `phpcsfixer`, `psalm`, `psalm-security`, `psalm-security-sarif`, `phpmd`, `phpinsights`, `deptrac`, `deptrac-debug`, `rector-apply`, `rector-ci`, `behat`, `infection`, `composer-validate`, `doctrine-migrations-migrate`, `doctrine-migrations-generate`, `ci-deptrac`, `build-spectral-docker`, `build-k6-docker` | scope 3.1 naming rule |
| Internal stages of `make ci`: composition by the architect | 4 | `ci-preflight`, `ci-static-analysis`, `ci-tests-and-openapi`, `ci-mutation` | I-8 |
| Not carried | 4 | `cache-clear`, `cache-warmup`, `commands`, `load-fixtures` | CAP-DM-31, CAP-24 |

New target or command names fixed by scope: `doctor`, `bootstrap` (Make); `migrate`, a rollback command, `check-config` (service CLI).

### Commit convention

PHP hook regex (`captainhook.json`, decoded), the form scope 3.1 keeps:

```
/^(feat|fix|refactor|build|chore|ci|docs|style|perf|test)\(\#\d+\): .*/i
```

- Ten types: `feat`, `fix`, `refactor`, `build`, `chore`, `ci`, `docs`, `style`, `perf`, `test`. The scope is mandatory and is `#` plus an issue number.
- PHP error text: `The commit message should be in the format: feat|fix|refactor|build|chore|ci|docs|style|perf|test(#1):`
- Version rules (`.github/AUTORELEASE.md`): `fix` -> patch, `feat` -> minor, footer `BREAKING CHANGE:` -> major.
- Bot subjects that do not match and must be exempt from the title check: `chore(release): vX.Y.Z [skip ci]`, `feat(deps): ...`, `chore(deps-dev): ...`, `chore(template): merge template changes :up:`.

### Secrets

| Secret | Used for | Status | Source |
| --- | --- | --- | --- |
| `VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY` | GitHub App token: release commit, tag and hosted release; App-flavour template sync | preserved | scope 3.1 ("same flow and secrets as PHP"); I-6 |
| `PERSONAL_ACCESS_TOKEN` | PAT-flavour template sync checkout | preserved | scope 3.1 (both sync workflows); I-6 |
| `CODECOV_TOKEN` | Coverage upload | preserved only if the non-gating upload is kept (analyst decision) | I-6 |
| `GITHUB_TOKEN` (automatic) | Read access on pull-request workflows | preserved with least privilege (NEW-27) | I-6 |
| `GPG_PRIVATE_KEY`, `GPG_PASSPHRASE` | Signing the auto-fix commit | not carried (CAP-TQ-27) | I-6 |

### Labels

| Label | Meaning | Status | Source |
| --- | --- | --- | --- |
| `approved-breaking-change` | Approves an intentional GraphQL breaking change | preserved, and extended to the OpenAPI breaking-change check | W-08; scope 3.1 |
| `template_sync` | Applied to template-sync pull requests | preserved | W-18, W-19 |
| `bug`, `enhancement` | Default labels of the two issue templates | preserved | I-9 |

### Release, sync and tool constants

| Contract | Exact value | Status | Source |
| --- | --- | --- | --- |
| Release tag | prefix `v` | preserved | I-10 |
| Release commit | `chore(release): vX.Y.Z [skip ci]` | preserved | I-10 |
| Changelog layout | Angular preset: `### Features`, `### Bug Fixes` | preserved | I-10 |
| Version file | PHP `composer.json` `"version"` | changed: `Cargo.toml`, mirrored in `Cargo.lock` | CAP-CI-25 |
| Sync source | PHP `VilnaCRM-Org/php-service-template` | changed: `VilnaCRM-Org/rust-service-template` | I-11 |
| Sync settings | `upstream_branch: main`, `pr_labels: template_sync`, `is_force_push_pr: true` | changed: `is_pr_cleanup: true` added (FR100(b)) | W-18, W-19 |
| Sync schedules | PAT `0 0 1 * *`; App `0 9 * * MON` | preserved; no-op in the template repository | W-18, W-19; scope 3.1 |
| Conformance flags | Schemathesis `--checks all --phases=examples,coverage` | preserved | W-16; research |
| Diff base | `origin/main` | preserved | I-9 scripts |
| K6 thresholds | `http_req_duration{test_type:<profile>}`: `p(99)<N`; `checks{scenario:<profile>}`: `rate>0.99`; check name `is status 204` | preserved (N changed) | I-10 |
| CI wait loop | `http://localhost:8081/ping` | preserved | W-16; scope 3.1 |
| OpenAPI metadata | contact `VilnaCRM Support`, `support@vilnacrm.com`; licence `CC0-1.0`, `https://creativecommons.org/publicdomain/zero/1.0/`; tags `ExampleApiResource` ("Template example resource endpoints.") and `HealthCheck` ("Runtime health-check endpoints."); response texts "Health check passed with no content to return.", "Health check failed.", "Problem response." | available for reuse; title, description, `info.version` and operation ids (`api_health_get`, `api_example_api_resources_get`): architect | I-12; scope 4 |
| Output markers asserted by Bats | `Usage:`, `make [target] [arg="val"...]`, `Targets:`; `CI checks successfully passed!`; `Configuration Validation`, `[OK] Configuration validation passed`, `[FAIL] Configuration validation failed`, `Modification of locked configuration file is not allowed: <file>`; `Launched instance`, `You can access the S3 bucket here` | recommended to keep so the Bats tests port with few edits; not mandated by scope | `php-docker-make-load.md` I-9, I-13 |
| Access-log markers | `"msg":"handled request"`, `"uri":"/ping"`, start-up line `FrankenPHP started` | not carried (web-server specific); the Rust service defines its own start-up marker and request-log shape, and the Bats tests change with them | I-13 |

## Thresholds

"Rust target" is what `scope-decisions.md` fixes. Where scope fixes the gate but not the number, the cell says "architect".

| # | Gate | PHP value | Enforced in PHP | Rust target |
| --- | --- | --- | --- | --- |
| 1 | Code-quality score | PHPInsights `min-quality` 100 | yes | No composite score. Static analysis with warnings denied: 0 findings (scope 3.1) |
| 2 | Complexity score | PHPInsights `min-complexity` 95 | yes | Complexity limits as a failing gate (scope 3.1); numeric limits: architect |
| 3 | Architecture score | PHPInsights `min-architecture` 100 | yes | Architecture gate: 0 violations, 0 unassigned code, cross-context rule (scope 3.1) |
| 4 | Style score | PHPInsights `min-style` 100 | yes | Formatting check: 0 differences (scope 3.1) |
| 5 | Line length | 80 columns, comments exempt | yes, through the style score | Not fixed by scope; formatter width: architect |
| 6 | Static analysis | Psalm `errorLevel="8"` (most lenient), 0 errors | yes | Strict profile, warnings denied, 0 findings (scope 3.1, P11) |
| 7 | Taint analysis | 0 findings | yes | No taint tool in scope; the security gates in rows 17 to 19 each at 0 findings |
| 8 | Mess detection | PHPMD stock limits: cyclomatic complexity 10, NPath 200, method length 100 | no (tool not installed) | Folded into row 2 |
| 9 | Refactoring drift | Rector dry run: 0 pending changes | yes | Folded into row 6 |
| 10 | Layer rules | Deptrac: 0 violations, 0 uncovered classes | yes (14 of 32 classes unassigned in practice) | As row 3 |
| 11 | Unit and integration tests | all pass | yes | all pass |
| 12 | Deprecations | tolerated up to 999999 | effectively disabled | none tolerated: warnings denied (P10, P11) |
| 13 | Acceptance tests | no failed step; undefined and pending steps pass | partly | every scenario passes; undefined or pending steps fail (P10) |
| 14 | Line coverage | none in the repository; the Codecov upload must succeed | no | 100% lines for testable code, enforced in the repository (scope 3.1, P9) |
| 15 | Mutation score | none; a Bats test matches the text `Mutation Code Coverage: 100%`; run with `-j8` | no | fails on any surviving mutant (scope 3.1, P8) |
| 16 | Manifest validation | valid; warnings allowed and expected by a test | yes | manifest and lockfile consistent, no tolerated warnings |
| 17 | Dependency vulnerabilities | 0 known advisories | yes | 0 advisories, 0 licence-policy violations, 0 source-policy violations, 0 unused dependencies (scope 3.1) |
| 18 | Secrets | no gate | no | 0 findings from secrets scanning (scope 3.1, P4) |
| 19 | Image vulnerabilities | no gate | no | image scan in scope (P6); failing severity: architect (the audit suggests HIGH and CRITICAL) |
| 20 | Protected configuration | 5 files unmodified, 14 directories present | local `make ci` only | guard runs in CI (scope 3.1, P27); file and directory lists: architect |
| 21 | Commit message | regex above, 10 types | local hook only | same form; local hook plus PR-title check in CI, bots exempt (scope 3.1) |
| 22 | OpenAPI lint | 0 findings at any severity (`--fail-severity=hint`) | yes | same ruleset and severity (scope 3.1) |
| 23 | OpenAPI compatibility | none (no fail flag) | no | any breaking change fails unless label `approved-breaking-change` is present (scope 3.1) |
| 24 | GraphQL compatibility | no breaking change unless label `approved-breaking-change` | yes | same (scope 3.1) |
| 25 | Snapshot freshness | none for OpenAPI; CI auto-commit for GraphQL | no | committed snapshot equals generated snapshot for both contracts (scope 3.1) |
| 26 | API conformance | Schemathesis `--checks all`, `--phases=examples,coverage`: 0 failed checks | yes | same, against the production image (scope 3.1, 5) |
| 27 | Smoke load | 10 rps for 10 s, 5 VUs; p99 < 9000 ms; checks > 99% | yes, on pull requests | profile kept, on pull requests; budget suited to a compiled service: architect (scope 3.1, P19) |
| 28 | Average load | peak 25 rps, 3 s rise / 10 s plateau / 3 s fall, 25 VUs; p99 < 200 ms; checks > 99% | no workflow runs it | profile kept; budget: architect |
| 29 | Stress load | peak 150 rps, 3 / 10 / 3 s, 150 VUs; p99 < 1000 ms; checks > 99% | no workflow runs it | profile kept; budget: architect |
| 30 | Spike load | peak 200 rps, 5 s rise / 5 s fall, 200 VUs; p99 < 5000 ms; checks > 99% | no workflow runs it | profile kept; budget: architect |
| 31 | Pause between profiles | `delayBetweenScenarios` 60 s; setup and teardown timeouts 30 min | n/a | kept with the kit; unused timeouts removed (DM:D-30) |
| 32 | Threshold sanity | each threshold must be a positive number, else `Invalid threshold value` | yes | kept with the kit |
| 33 | Health scenario check | status equals 204 | yes | same |
| 34 | Load-script lint | 0 ESLint errors (warnings pass); `max-len` 150 | only when `tests/Load/**` changes | 0 findings with a minimal configuration (analyst decision) |
| 35 | YAML lint | indentation 2 (error), line length 120 (warning) | no (nothing runs it) | YAML lint is a gate (scope 3.1); rule values: architect |
| 36 | Container health | `/ping` on 8081: interval 10 s, timeout 3 s, retries 3, start period 180 s | yes | `/ping` on 8081 (scope 3.1); timings: architect |
| 37 | Database readiness | `pg_isready`: interval 10 s, retries 10 | yes | kept; must use the configured user and database (DM:D-08) |
| 38 | Start-up database wait | 60 attempts, 1 s apart; abort on exit status 255 | yes | bounded wait kept (CAP-DM-08); values: architect |
| 39 | CI readiness loop | 60 attempts, 2 s apart, on `http://localhost:8081/ping` | yes | same endpoint; redundant after a health-gated start (CI:D-23) |
| 40 | Health probe timeout | none | no | each probe under a timeout (scope 3.1); value: architect (the research spike used 3 s) |
| 41 | Outbound call timeouts | driver defaults | no | explicit timeout on every outbound call (P16); values: architect |
| 42 | List page size | none (one fixed object) | n/a | bounded page (scope 3.1); bound: architect |
| 43 | Cloud bucket-policy retry | 53 attempts, 5 s apart | yes | kept and explained (DM:D-38) |
| 44 | Dependency-update cadence | composer weekly; npm monthly, at most 5 open pull requests | yes | cargo, GitHub Actions, Docker, K6 npm package (scope 3.1); cadence: architect |
| 45 | Template-sync schedule | PAT `0 0 1 * *`; App `0 9 * * MON` | yes | kept; no-op in the template repository (scope 3.1) |
| 46 | Changelog retention | last 5 releases (action default) | yes | not fixed by scope; to be chosen deliberately (AD:D-AD-27) |
| 47 | Agent verification baseline | 15 commands; 5 diverging lists | not checked | one list equal to `make ci`, checked for drift (scope 3.1) |
| 48 | Database engine version | `DB_VERSION=16` | yes | 16 (scope 3.1) |

Reference counts at the analysed PHP commit (not gates): 36 unit tests in 18 classes; 6 integration tests in 3 classes; 5 Gherkin scenarios in 2 features; 50 Bats tests in 8 files, of which 12 negative; 68 Make targets, of which 56 documented; 21 ordered steps in `make ci`; 20 workflows, of which 17 run on pull requests; 17 targets honour the direct-mode switch.

Version pins in PHP (scope principle 6: Rust versions come from the research spikes, not from this list): PostgreSQL 16; LocalStack 3.4.0; Structurizr Lite 2024.07.02; Spectral 6.15.1; openapi-diff 2.1.2; Schemathesis pinned by digest.

## Defects not to replicate

Deduplicated digest of the five defect lists: 188 source items, 150 lines. Each line cites every source item it covers. Counts are per area.

### Service runtime and HTTP surface (6)

1. Edge behaviour is neither specified nor tested: unknown routes, wrong methods, unsupported media types, CORS, request size, request correlation. (APP:D-23)
2. No application logging, metrics, tracing, authentication or security headers, despite the "enterprise-grade" positioning. (APP:D-37)
3. The committed env file is tracked although git-ignored and carries secret-shaped defaults; application and infrastructure variables are mixed; a queue name from another service remains. (APP:D-35, DM:D-09, CI:D-32)
4. The test env file is rotten: wrong kernel class, variables for tools that are not installed, a variable nothing reads. (APP:D-34, TQ:38)
5. The tooling listener serves a different application mode in each stack, and contract fuzzing prepares a database the server under test does not use. (DM:D-14, CI:D-29)
6. The GraphQL schema is an accidental projection: Relay node, connections and create, update and delete mutations for both resources with no code behind them, yet diffed as a contract. (APP:D-19, CI:D-27)

### Health and liveness (7)

1. Probes sit in the Application layer but call the database driver, the AWS SDK and the cache directly; the controller injects a concrete Infrastructure factory. (APP:D-11)
2. Probes run sequentially, fail fast, with no timeout and no aggregation; their order is an accident of file names. (APP:D-12)
3. Failure is a generic 500 whose text is whatever the driver throws; the three asserted messages come from test mocks and are visible only because the test environment runs with debug on. (APP:D-13, TQ:10)
4. The broker probe mutates infrastructure by creating a queue on every request, and an integration test leaves a second queue behind. (APP:D-14, TQ:13)
5. The cache probe does not test a cache service: a never-expiring read on a filesystem pool, replaced by an in-memory array in tests, while the model and the image claim Redis. (APP:D-15)
6. The health check uses the framework dispatcher with an event that is not a domain event, while the template's own event bus is unused. (APP:D-07)
7. Container health checks call a path answered by the web server, which stays green when the application is broken; a 180 s start period hides slow starts. (APP:D-18, DM:D-19) Scope keeps `/ping` for container health by decision; in Rust the service process itself answers it.

### Example bounded context (1)

1. The example endpoint is a framework accident: a single-item read at a collection-style path that returns hard-coded values. (APP:D-21)

### Shared kernel (10)

1. The command-bus adapter does not exist although configuration and an analyser exclusion reference it. (APP:D-01, TQ:35)
2. A duplicate event-bus port survives only for a test fixture. (APP:D-02)
3. The event bus silently keeps one subscriber per event and ignores the declared subscription list. (APP:D-03)
4. Publishing an event that has no subscriber is an error. (APP:D-04)
5. The subscriber contract is incomplete: the required invoke method is found by reflection and its absence fails at run time. (APP:D-05)
6. Reflection-based routing registers malformed subscribers under an empty key, and a unit test blesses it. (APP:D-06)
7. The UUID value object validates nothing and cannot generate; tests assert that non-UUID strings convert. (APP:D-08, TQ:23)
8. The domain event keeps its timestamp as an unvalidated string, and its test compares two "now" values and hard-codes UTC. (APP:D-09, TQ:18)
9. Dead scaffolding: unused classes, empty directories, a zero-byte route file, a template calling functions that are not installed, session and validator configuration nothing uses. (APP:D-10)
10. Naming and grammar slips in public identifiers and messages. (APP:D-38)

### Persistence, cache and messaging (5)

1. The message-transport configuration is malformed and inert, and its queue name belongs to another service; there is no working asynchronous messaging. (APP:D-16)
2. Broker client configuration: endpoint without scheme, API version `latest`, committed fake credentials, emulator-named port variable. (APP:D-17)
3. A foreign migration creates an unused user table with a password column, has an asymmetric down step and is applied at every container start. (APP:D-30)
4. ORM mapping points into the Domain layer at an empty directory, while guidance names a mapping directory that does not exist. (APP:D-31)
5. Connection settings are partly hard-coded, use two URL schemes and configure the server version twice. (APP:D-32)

### Architecture and its enforcement (4)

1. 14 of 32 classes fall outside every layer, including the whole health feature and the example module. (APP:D-24)
2. Layer patterns hard-code the placeholder names, so a new or renamed module needs an edit to a locked file or goes unguarded. (APP:D-25)
3. The rules are not hexagonal (Application and Infrastructure may depend on each other), carry vocabulary copied from another service and define a Logging layer without a logger. (APP:D-26)
4. Exclude patterns match nothing or too much. (APP:D-27)

### Formatting, static analysis and complexity (8)

1. The code-style gate never fails: always fix mode, fixes discarded on pull requests, not re-staged in the hook. (TQ:4, DM:D-02, CI:D-03)
2. The "changed files only" expression in the formatter command expands to nothing. (TQ:5)
3. Static analysis runs at the most lenient level, installed plugins are not enabled, and the tool's version is unconstrained. (TQ:6)
4. Dead lint configuration: a code-sniffer file nothing invokes and a security rule set installed but unreferenced. (TQ:33)
5. The refactoring check's mode variable never reaches the tool, the check depends on a warmed cache, and it is absent from `make ci`. (TQ:37, CI:D-08)
6. The second quality-score invocation differs in flags without explanation, and the first disables the security check. (TQ:41)
7. The protected-configuration guard is run by no workflow, is blind on shallow checkouts, is absolute (it blocks tightening and legitimate edits), omits several gate configurations and disagrees with the guidance. (TQ:42, DM:D-11, CI:D-05, AD:D-AD-02, AD:D-AD-03)
8. Lint configuration for YAML and Prettier exists but nothing runs it. (CI:D-18)

### Dependency and security checks (4)

1. Manifest hygiene: package name typo, licence `proprietary` against CC0, a development tool in production requirements, unused production and quality dependencies, inconsistent framework pins, three language versions. (APP:D-33, TQ:34)
2. The framework recipe lock file is stale. (APP:D-36)
3. The SARIF upload is skipped exactly when findings exist, and the workflow lacks the `security-events` permission. (CI:D-04)
4. The release tool writes a version field that makes manifest validation warn permanently, and a test requires the warning. (CI:D-15, TQ:27)

### Unit and integration tests (9)

1. Final and concrete classes are mocked through a load-time hack that strips `final`; subjects depend on concrete classes even where an interface exists. (TQ:24)
2. Random test data is unseeded, so failures are not reproducible. (TQ:19)
3. Integration assertions do not observe the subject. (TQ:15)
4. The same assertion is duplicated six times across the unit and integration levels. (TQ:16)
5. Misleading test names, a test that asserts nothing, and assertions inside a `catch` block that may never run. (TQ:17)
6. No test at any level touches a real cache backend; the "integration" cache test builds its own in-memory adapter. (TQ:12)
7. An orphan test sits outside every suite, next to a fixture that loads nothing and is not registered. (TQ:20)
8. Tests are filed under the wrong layer or path, breaking the mirror convention. (TQ:21)
9. Test base classes are used inconsistently and one set-up omits the parent call. (TQ:22)

### End-to-end tests (2)

1. Both acceptance suites run every feature file, and undefined steps do not fail because the run is not strict. (TQ:9)
2. The acceptance context inherits from a unit-test base class to reach mocking helpers, never verifies mock expectations and uses two different containers for fault injection. (TQ:11)

### Coverage and mutation testing (3)

1. Mutation testing has no minimum score, and the "should fail" test asserts only output text. (TQ:2, CI:D-02)
2. No coverage threshold exists in the repository; two workflows run the same suites and upload twice from different paths. (TQ:3, CI:D-07)
3. Two coverage targets use different output conventions, one writing to the container root. (DM:D-15)

### Makefile command surface and its tests (20)

1. `make ci` cannot pass at HEAD because it calls a tool that is not installed, and nothing runs it. (TQ:1, DM:D-01)
2. `make ci` differs from hosted CI in both directions. (CI:D-24)
3. `CI` has two meanings: the Makefile reacts only to `CI=1`, the entrypoint to any non-empty value, and only 17 targets honour direct mode. (TQ:39, DM:D-12)
4. The Makefile includes the test env file wholesale: mandatory for every target, mangles values, overrides the caller's environment. (DM:D-13)
5. Dead Makefile content: unused variables, a no-op directive, a prerequisite without a rule, a maintainer's name. (DM:D-27)
6. Help texts are wrong or stale, and the help column is too narrow. (DM:D-28)
7. Two mandatory commands are hidden from `make help`. (AD:D-AD-17)
8. `purge` deletes a directory that does not exist. (DM:D-29)
9. `make start` never rebuilds a stale image, `make build` always rebuilds from scratch, and there is no production-stack target. (DM:D-20)
10. Files written through the container are owned by root on the host. (DM:D-39)
11. Generated artefacts are not git-ignored: coverage, mutation logs, SARIF, bucket-name file, local load configuration, Bats helper directories. (TQ:40, DM:D-37)
12. Bats helper libraries cannot be obtained from the repository: submodule declarations with doubled paths and no gitlinks, while a rule demands Docker-backed commands the suite cannot follow. (TQ:7, DM:D-26, AD:D-AD-10)
13. `$!` is expanded by the wrong shell, so "interrupt" tests pass on a usage error. (TQ:25)
14. Assertions match Make's echoed recipe text rather than tool behaviour. (TQ:26)
15. A test for the OpenAPI export checks the GraphQL file. (TQ:28, CI:D-30)
16. Refactoring tests are mislabelled and contradict each other. (TQ:29)
17. Negative tests mutate the tree without teardown; positive tests rewrite the lock file, generate migrations and depend on file order. (TQ:30)
18. A negative acceptance test is duplicated under the name of a target that does not exist, and proves failure by deleting a class. (TQ:14)
19. A dead fixture and typos in test titles and file names. (TQ:31)
20. The CLI test runner is installed in the development image although the suite must run on the host and needs undeclared host tools. (TQ:32)

### Contract artifacts and contract checks (7)

1. The OpenAPI diff never fails on a breaking change because no fail flag is passed. (DM:D-10, CI:D-01)
2. The committed OpenAPI snapshot can drift: it is neither committed by CI nor compared with the generated one. (CI:D-10)
3. The OpenAPI snapshot under-describes the API: a schema without properties, an unused schema, no error schema, awkward summaries. (APP:D-20, CI:D-26)
4. API metadata drifts: a static `info.version` unrelated to the release, a legacy flag, grammar slips. (APP:D-22)
5. The GraphQL workflow commits to the pull-request branch with a release-relevant `feat` message, cannot handle fork pull requests, leaves the new head commit unchecked, races the merge ref, runs on a deprecated runtime and holds write permission. (CI:D-09)
6. The Spectral response rules accept a bare `default`, enumerate 200 status codes one by one and overlap built-in rules. (CI:D-28)
7. Schemathesis localhost detection never matches `127.0.0.1`. (DM:D-07)

### Load testing (15)

1. The load-test stack shares the compose project, volumes and ports with the development stack and drops the development database. (DM:D-03)
2. AWS: the IAM policy is created from an unrendered template. (DM:D-04)
3. AWS: the instance installs the standalone compose binary while the Make target uses the plugin syntax. (DM:D-05)
4. AWS: there is no failure path; the instance keeps running on error and the launcher never polls or reports. (DM:D-06)
5. The single-scenario target is half-wired: an empty scenario shifts the arguments, no stack is started, and its test never asserts success. (DM:D-18)
6. Runner scripts are inconsistent: debug tracing, `eval`, a working-directory assumption, and only one guards an empty scenario list. (DM:D-21)
7. AWS configuration script: an advertised option is missing, options cannot be passed through Make, env export is fragile, host tools are unchecked. (DM:D-22)
8. AWS cleanup ignores local mode, fails on a missing security group and uses fixed resource names and temporary paths. (DM:D-23)
9. The AWS run is narrower than documented: smoke only, the smallest instance class, a region-specific image id, unpinned downloads, a clone URL hard-coded to the template. (DM:D-24)
10. Load generator and system under test share one host, so thresholds are loose (smoke p99 below 9 s). (DM:D-25)
11. Dead or broken load-test JavaScript: an unused manager class calling an undefined method, a missing configuration key, 30-minute timeouts with no set-up function. (DM:D-30)
12. Load-generator image debt: unused extensions, unpinned core and base image, generic tag, whole repository as build context, rebuilt on every run. (DM:D-31)
13. The ESLint set-up is copied from a frontend project, has no Make target and runs only in CI. (DM:D-33)
14. An unexplained retry constant (53 attempts) and a misspelled test title. (DM:D-38)
15. Load results are not uploaded from CI. (DM:D-40)

### Containers and compose stacks (5)

1. The database healthcheck ignores the configured user and database. (DM:D-08)
2. The load-test "override" file is a full duplicate of the base file with a misleading comment and drift. (DM:D-17)
3. Port 80 is published but nothing serves it. (DM:D-32)
4. Leftovers: unused Mercure and Vulcain, JWT secrets in three compose files, a Redis extension without a Redis service, development files in the production image, unused build arguments. (DM:D-34)
5. An empty, misplaced emulator init hook is mounted in two stacks. (DM:D-35)

### CI workflows (9)

1. Container and image names are hard-coded to the template's name in workflows and scripts, which breaks downstream repositories. (DM:D-16, CI:D-06)
2. No caching, concurrency control, timeouts or artefacts; each push starts 15 uncached development-image builds. (CI:D-16)
3. Action pinning is inconsistent: mutable tags are used exactly where write tokens and secrets are present. (TQ:43, CI:D-19)
4. 12 of 20 workflows declare no `permissions`. (CI:D-20)
5. Naming: two workflows share one name, and file names mix styles and extensions. (CI:D-21)
6. Action runtimes are outdated. (CI:D-22)
7. Wasted work: the specification is generated twice, a readiness loop repeats a health-gated start, the architecture viewer starts in every job. (CI:D-23)
8. The auto-fix workflow is misnamed, pushes to `main` with a non-conforming message, interpolates a secret into a script and hands the signing key to an unpinned action. (CI:D-11)
9. The AI review configuration is a single setting that depends on an external App installation. (CI:D-33)

### Release, dependency updates and template sync (7)

1. Dependency bumps use the `feat` prefix and cut MINOR releases listed under "Features". (CI:D-12, AD:D-AD-29)
2. Template sync: two overlapping active workflows, a file-name typo, runs in the template against itself, a hard-coded source, no ignore list, a deprecated input, a mutable action tag with a write token, errors in the guides. (CI:D-13)
3. Release automation depends on an archived action and over-broad App permissions, bypasses review, produces no artefact and fails on every push when the secrets are absent. (CI:D-14, AD:D-AD-31)
4. The changelog keeps only the last five releases. (AD:D-AD-27)
5. Squash-merge bodies leak into the changelog (one issue linked 35 times). (AD:D-AD-28)
6. Changelog entries show the issue number where a component is expected. (AD:D-AD-30) Accepted: it follows from the `type(#issue): subject` form that scope 3.1 keeps.
7. Dependabot covers neither GitHub Actions nor Docker and names one individual as reviewer. (CI:D-25)

### Git hooks and commit convention (2)

1. The commit-message policy is enforced only by a local hook; history, the auto-fix bot, the dependency bot and the release commit violate it; the commit prompt builds messages the regex rejects. (TQ:8)
2. The pre-commit hook is heavy (integration and acceptance tests, a network audit, a running stack) yet omits the architecture check. (TQ:44)

### AI-assistant guidance (14)

1. The agent contract is unenforced: no effective hosted guard, no assistant settings, nothing verifies the reading order or the close-out. (AD:D-AD-01)
2. Five diverging verification command lists exist; none equals `make ci` or hosted CI. (AD:D-AD-04)
3. The skill list is maintained in four places with three different one-line descriptions per skill. (AD:D-AD-05)
4. Skill descriptions say what a skill is, not when to use it. (AD:D-AD-06)
5. The template name is baked into files meant to be synced downstream. (AD:D-AD-07)
6. The close-out wording of the verification skill is logically ambiguous. (AD:D-AD-08)
7. Verification steps exist that no command backs (architecture model, documentation sync). (AD:D-AD-09)
8. A skill points at a directory that does not exist. (AD:D-AD-11)
9. Skills describe patterns that have no exemplar in the repository. (AD:D-AD-12)
10. The gate reference skill is incomplete and duplicates numbers kept elsewhere. (AD:D-AD-13)
11. The contract skill covers export only, not lint, diff or conformance. (AD:D-AD-14)
12. The documentation-sync surface list is incomplete. (AD:D-AD-15)
13. No guidance covers commits, pull-request titles, branches, hooks, releases or migrations. (AD:D-AD-16)
14. No assistant settings file exists: no allowlist and no guard hook. (AD:D-AD-18)

### Human documentation (10)

1. README promises that are false or weak: a mess detector in the workflow, a working `make ci`, CodeQL and advanced security analysis, enforced backward compatibility, subdomain isolation. (CI:D-17, AD:D-AD-19)
2. Badges are mostly static images with hard-coded values that disagree with the gates. (TQ:36, AD:D-AD-20)
3. Language-version statements disagree across documents and manifest. (AD:D-AD-21)
4. Critical documentation lives on an external wiki that sync does not propagate; the repository has no architecture overview, decision records, troubleshooting guide or code of conduct. (AD:D-AD-22)
5. The first run is under-documented. (AD:D-AD-23)
6. The licence in `LICENSE` and README contradicts the manifest. (AD:D-AD-24)
7. The contributing guide is generic boilerplate with errors and omits every project rule. (AD:D-AD-25)
8. The security policy is a stub that says `TBD`. (AD:D-AD-26)
9. Issue and pull-request templates are frontend defaults with links hard-coded to the template. (CI:D-31, AD:D-AD-32)
10. The audit itself contains factual slips; use its problem list, not its evidence sentences. (AD:D-AD-33)

### Architecture model (Structurizr) (2)

1. The model disagrees with the code: wrong layers, a false relationship, Redis, dependencies drawn as internal components, missing components, a single view. (APP:D-28)
2. The viewer's lock file is committed, and the viewer mounts the whole repository read-write. (APP:D-29, DM:D-36)

## Audit problems P1-P28

Disposition text is taken from `scope-decisions.md` section 6. "Carried by" points at catalog rows.

| Problem | Disposition (scope section 6) | Carried by |
| --- | --- | --- |
| P1 logging and correlation IDs | In scope | NEW-01, NEW-02 |
| P2 observability | Partly: liveness/readiness split and dependency-naming failure response in scope; tracing and metrics deferred | CAP-DM-20, CAP-08; seam documented in NEW-42 |
| P3 authentication | Non-goal | scope 3.2 |
| P4 secrets | In scope: nothing secret-shaped committed, env reference, secrets scanning | CAP-03, CAP-DM-16, NEW-18, NEW-35 |
| P5 hardening | Security headers in scope; rate limiting non-goal | NEW-03 |
| P6 supply chain | Advisory/licence/source policy, image vulnerability scan, hardened runtime in scope; OSSF Scorecard follow-up | CAP-TQ-30, NEW-19, CAP-DM-02, CAP-DM-21 |
| P7 release artifacts | Non-goal | scope 3.2 |
| P8 mutation threshold | In scope | CAP-TQ-17 |
| P9 coverage floor, one coverage job | In scope | CAP-TQ-16 |
| P10 deterministic strict tests | In scope | CAP-TQ-04, CAP-TQ-09, CAP-TQ-13 |
| P11 strict static analysis | In scope | CAP-TQ-20 |
| P12 governance | PR-title convention check and no bot pushes in scope; CODEOWNERS and rulesets follow-up | CAP-TQ-28, CAP-TQ-27 |
| P13 SECURITY.md | In scope | CAP-AD-28 |
| P14 runbooks | Migration rollback command and operations notes in scope | NEW-13, NEW-37, CAP-DM-09 |
| P15 HA reference | Statelessness, graceful shutdown, limits and restart policy in scope; orchestrator manifests non-goal | NEW-04, CAP-DM-21, NEW-33 |
| P16 resilience | Timeouts on every outbound call in scope; the rest non-goal | NEW-06, CAP-12 |
| P17 CI cost | In scope | NEW-25, NEW-26, NEW-28, CAP-TQ-16 |
| P18 autonomous maintenance | Scheduled checks and wider Dependabot in scope; auto-merge and stale bot follow-up | NEW-29, CAP-CI-28 |
| P19 performance budgets | Realistic budgets and uploaded artifacts in scope; baselines follow-up | CAP-DM-41, CAP-DM-44, CAP-CI-17 |
| P20 i18n | Non-goal | CAP-35 |
| P21 dev environment | Dev container, `.editorconfig`, no submodules, `doctor` in scope; multi-arch follow-up | NEW-40, NEW-39, CAP-TQ-18, NEW-21 |
| P22 in-repo docs | In scope | NEW-33, NEW-34, NEW-35, NEW-36, NEW-22, CAP-CI-29 |
| P23 scaffolding | In scope | NEW-15, NEW-38, CAP-38 |
| P24 contract enforcement | In scope | CAP-DM-36, CAP-CI-24, CAP-16, NEW-24 |
| P25 debugging tooling | GraphQL explorer in dev and request collection in scope | CAP-19, NEW-41 |
| P26 hygiene | In scope | CAP-AD-30, CAP-CI-27, CAP-DM-05, NEW-27, CAP-CI-36 |
| P27 AI-native hardening | In scope | CAP-41, NEW-30, NEW-31 |
| P28 CQRS exemplars | In scope through the example slice | NEW-07 to NEW-12, CAP-28, CAP-34 |

## Capabilities added beyond PHP

Everything `scope-decisions.md` puts in scope that has no PHP capability id. Rule used: a scope item that is the corrected form of an existing capability stays in that capability's row (`FIX` or `EXTEND`); only items with no PHP row to attach to get a `NEW` id.

| Id | Capability | Area | Scope reference |
| --- | --- | --- | --- |
| NEW-01 | The service writes structured JSON logs. | 1 | 3.1 Service runtime; P1 |
| NEW-02 | A request/correlation id is accepted from or generated for every request, echoed in the response and attached to every log line. | 1 | 3.1 Service runtime; P1 |
| NEW-03 | Security response headers are set on every response. | 1 | 3.1 Service runtime; P5 |
| NEW-04 | The service shuts down gracefully on SIGTERM. | 1 | 3.1 Service runtime; P15 |
| NEW-05 | Configuration is typed and validated at start-up, and the service refuses to start on an invalid or missing value. | 1 | 3.1 Service runtime (with CAP-03, CAP-CI-15) |
| NEW-06 | Every outbound call (database, broker, any remote dependency) has an explicit timeout. | 1 | section 6, P16 |
| NEW-07 | An `ExampleApiResource` is created through the command path: validated input, command bus, aggregate, domain event. | 3 | 3.1 Example bounded context; P28 |
| NEW-08 | One `ExampleApiResource` is read by id through the query path. | 3 | 3.1 Example bounded context; P28 |
| NEW-09 | An in-process domain-event subscriber writes a structured audit log line. | 3 | 3.1 Example bounded context |
| NEW-10 | GraphQL exposes the queries that match the REST reads and the create mutation, and nothing else. | 3 | 3.1 Service runtime and Example bounded context |
| NEW-11 | A query-bus port has a working in-process adapter, and a missing query handler is an error. | 4 | 3.1 Shared kernel; P28 |
| NEW-12 | A repository port has a PostgreSQL adapter and a migration for the example resource. | 5 | 3.1 Example bounded context and Persistence; P28 |
| NEW-13 | A documented command rolls a migration back. | 5 | 3.1 Persistence; P14 |
| NEW-14 | Architecture rules forbid one bounded context from reaching into another except through the shared kernel. | 6 | 3.1 Architecture |
| NEW-15 | A scaffolding command creates a new bounded-context module that compiles and passes the architecture gate. | 6 | 3.1 Architecture; P23 |
| NEW-16 | The Structurizr model is validated in CI. | 21 | 3.1 Architecture |
| NEW-17 | An unused-dependency check is a gate. | 8 | 3.1 Quality gates |
| NEW-18 | Secrets scanning is a gate. | 8 | 3.1 Quality gates; P4 |
| NEW-19 | The production image is scanned for vulnerabilities. | 8 | section 6, P6 |
| NEW-20 | Workflow files, shell scripts and Dockerfiles are linted. | 7 | 3.1 Quality gates |
| NEW-21 | A `doctor` target checks host prerequisites. | 12 | 3.1 Delivery; P21 |
| NEW-22 | A one-shot `bootstrap` target prepares a fresh clone. | 12 | 3.1 Delivery; P22 |
| NEW-23 | Image, container and compose project names derive from one project-name variable. | 15 | section 4 |
| NEW-24 | The toolchain is pinned in `rust-toolchain.toml`. | 15 | section 5 |
| NEW-25 | CI caches dependencies and build output. | 16 | 3.1 Delivery; P17 |
| NEW-26 | CI cancels superseded runs and every job has a timeout. | 16 | 3.1 Delivery; P17 |
| NEW-27 | Workflows declare least-privilege permissions, and third-party actions are pinned by commit SHA. | 16 | 3.1 Delivery; P26 |
| NEW-28 | Heavy jobs are skipped for documentation-only changes. | 16 | 3.1 Delivery; P17 |
| NEW-29 | The security and CI checks run on a schedule on `main`. | 16 | 3.1 Delivery; P18 |
| NEW-30 | A documentation-drift check verifies the commands cited in the guidance. | 19 | 3.1 AI guidance; P27 |
| NEW-31 | A committed assistant settings file carries an allowlist and a guard on protected files. | 19 | 3.1 AI guidance; P27 |
| NEW-32 | BMAD and bmalph are integrated following the `core-service` convention: tracked `bmalph/config.json`, command wrappers, Make targets to install and initialise bmalph, the `bmad-autonomous-planning` skill, generated assets kept out of git, planning bundles tracked under `specs/`. | 19 | 3.1 AI guidance |
| NEW-33 | An architecture overview with decision records lives in the repository and states the statelessness assumptions. | 20 | 3.1 documentation; P22, P15 |
| NEW-34 | A troubleshooting guide lives in the repository. | 20 | 3.1 documentation; P22 |
| NEW-35 | An environment-variable reference lists every variable and documents the PHP-to-Rust name mapping. | 20 | 3.1 documentation; section 4; P4 |
| NEW-36 | A Make-target reference lists every target and documents the PHP-to-Rust name mapping. | 20 | 3.1 Delivery and documentation; P22 |
| NEW-37 | Operations notes cover migrations and rollback and give a health-probe runbook. | 20 | 3.1 documentation; P14 |
| NEW-38 | A guide explains how to create a service from the template. | 20 | 3.1 Architecture; P23 |
| NEW-39 | An `.editorconfig` file is committed. | 20 | 3.1 documentation; P21 |
| NEW-40 | A dev container definition is committed. | 20 | 3.1 documentation; P21 |
| NEW-41 | A request collection covers the endpoints. | 20 | 3.1 documentation; P25 |
| NEW-42 | Extension seams are documented: a Redis adapter through the cache port, and the tracing and metrics seam. | 20 | 3.1 Persistence; 3.2 |

## Decisions taken by the analyst

Each item is a choice `scope-decisions.md` does not make explicitly. The disposition rule from the task was applied: `KEEP` when the capability is language-neutral and useful, `DROP` when it is a PHP or Symfony artefact with no Rust meaning.

**Dispositions where scope is silent**

1. CAP-04, CAP-29: `KEEP`. Wiring and handler registration move to one explicit composition root. Discovery by reflection is not reproduced.
2. CAP-05, CAP-DM-10, CAP-DM-12: `KEEP`, met by construction (release build; lock file resolved on every build; binary as image entry point). They create no work items of their own.
3. CAP-10: `FIX`. The cache probe writes and reads on each call; scope only says the cache must "respond".
4. CAP-17: `KEEP`. Tag descriptions are declared in the code-first document; the decorator hook is not reproduced.
5. CAP-20: `DROP`. The error-page preview is treated as part of HTML templating (scope 3.2).
6. CAP-23: `FIX`. "A place for persistence mapping" becomes the Infrastructure adapter behind the repository port.
7. CAP-24, CAP-TQ-15: `DROP`. The empty fixture scaffold and its `load-fixtures` target are dead scaffolding (scope principles 5 and 7).
8. CAP-39: `KEEP` as output of the architecture gate, not necessarily a separate target.
9. CAP-DM-09: `FIX`. The stack start runs the explicit `migrate` command as its own step; the service process does not migrate implicitly.
10. CAP-CI-14: `KEEP`. Manifest and lockfile consistency through locked builds.
11. CAP-TQ-05: `KEEP`. "Tests mirror the source tree" becomes the Rust idiom (unit tests beside the code, other levels in a tree that follows the bounded contexts).
12. CAP-TQ-08: `KEEP`. The test profile becomes test-built configuration and port substitution.
13. CAP-TQ-21, CAP-CI-10: `FIX`. No taint tool is added. The scoped security gates carry the intent; SARIF publication is kept where one of them can emit it.
14. CAP-TQ-23: `KEEP` for the sub-rules that have Rust meaning; the line-width value is left to the architect.
15. CAP-TQ-26, CAP-CI-12: `KEEP`, folded into the static-analysis gate and its auto-fix target.
16. CAP-TQ-29: `FIX`. A pre-commit pipeline is kept as a fast subset; scope names only the commit-message hook.
17. CAP-TQ-16, CAP-CI-04: publication to the external coverage service is kept as a non-gating step; the gate is the in-repository floor.
18. CAP-DM-08: `KEEP`. Bounded database wait with a clear error.
19. CAP-DM-11: `KEEP`. Watch-and-rebuild in the development target, with the condition that the architect records a non-goal if it is not delivered.
20. CAP-DM-22: `DROP`. Debugger call-back wiring is Xdebug-specific.
21. CAP-DM-31: `KEEP` for migrations and `purge`; `cache-clear`, `cache-warmup`, `commands` and `load-fixtures` are not carried.
22. CAP-DM-47, CAP-CI-18: `FIX`. JavaScript lint for the load scripts stays as a gate with a minimal configuration and a Make target.
23. CAP-AD-23: `KEEP`. Prompt pattern in the README.
24. CAP-AD-25: `FIX`. Dynamic badges only; the banner stays.
25. CAP-AD-33: `DROP`. No new audit document; the PHP audit is input only.
26. Rows kept as implementation details of an in-scope deliverable without a sentence of their own in scope: CAP-DM-04, CAP-DM-06, CAP-DM-15, CAP-DM-17, CAP-DM-40, CAP-DM-42, CAP-TQ-12, CAP-AD-21.

**Method decisions**

27. Merge rule: two ids are merged when they state the same capability, including the case where one inventory describes the gate and another describes the same gate "in CI". Composite ids were placed with their closest match and cross-referenced in the Note (CAP-TQ-30, CAP-CI-01, CAP-CI-06).
28. Area placement where the task's area list has no exact home: the protected-configuration guard is in area 7; ignore rules (CAP-CI-36) are in area 12; supply-chain pinning (CAP-DM-05) is in area 8; the platform check (CAP-CI-15) is in area 1; the review-bot configuration (CAP-CI-30) is in area 16.
29. `NEW` boundary: corrected forms of existing capabilities are not given `NEW` ids. Examples: the pull-request title check (row CAP-TQ-28), the sync ignore list and self-sync guard (CAP-CI-27), the development-only GraphQL explorer (CAP-19), the bounded list page (CAP-13), licence and source policy (CAP-TQ-30).
30. Make target classification: five targets outside the research's language-neutral list are classified as capability-named and kept (`setup-load-test-db`, `all`, `ci-sequential`, `check-requirements`, `check-security`); four have no Rust meaning and are not carried although their names are capability-style.
31. Defect AD:D-AD-30 (issue number in the changelog scope) is recorded as accepted, because scope 3.1 keeps `type(#issue): subject`.
32. Output markers and OpenAPI metadata strings from PHP are listed as reusable, not as binding contracts.

**Research findings that conflict with `scope-decisions.md` (scope prevails)**

33. `rust-http-api-stack.md` listens on `:8080` and proposes `GET /api/health/live` for liveness. Scope 3.1 fixes port `8081` and `GET /ping`. Port 8080 on the host belongs to Structurizr Lite.
34. `rust-data-cache-messaging.md` D6 keeps `CreateQueue health-check-queue` as the broker probe "for parity" and reports the first failure only. Scope 3.1 requires a non-mutating probe and a report of all failing dependencies.
35. `rust-http-api-stack.md` verifies OpenTelemetry and a Prometheus exporter. Scope 3.2 defers both; only the seam is documented (NEW-42).
