---
name: complexity-management
description: Use when the complexity gate fails or a function approaches a limit.
---

# Complexity Management

## Use This Skill When

- `make complexity` reports a function over a limit.
- A function grows towards a limit and should be split before it fails.

## Commands

- `make complexity` runs the complexity lints alone and prints only complexity findings.
- `make static-analysis` runs the full strict lint, which includes the same four lints.

## Rules

- The limits live in `clippy.toml` and are quoted in `quality-standards` only; never copy a number into another file and never raise a limit to pass.
- Never add `#[allow]` or `#[expect]` to silence a complexity lint; refactor.
- Too long: extract a named function for each step of the body, keeping the call order readable.
- Too complex: replace nested `if` chains with early returns, guard clauses or a `match` on one value; move each branch into a small function.
- Too deeply nested: return early, use `?` instead of `match` on `Result`, flatten with iterator adaptors.
- Too many arguments: group related arguments into a struct or a value object of the domain, and pass the struct.
- Handlers follow the pattern of the architecture document: the trait method only delegates to an inherent `async fn`, and each step of the logic is its own function.
- Keep a refactor behaviour-preserving: run the unit tests before and after.
- Test code is relaxed for two of the lints, production code is not.

## Verification

```bash
make complexity
make unit-tests
```

Continue with `testing-workflow` when a test breaks after the refactor and with `architecture-boundary-fixer` when code must move between layers.
