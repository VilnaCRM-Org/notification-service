# Rust Service Template Bootstrap — Planning Specs

Planning bundle for bootstrapping `rust-service-template` as the
Rust-ecosystem counterpart of
[`php-service-template`](https://github.com/VilnaCRM-Org/php-service-template).
It was produced with BMAD-METHOD through bmalph, run autonomously: one focused
subagent per stage, independent validators after each planning artifact, and
the orchestrating agent acting as the user's surrogate for decisions.

## Artifacts

| Phase | BMAD command | Artifact | Status |
| --- | --- | --- | --- |
| 1. Analysis | `analyst`, `technical-research` | [Research digest](research.md) | Complete |
| 1. Analysis | `analyst` | [Capability catalog](capability-catalog.md) | Complete |
| 1. Analysis | `create-brief` | [Product brief](product-brief.md) | Complete |
| 2. Planning | `create-prd`, `validate-prd`, `edit-prd` | [PRD](prd.md) | Complete |
| 3. Solutioning | `create-architecture`, `validate-architecture` | [Architecture: core](architecture.md) | Complete |
| 3. Solutioning | `create-architecture`, `validate-architecture` | [Architecture: delivery](architecture-delivery.md) | Complete |
| 3. Solutioning | `create-epics-stories`, `validate-epics-stories` | [Epics and stories](epics.md) | Complete |
| 3. Solutioning | `implementation-readiness` | [Implementation readiness](implementation-readiness.md) | Complete: READY WITH CONDITIONS |
| 3. Solutioning | `bmalph implement` preparation | [Parity matrix](parity-matrix.md) | Complete |
| — | orchestrator | [Scope decisions](scope-decisions.md) | Living record |
| — | orchestrator | [Run summary](run-summary.md) | Complete |

## How to read the bundle

1. [Scope decisions](scope-decisions.md) — what is in scope, what is not, and
   every decision taken on the maintainers' behalf. Start here if you want to
   overrule something.
2. [Research digest](research.md) — what the PHP template really contains,
   what its enterprise audit found, which Rust tools replace which PHP tools
   (with the versions that were built and run), and the pitfalls found by
   running them.
3. [Capability catalog](capability-catalog.md) — every PHP-template capability
   with its disposition (keep, fix, extend, drop) and the contracts to
   preserve. The PRD traces to these ids.
4. [Product brief](product-brief.md) and [PRD](prd.md) — users, journeys,
   functional and non-functional requirements, quality gates.
5. [Architecture: core](architecture.md) — workspace and crates, the 27
   architecture decisions, signatures, API contract, data, configuration,
   quality-gate commands and test architecture.
6. [Architecture: delivery](architecture-delivery.md) — Make command surface,
   containers and compose, CI workflows, release, hooks, load testing, Bats,
   AI guidance and the repository file manifest.
7. [Epics and stories](epics.md) — 10 epics and 57 stories with dependencies,
   owned paths, verification commands and acceptance criteria, in the form
   `bmalph implement` parses.
8. [Implementation readiness](implementation-readiness.md) — the independent
   cross-artifact assessment, its conditions and how each was handled.
9. [Parity matrix](parity-matrix.md) — one row for each of the 196 PHP
   capability ids and the 42 added ids with disposition, requirements and
   implementing stories, plus the disposition of the audit problems P1 to P28.
10. [Run summary](run-summary.md) — how the run was orchestrated, the subagent
   execution log, validation rounds, open questions and the next step.

## Files

The bundle holds the eleven files of the planning bundle layout, one line each,
and the scope decisions that they all depend on:

- `README.md` — this index.
- `research.md` — research digest: PHP findings, tool choices and pitfalls.
- `capability-catalog.md` — every PHP capability with its disposition.
- `product-brief.md` — users, problem and goals.
- `prd.md` — functional and non-functional requirements, gates, traceability.
- `architecture.md` — core architecture: workspace, decisions, quality gates.
- `architecture-delivery.md` — delivery architecture: Make, containers, CI.
- `epics.md` — epics and stories with owned paths and verification commands.
- `implementation-readiness.md` — cross-artifact readiness assessment.
- `parity-matrix.md` — capability ids traced to requirements and stories.
- `run-summary.md` — how the planning run was orchestrated.
- `scope-decisions.md` — binding scope decisions (an addition to the eleven).

Later planning bundles go to `specs/<bundle-id>/` with the same file names.

## Notes

- The nine raw research reports (five PHP inventories, four Rust technical
  reports) and the spike sources are working material and stay local under
  `_bmad-output/`; they are not tracked;
  the research digest stands in for them. References such as
  `research/php-*.md` and `research/rust-*.md` point to those local files.
- In `epics.md`, `${PHP_TEMPLATE_DIR}` and `${CORE_SERVICE_DIR}` stand for
  local checkouts of `php-service-template` and `core-service`, used by the
  stories that reuse or adapt files from them.
- The generated BMAD and Ralph assets (`_bmad/`, `.ralph/`, `_bmad-output/`)
  stay out of git; see `.gitignore`.
