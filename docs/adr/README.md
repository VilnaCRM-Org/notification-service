# Architecture decision records

These records explain why the structure of the template is what it is. Each one has the same parts: the context that forced a decision, the decision, and its consequences. Each names the planning decisions it summarises, so a reader can trace a record back to the planning bundle (`AD-nn` are the core architecture decisions, `DD-nn` the delivery decisions). The [architecture overview](../architecture.md) shows how the decisions fit together.

| Record | Title | Planning decisions |
| --- | --- | --- |
| [0001](0001-workspace-and-layers.md) | Workspace and layers | AD-01, AD-02 |
| [0002](0002-architecture-gate.md) | Architecture gate | AD-03, AD-25 |
| [0003](0003-error-model.md) | Error model | AD-06 |
| [0004](0004-configuration.md) | Configuration | AD-07 |
| [0005](0005-persistence-and-migrations.md) | Persistence and migrations | AD-08, AD-20 |
| [0006](0006-health-and-liveness.md) | Health and liveness | AD-11, AD-12 |
| [0007](0007-api-contracts.md) | API contracts | AD-16, AD-17, AD-18, AD-19, DD-10 |
| [0008](0008-tests-coverage-mutation.md) | Tests, coverage and mutation | AD-23, AD-24, DD-09 |
| [0009](0009-make-execution-model.md) | Make execution model | DD-01, DD-02 |
| [0010](0010-workflow-layout.md) | Workflow layout | DD-08, DD-13 |
| [0011](0011-release-flow.md) | Release flow | DD-11, DD-12 |
| [0012](0012-container-images.md) | Container images | DD-19, DD-20 |

A downstream service keeps its own records in this directory next to the twelve of the template. To add one, copy the form of an existing record, number it with the next free four-digit prefix and add a row to the table above.
