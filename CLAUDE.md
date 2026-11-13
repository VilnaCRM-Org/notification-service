# Claude Code Guide

## Start Here

Read these files, in this order, before you change anything:

1. [`AGENTS.md`](AGENTS.md): the working rules, the verification list and the close-out.
2. [`.claude/skills/README.md`](.claude/skills/README.md): the skill inventory and how to pick a skill.
3. [`.claude/skills/AI-AGENT-GUIDE.md`](.claude/skills/AI-AGENT-GUIDE.md): the cycle and the flows for a code change, an API change and an architecture change.

## Project Snapshot

- A Rust 1.99.1 workspace with one crate per bounded-context layer.
- REST and GraphQL interfaces served with axum.
- PostgreSQL access through sqlx.
- Contract snapshots under `.github/`.
- Load tests under `tests/Load/`.
- The architecture model in `workspace.dsl`.

## Command Reference

Run `make help` to list every target. The ordered list of verification targets is in the [Verification section of `AGENTS.md`](AGENTS.md#verification); it is kept there and not repeated here.

## BMAD-METHOD Integration

Use `/bmalph` to navigate phases. Use `/bmad-help` to discover all commands. Use `/bmalph-status` for a quick overview. See `_bmad/COMMANDS.md` for a full command reference.

### Phases

| Phase | Focus | Key Commands |
| --- | --- | --- |
| 1. Analysis | Understand the problem | `/create-brief`, `/brainstorm-project`, `/market-research` |
| 2. Planning | Define the solution | `/create-prd`, `/create-ux` |
| 3. Solutioning | Design the architecture | `/create-architecture`, `/create-epics-stories`, `/implementation-readiness` |
| 4. Implementation | Build it | `/sprint-planning`, `/create-story`, then `/bmalph-implement` for Ralph |

### Workflow

1. Work through Phases 1-3 using BMAD agents and workflows (interactive, command-driven)
2. Run `/bmalph-implement` to transition planning artifacts into Ralph format, then start Ralph

### Management Commands

| Command | Description |
| --- | --- |
| `/bmalph-status` | Show current phase, Ralph progress, version info |
| `/bmalph-implement` | Transition planning artifacts → prepare Ralph loop |
| `/bmalph-upgrade` | Update bundled assets to match current bmalph version |
| `/bmalph-doctor` | Check project health and report issues |

### Available Agents

| Command | Agent | Role |
| --- | --- | --- |
| `/analyst` | Analyst | Research, briefs, discovery |
| `/architect` | Architect | Technical design, architecture |
| `/pm` | Product Manager | PRDs, epics, stories |
| `/sm` | Scrum Master | Sprint planning, status, coordination |
| `/dev` | Developer | Implementation, coding |
| `/ux-designer` | UX Designer | User experience, wireframes |
| `/qa` | QA Engineer | Test automation, quality assurance |

<!-- rust-backend-sdlc:begin -->

## rust-backend-sdlc governance (managed block — do not edit between markers)

The rust-backend-sdlc plugin writes this block and restores it when its setup command runs again. The values it refers to live in the project profile `.claude/rust-sdlc.yml`, which is committed together with this block.

- Skill triage: before implementation or review work, every plugin skill receives a recorded verdict, EXECUTE with evidence or NOT APPLICABLE with a reason. The rule is: every skill verdict recorded, no silent skips. A full skill body is loaded only on EXECUTE.
- Repository rules first: `AGENTS.md` and the skills listed in `.claude/skills/README.md` win over a plugin skill of the same name; the plugin skill adds only its loop, its evidence format and its escalation block.
- Tighten-only quality bars: the `quality.*` keys of the profile may only equal or tighten the values of the gates, and a file listed in `protected-config.toml` is never changed to make a check pass.
- Execution through make: every tool runs through the target that the `make.*` keys of the profile map; the order of the checks is the verification section of `AGENTS.md`.
- Commits and pull-request titles use the form type(#issue): subject, and the git hooks are never bypassed.
- Protected configuration is escalated to a maintainer with the approval label, never edited by an agent.
- Plugin commands, named here in prose: /rs-sdlc drives a feature from issue to finished pull request; /rs-sdlc-setup restores this block; /rs-sdlc-new-service, /rs-sdlc-new-context, /rs-sdlc-issue, /rs-sdlc-plan, /rs-sdlc-implement, /rs-sdlc-review, /rs-sdlc-qa and /rs-sdlc-finish-pr run one step each.

<!-- rust-backend-sdlc:end -->
