---
name: bmad-autonomous-planning
description: Use when a BMAD planning bundle must be produced from a short task description without interactive menus.
---

# BMAD Autonomous Planning

## Use This Skill When

- the caller gives a short task description and wants research, brief, PRD, architecture, epics and a readiness check without walking through menus;
- a planning bundle for a new feature or service has to land under `specs/<bundle-id>/`.

## Commands

- `make bmalph-setup` installs the pinned bmalph and creates the local `_bmad/` and `.ralph/` assets that this repository keeps out of git; run it first in a fresh clone.
- `make bmalph-setup BMALPH_DRY_RUN=true` previews the initialisation and changes nothing.
- Start from the `bmalph` command and `_bmad/COMMANDS.md`; name the bmalph command that each stage follows.

## Rules

- Run the flow in the current session and write specs only. Never write production code, tests or configuration while planning.
- Use one focused subagent per stage (`analyst`, `create-brief`, `create-prd`, `create-architecture`, `create-epics-stories`, `implementation-readiness`) with the strongest available model. Give each only the inputs of its stage and an output contract: draft, assumptions, open questions, findings.
- The main agent acts for the user. When a menu asks for a choice, decide from the task and the repository, continue, and record the open question in `run-summary.md`.
- Write the bundle to `specs/<bundle-id>/` and never to the ignored `_bmad-output/`. It holds `research.md`, `product-brief.md`, `prd.md`, `architecture.md`, `epics.md`, `implementation-readiness.md` and `run-summary.md`.
- Use one to three validation rounds per artifact and stop when only minor issues remain.
- Keep the artifact format that `bmalph implement` parses:
  - `prd.md` has the second-level headings `## Executive Summary`, `## Functional Requirements`, `## Non-Functional Requirements` and `## Product Scope`.
  - `architecture.md` has `## Tech Stack`, which names only the Rust toolchain and crates; the words that bmalph reads as another stack (other languages' runtimes and package managers) stay out of it.
  - `epics.md` uses `## Epic N: Title` and `### Story N.M: Title`, a short description, then `**Acceptance Criteria:**` with `**Given**`, `**When**`, `**Then**` lines. No other second- or third-level heading appears inside a story, and story ids are unique.
  - `implementation-readiness.md` states its verdict as "ready" or "needs work"; the words bmalph reads as a blocking verdict (a hyphenated, spaced or joined "no" followed by "go", in any letter case) must not appear, not even inside other words.
- Create a GitHub issue or a specs-only pull request only when the caller asks, after the bundle is stable. A failure there is a warning and never discards the bundle.

## Verification

```bash
make bmalph-install
make bmalph-init BMALPH_DRY_RUN=true
```

Hand the finished bundle to `documentation-sync` when it changes guidance, and see `verification-workflow` before declaring the change complete.
