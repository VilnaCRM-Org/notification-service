# 0007. API contracts

Planning decisions: AD-16 (OpenAPI document, API version and service identity), AD-17 (contract export determinism), AD-18 (GraphQL schema and transport), AD-19 (documentation UI), DD-10 (snapshot drift policy).

## Context

Clients depend on the paths, statuses and operation ids of the PHP template, so the contract must be preserved and must not drift unnoticed. A release bumps the package version on every merge, so a contract document that carried it would change on every release. The crates that generate the documents must not need a database or a network.

## Decision

The OpenAPI document is code-first: `service::openapi::api_router()` builds one router from the per-context routers and produces the document from the same definitions that serve traffic. The title and the contact URL come from the package description and repository of the root manifest. `info.version` is the constant `API_VERSION`, the version of the contract and not of the release; it changes only in a pull request that changes the contract.

`service openapi-export` and `service graphql-export` print the documents before any configuration is read, with sorted maps and sorted SDL options, so twenty generations are byte-identical and need no environment. Both snapshots, `.github/openapi-spec/spec.yaml` and `.github/graphql-spec/spec`, are regenerated only by a developer and committed with the code change. CI fails on any byte of difference; no workflow writes to a branch.

GraphQL merges the context query and mutation types into root types `Query` and `Mutation`, with no subscription. `POST /api/graphql` is an ordinary axum handler that uses the problem-aware JSON extractor, so transport errors are problem documents, while operation errors are returned with status 200 in `errors[]` with a stable `extensions.code`. The explorer page is served only in development and loads four assets from one pinned origin with integrity attributes.

`GET /api/docs` returns the Swagger UI page with status 200 directly, because the crate integration answers the bare path with a redirect. The page loads nothing from a third-party origin.

## Consequences

Snapshot diffs are meaningful because the contract version moves only with the contract. The rule for `API_VERSION` is a review convention; the breaking-change gates still detect the change itself. A deliberate breaking change needs the label `approved-breaking-change`. Developers must run the two generation targets before committing a contract change. The explorer needs internet access in the browser; the docs page does not. Upgrading a pinned explorer asset changes the URL and the digest together.

Alternatives rejected: the package version as contract version, exporting from a running service, automatic commits of snapshots (they need a write token and cannot work for forks), the GraphQL crate's own explorer source (unpinned versions and a second origin) and accepting the redirect on the docs path.
