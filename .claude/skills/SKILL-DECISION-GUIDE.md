# Skill Decision Guide

Pick one primary skill per intent. The index is [README.md](README.md); how to use a skill is in [AI-AGENT-GUIDE.md](AI-AGENT-GUIDE.md).

## Decision Tree

- Add or change an endpoint, field, command, query, subscriber or migration: `api-resource-development`.
- Run the checks or find out which skill owns a red gate: `verification-workflow`.
- A function is too long, too complex, too nested or has too many arguments: `complexity-management`.
- The architecture check fails or code moves between layers or contexts: `architecture-boundary-fixer`.
- A command, variable, endpoint, workflow or guidance file changed: `documentation-sync`.
- A K6 scenario or latency budget changes, or load must be measured: `load-testing`.
- The REST or GraphQL contract changes or a contract gate fails: `api-contract-development`.
- You need a threshold, a gate target or a protected file: `quality-standards`.
- A crate, context, external dependency or relationship appears or disappears: `architecture-model-sync`.
- Tests are written, run or debugged: `testing-workflow`.
- A planning bundle must be produced from a short description: `bmad-autonomous-planning`.

## Combined Scenarios

- New endpoint: `api-resource-development`, then `api-contract-development`, then `testing-workflow`, then `documentation-sync`.
- Boundary failure after a refactor: `architecture-boundary-fixer`, then `architecture-model-sync` when crates moved, then `verification-workflow`.
- Complex handler: `complexity-management`, then `testing-workflow` to prove the behaviour is unchanged, then `api-resource-development` for the handler pattern.
- New bounded context: `api-resource-development`, then `architecture-model-sync`, then `architecture-boundary-fixer` if the check complains, then `documentation-sync`.

## Non-Negotiable Rules

- Fix root causes; never weaken a gate, a threshold, a test or a protected file.
- Run every tool through `make`.
- Keep framework concerns out of Domain crates.
- Regenerate contract snapshots and update `workspace.dsl` in the same change as the code.
- Never edit a migration that has reached `main`.
- Never add `#[allow]`.
- Close out with what was run, what was not run and why.
