---
name: api-contract-development
description: Use when the REST or GraphQL contract changes or a contract gate fails.
---

# API Contract Development

## Use This Skill When

- A REST or GraphQL operation, type or error response changes.
- A contract snapshot, lint, breaking-change or conformance check fails.

## Commands

- `make generate-openapi-spec` writes `.github/openapi-spec/spec.yaml`.
- `make generate-graphql-spec` writes `.github/graphql-spec/spec`.
- `make validate-contract-snapshots` fails when a committed snapshot differs from the generated one.
- `make validate-openapi-spec` lints the OpenAPI snapshot.
- `make openapi-diff` and `make graphql-diff` fail on a breaking change against the base ref.
- `make schemathesis-validate` tests the production image against the OpenAPI snapshot.

## Rules

- The contract is generated from the code; never edit a snapshot by hand.
- Regenerate both snapshots in the same change as the handler, DTO or GraphQL field and commit them.
- A lint finding is fixed in the `#[utoipa::path]` annotation or the doc comment, not in the lint configuration.
- A breaking change needs the approval label named in the failure message; locally the matching environment variable stands in for the label.
- Prefer an additive change: new optional fields, new operations, new enum values only where clients tolerate them.
- A conformance failure is a defect in the code or the contract; fix the code, then regenerate.
- REST and GraphQL call the same command or query, so a behaviour change touches both contracts.

## Verification

```bash
make validate-contract-snapshots
make validate-openapi-spec
```

Continue with `api-resource-development` for the code side and `verification-workflow` before the close-out.
