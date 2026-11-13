# AI Agent Guide

How to use the skills of this directory. The index is [README.md](README.md); the intent-to-skill map is [SKILL-DECISION-GUIDE.md](SKILL-DECISION-GUIDE.md).

## The Cycle

1. Classify the task: code change, API change or architecture change, and note the files it will touch.
2. Pick the primary skill with the decision guide; add companion skills when the scenarios there say so.
3. Follow the skill: read its `Use This Skill When`, run its `Commands`, obey its `Rules`.
4. Verify with the `Verification` block of each skill used, then run the full list of the [verification section](../../AGENTS.md#verification) before declaring completion.
5. Summarise: state which targets were run with their result, which were not run and why, and every deviation from the skill.

## General Rules

- Run every tool through `make`; do not call cargo, docker or linters directly unless a skill says so.
- Fix root causes. Never edit a protected file, add a suppression, skip a test or loosen a threshold to obtain a pass.
- Keep changes small and reviewable; one concern per change.
- Prefer the existing example slice and its patterns over new structure.
- When a skill and the code disagree, trust the code, then repair the skill through `documentation-sync`.

## Flows

### Code Change

1. Choose the skill for the layer touched: `api-resource-development` for slice code, `complexity-management` for refactors.
2. Write the tests with the code (`testing-workflow`).
3. Check that boundaries still hold (`architecture-boundary-fixer` on failure).
4. Update the documentation surfaces the change touches (`documentation-sync`).

### API Change

1. Change the handler, DTO or GraphQL field (`api-resource-development`).
2. Regenerate both snapshots and run the contract checks (`api-contract-development`).
3. Add or adjust the Gherkin scenario and the request collection.
4. Extend a load scenario when the endpoint is new and latency matters (`load-testing`).

### Architecture Change

1. Add or move crates with `make new-context` or the manifests; keep layers and contexts apart (`architecture-boundary-fixer`).
2. Update `workspace.dsl` in the same change (`architecture-model-sync`).
3. Update the architecture documents and decision records (`documentation-sync`).
4. A protected file needs the approval label; ask for it instead of editing around the guard (`quality-standards`).
