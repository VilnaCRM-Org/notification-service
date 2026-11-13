# 0001. Workspace and layers

Planning decisions: AD-01 (workspace and crate granularity), AD-02 (meaning of the three layers and allowed dependency directions).

## Context

The PHP template arranges code as `src/<Context>/<Module>/<Layer>` with the layers Domain, Application and Infrastructure. A Rust template needs the same shape, and it needs the boundaries to be more than a convention. Cargo forbids dependency cycles, so Application and Infrastructure cannot use each other if they are separate crates. A single crate with modules would leave the boundaries syntactic only: the best module-level checker evaluated missed two of seven planted violations.

## Decision

One virtual Cargo workspace with thirteen crates: nine layer crates (three bounded contexts times three layers), the composition root `service`, `test-support`, `e2e-tests` and `xtask`. Layer crates are named `<context>-<layer>` and live at `crates/<group>/<context>/<layer>`; the shared kernel lives at `crates/shared/<layer>`. Workspace members are listed explicitly, because cargo fails on a glob when a matching directory has no manifest. No crate declares cargo features.

The layers keep their PHP names and these directions. Domain depends on Domain crates only. Application and Infrastructure each depend on Domain and never on each other. Only `service` depends on all three. Two refinements are recorded:

- Every port that Infrastructure implements lives in a Domain crate, including the technical ports for the cache, the clock, the UUID generator, the buses and the health probe. Infrastructure cannot see Application, so a port placed there could not be implemented.
- Application crates keep use cases and API adapters in separate top-level modules. The use-case modules do not import `axum`, `utoipa` or `async-graphql`; the `http` and `graphql` modules may.

## Consequences

The compiler enforces direction and rejects cycles. A handler cannot run SQL and an adapter cannot call a use case. Application crates carry the web and GraphQL dependencies, so their allow-lists are long but explicit. The module split inside Application is a review rule, because no gate checks module names; a context that needs another module edits no protected file. The scaffolding command must add three paths to the workspace members. Crate names must be unique across contexts. `service` and `xtask` keep neutral names so that a downstream service never renames a crate that a configuration file or a workflow refers to.

Alternatives rejected: a single crate with modules, glob members, a fourth presentation layer, classic hexagonal direction with Infrastructure depending on Application, and ports placed in Application.
