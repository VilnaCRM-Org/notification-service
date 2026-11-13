---
name: architecture-model-sync
description: Use when a crate, bounded context, external dependency or relationship is added or removed.
---

# Architecture Model Sync

## Use This Skill When

- A workspace crate or bounded context is added, renamed or removed.
- An external system (database, queue, cache) or a relationship between components appears or disappears.

## Commands

- `make validate-architecture-model` checks `workspace.dsl`: it must be valid, define the three views, name a component for every workspace crate and hold a relationship for every dependency between them.
- `make start` starts the development environment, which includes the Structurizr viewer for looking at the model.

## Rules

- `workspace.dsl` is edited in the same change as the crate, context, dependency or relationship it describes.
- Every workspace crate has exactly one component; a removed crate loses its component and every relationship that names it.
- Keep the three views: system context, containers and components; a new element appears in the view where it belongs.
- Name components after the crates so the validator can match them.
- A relationship states a real dependency: a driven port implemented by an infrastructure crate, an external system reached by an adapter.
- Look at the result in the viewer after `make start` before committing; the page lists the views by name.
- A new bounded context follows the steps of `api-resource-development`, and its components are added before the model gate is run.
- Do not edit `architecture.toml`; a context needs no line there.

## Verification

```bash
make validate-architecture-model
```

Continue with `architecture-boundary-fixer` when the boundary check fails and with `documentation-sync` when the architecture documents describe the change.
