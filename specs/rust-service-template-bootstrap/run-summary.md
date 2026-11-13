# Rust Service Template Bootstrap — BMALPH Planning Run

Bundle source: `_bmad-output/planning-artifacts/`

Tracked spec directory: `specs/rust-service-template-bootstrap`

Task: "Using BMAD + BMALPH please setup the same template like we did for PHP,
but with rust ecosystem" — bootstrap `VilnaCRM-Org/rust-service-template` as
the Rust-ecosystem counterpart of `VilnaCRM-Org/php-service-template`.

Scope guard: this bundle is planning only. Implementation follows through the
bmalph Ralph loop on the same branch.

## How the run was orchestrated

The run was autonomous. One orchestrating agent acted as the user's surrogate:
it answered every question the BMAD workflows would have asked, recorded each
answer in [scope-decisions.md](scope-decisions.md), and reviewed each artifact
before the next stage started. Every stage was executed by a focused subagent
framed around one BMALPH command; every planning artifact was then checked by
independent validators that had not written it, and revised from their
findings. Validators were asked to verify by running things where possible,
not only by reading.

## Subagent Execution Log

| Phase | BMALPH command | Artifact | Agents | Status |
| --- | --- | --- | --- | --- |
| Analysis: PHP template inventory | `analyst` | five inventory reports (local, untracked) | 5 | Complete |
| Analysis: Rust ecosystem research | `technical-research` | four research reports with compiled and executed spikes (local, untracked) | 4 | Complete |
| Analysis: capability catalog | `analyst` | `capability-catalog.md` | 1 | Complete |
| Analysis: product brief | `create-brief` | `product-brief.md` | 1 | Complete |
| Planning: PRD | `create-prd` | `prd.md` | 1 | Complete |
| Planning: PRD validation | `validate-prd` | 70 findings (3 critical) | 2 | Complete |
| Planning: PRD revision | `edit-prd` | `prd.md` (126 FRs, 47 NFRs, 31 gates) | 1 | Complete |
| Analysis: research digest | `analyst` | `research.md` | 1 | Complete |
| Solutioning: architecture | `create-architecture` | `architecture.md`, `architecture-delivery.md` | 2 | Complete |
| Solutioning: architecture validation | `validate-architecture` | 84 findings (9 critical) | 3 | Complete |
| Solutioning: architecture revision | `create-architecture` | both documents revised; PRD amended in two places | 2 | Complete |
| Solutioning: epics and stories | `create-epics-stories` | `epics.md` draft (55 stories) | 1 | Complete |
| Solutioning: epics validation | `validate-epics-stories` | 52 findings (7 critical) | 2 | Complete |
| Solutioning: epics revision | `create-epics-stories` | `epics.md` (10 epics, 57 stories), story index | 1 | Complete |
| Solutioning: readiness | `implementation-readiness` | `implementation-readiness.md` | 1 | Complete: READY WITH CONDITIONS |

## Validation Rounds

- Research: no separate validation round. Each Rust research report had to
  compile and run a spike on the pinned toolchain; the reports state what was
  run and what was only read.
- Capability catalog: one mechanical check by its author (all 196 capability
  ids of the five inventories covered exactly once).
- Product brief: no separate round; checked through the PRD validation.
- PRD: 1 round, 2 validators (coverage and traceability; requirement quality).
  70 findings, all applied, 11 in an adapted form. Two later amendments came
  from the architecture validation (the digest-pin rule cannot apply to the
  PostgreSQL image whose tag is `DB_VERSION`).
- Architecture: 1 round, 3 validators (requirements conformance; Rust
  feasibility by building a 13-crate skeleton and running the open questions
  against PostgreSQL and LocalStack; delivery and CI feasibility). 84
  findings; 53 of 54 applied to the core document and all 53 processed for the
  delivery document, with the parts not taken listed in each document.
- Epics and stories: 1 round, 2 validators (coverage and consistency with the
  revised architecture; story quality and schedulability). 52 findings, all
  processed. The critical path went from 22 to 14 stories.
- Implementation readiness: 16 findings, 0 blocking, 12 conditions. Their
  disposition is at the end of the readiness report.

## Results in Numbers

- PHP template: 196 capabilities catalogued (81 kept, 95 fixed, 9 extended, 11
  dropped), 42 capabilities added, 188 defects digested, 28 audit problems
  dispositioned.
- PRD: 126 functional requirements, 47 non-functional requirements, 31
  quality gates, 31 recorded decisions.
- Architecture: 27 core decisions, 25 delivery decisions, 13 crates, 20 gate
  command lines, 107 Make targets, 24 scripts, 16 Bats files, 11 workflows.
- Epics: 10 epics, 57 stories, 339 acceptance criteria, 14 waves, critical
  path of 14 stories. Every FR, NFR and gate is implemented by at least one
  story; every file of the architecture is owned by exactly one story.

## Local Validation

- Nothing of the template is implemented yet, so no gate of the template has
  run. What did run, in the research and validation stages on Rust 1.99.1 with
  Docker: the 13-crate skeleton built from the architecture (format, both lint
  passes, documentation, dependency policy, unused dependencies, 23 Gherkin
  scenarios, Schemathesis with 111 of 111 cases, byte-identical contract
  exports, a production image of 43.7 MB that stays healthy with a read-only
  root file system), `bmalph` preflight on the bundle (no issue), and the
  bmalph story parser on `epics.md` (57 stories, 0 warnings).
- No hosted workflow has run. The repository has no workflow file until the
  last epic.

## Interruptions

- The session's container was restarted once, during the readiness check. No
  artifact was lost; the check was re-run from the cached results of the
  preceding stages.

## Open Questions, Warnings, Blockers

Decisions taken on the maintainers' behalf that they may want to overrule
(details in `scope-decisions.md`):

- The example bounded context is a small real slice (create, read, list
  through the buses, a repository and a migration) instead of the static PHP
  placeholder.
- The cache is in-process behind a port, as the PHP template effectively runs;
  Redis is a documented extension.
- `GET /api/health` keeps the PHP wire contract (`204` / `500`); `GET /ping`
  is the liveness contract.
- Authentication, rate limiting, telemetry exporters, image publishing with
  SBOM and signing, and internationalisation are explicit non-goals of this
  release.
- LocalStack is pinned to the tokenless community tag `3.4.0`; newer images
  need an auth token.

Open until measured:

- `make ci` is estimated at 44 to 56 minutes on a cold machine against the 45
  minutes of NFR35, and the clean-clone job at 55 to 69 minutes against its
  75-minute limit. The PRD is revised only from measured numbers.
- Full mutation testing is estimated at 60 to 80 minutes unsharded; CI plans
  one shard per 150 mutants.

Cannot be verified before the first push or merge: the hosted workflows, the
release flow, template sync, and everything that depends on organisation
secrets, labels and required checks.

Records the readiness review asked for:

- Four PRD readings are in use: `detail` is optional in a problem document
  (FR6); page parameters are 32-bit values (FR22); "two values" in FR92(d)
  means the two seeds; NFR23 counts files that an external reader consumes.
- The core revision lists 8 findings applied in adapted form; its summary
  counted 9.
- The core document said "four shards" in places where the delivery
  document and the stories use one shard per 150 mutants, up to 8. The core
  text (AD-24, QG-17 and the first row of its risk table) was corrected
  together with the deviation entries before the last story; every document
  now counts the shard `k` of `--shard k/n` from 0 to n-1.

Blockers: none.

## Recommended Next Step

Run `bmalph implement`, then the Ralph loop: one git worktree per story, one
implementation agent per story, an independent verification agent that runs
the story's `Verify` line and acceptance criteria and hands the story over.
Stories of Epics 1 to 9 are pushed as they land; from the first workflow file
on, nothing is pushed until the clean-clone verification of the last story has
passed.

## Outcome of the Implementation Run

This section was added after the implementation. The sections above describe
the planning run and are kept as they were written.

- All 57 stories were implemented through the Ralph loop, one git worktree
  per story, and each was verified by an agent that had not written it. The
  last two stories are verification stories: a full `make ci` on the working
  tree and the clean-clone run.
- 53 deviation entries were recorded during the implementation, each with the
  old text, the new text and the proof. They were applied to the architecture,
  the delivery architecture and the epics in two documentation passes and with
  the last fix batches. The tracked copies equal the planning sources.
- The push policy of the paragraph above changed during the run: the workflow
  stories were pushed as soon as Epic 9 was merged, so that the hosted
  workflows could run on the pull request before the last story. The first
  hosted run found five causes of failure. All five were tests or fixtures
  that depended on the environment of the caller (variables of the hosted
  runner, a detached checkout, a variable that make passes on), and each was
  fixed at its cause.
- A final review of the whole tree, in nine areas and with every finding
  checked by a second reviewer, confirmed 44 findings: 14 major, 30 minor and
  none blocking. All were fixed, and each fix was verified independently.
- Measured on 2026-11-11 on a 4-vCPU machine, in a fresh clone without a build
  cache: `make bootstrap` 116 s, `make ci` 24 min 42 s with 297 Makefile
  tests, and `make start` with images and build output present 12 s. The
  estimates under "Open until measured" were higher than the measurement, and
  the budgets of the PRD are unchanged. The conditions of the run and the
  durations of each stage are in `docs/operations.md`.
- Not verified before the merge, because they run only on `main` or on a
  schedule: the release workflow, the scheduled verification and template
  sync.
- After the squash merge of pull request #1 into `main` (commit `3e0bd3e`,
  2026-11-11), the scheduled verification was started by hand on `main`
  (run 43107219243, runner label `ubuntu-24.04`). Its clean-clone job
  passed: `make bootstrap` 447 s and `make ci` 1828 s from a clean clone with
  no restored cache, so NFR34 and the NFR35 budgets of 15 and 45 minutes are
  met on a hosted runner, and the five mutation shards passed. Its
  development-container build failed (FR95): the `dev` stage of the
  `Dockerfile` gave the home directory the sticky bit, and the common-utils
  feature then could not append as root to the user's `.bashrc` under the
  runner's kernel setting `fs.protected_regular`. The development-container
  build failed after 175 s on `3e0bd3e` (run 43107219243); after the
  `Dockerfile` fix of the follow-up pull request (#2) it succeeded in 175 s
  (run 43119953189, job 130857597533, head `027e1f1`), a dispatch of the
  scheduled workflow on that branch.

Review-verified requirements, checked on the repository (readiness condition C11) at commit
`aaa0f3f` on 2026-11-11; the G5 line of the defect digest was re-read at `dcfe564` after
batch B2 of the FR/NFR gate landed, the skill paths were recounted at `8a1aede` and on the
commit of batch B7, and the two digest findings were closed by batch B7 (one fixed in
`docs/repository-settings.md`, one accepted by a recorded architect decision). The record
with every command and count is kept with the implementation evidence.

| Requirement | What was checked | Count | Result |
| --- | --- | --- | --- |
| NFR20 | the protected mutation scope record `.cargo/mutants.toml` and every other way to narrow the scope | 4 scope entries, each with a reason; 0 equivalent-mutant entries; no other exclusion | pass |
| NFR23 | public shared-kernel items, ports, configuration files, tests | 82 public items, 0 without a user; 15 ports, 0 without an adapter and a caller; 48 configuration files in 42 list entries (the tracked configuration files at the repository root and in `.cargo/`, `.claude/settings.json`, `.cog/`, `.config/`, `.devcontainer/`, `.githooks/`, `make/`, `bmalph/`, `.github/dependabot.yml`, `.github/zizmor.yml`, `tests/Load/config.json.dist`, `tests/Load/eslint.config.js`; not counted: `Makefile`, `Cargo.lock`, `LICENSE`), 0 without a reader; 796 Rust tests, 312 Bats tests and 24 scenarios, 0 without an assertion | pass |
| NFR27 | the parity matrix and the 150 lines of the defect digest, read against the repository | 196 of 196 PHP ids (185 delivered, 11 dropped), 42 of 42 additions; digest: 0 replicated, 3 accepted by decision (`/ping` as container health check; the issue number in the changelog scope; the unchanged PHP Spectral ruleset, catalog note CI:D-28), 4 kept in a changed form by a recorded decision, 0 findings, 1 not applicable, 142 not replicated | pass, once the review-bot section of `docs/repository-settings.md` (batch B7) is on `main` and the Spectral decision is recorded in prd.md NFR27 |
| NFR44 | the 11 skill descriptions; every path and identifier the skills name | 11 of 11 descriptions say when to use the skill; on the commit of batch B7: 144 paths, of which 104 resolve from the repository root, 36 relative to a directory the same sentence names, 4 are the generated, git-ignored directories of FR112(d) that the skill names as local (139, 99, 36, 4 at `aaa0f3f`; 140, 100, 36, 4 at `8a1aede`); 32 of 32 identifiers resolve | pass |
| NFR46 | the documents of journeys J1 to J8; placeholders in `SECURITY.md` | 30 documents, all in the repository; 0 placeholders | pass |

The first hosted run after the merge (condition C12) was the first `workflow_dispatch`
run of `Scheduled verification` on `main`: run 43107219243, commit `3e0bd3e`,
2026-11-11.

| Requirement | Job | What that run has to record | Status |
| --- | --- | --- | --- |
| NFR34 | `clean-bootstrap-ci` | result of `make bootstrap` followed by `make ci` on a clean checkout with no restored cache on a GitHub-hosted runner, with the run link and the commit | met: both exit 0, the working tree unchanged |
| NFR35 | `clean-bootstrap-ci` | durations of `make bootstrap` and `make ci` and the disk use of the runner, entered in `docs/operations.md`, "First measurements"; the architect confirms or revises the three budgets (bootstrap 15 minutes, `make ci` 45 minutes, `make start` 60 s) from them with a recorded reason | `make bootstrap` 447 s and `make ci` 1828 s, entered in `docs/operations.md`; the two clean-clone budgets are confirmed: met on a GitHub-hosted ubuntu-24.04 runner (run 43107219243); the log does not show the CPU count; the disk use was not recorded; `make start` with images and build output present: 12 s locally and 12 s on a hosted runner (run 43151995703, job 130963102019, a dispatch on the branch of pull request #2 at `da5ea9c`, 2026-11-12: the clean-clone job stops the stack after `make bootstrap` and times `make start` before `make ci`, failing above 60 s), so the third budget is confirmed as well; runs 43119953189 and 43151995703 measured `make bootstrap` again on a hosted runner, 472 s and 452 s |
| NFR39 | every pull-request workflow | the time from the push to the end of the last required check, warm and for a documentation-only pull request | warm: 9 min 0 s on `e9eef29` and 9 min 21 s on `95547ba` (pull request #2, caches saved by the push of `3e0bd3e`; push = creation of the first check suite, end = last of the 36 required checks, `Makefile tests (Bats)`; the mutation check ended earlier), budget 20 minutes, met; documentation-only: about 1 minute, an estimate from the jobs that run when `code=false` (last of them 57 s and 52 s after the push), budget 5 minutes, until a documentation-only pull request runs |
| FR95 | `devcontainer-build` | result and duration of `npx --yes @devcontainers/cli@0.90.0 build --workspace-folder .` and the last lines of its log | failed after 175 s on `3e0bd3e` (run 43107219243) in the common-utils feature (the cause is in the last bullet above); after the `Dockerfile` fix of pull request #2 it succeeded in 175 s (run 43119953189, job 130857597533, head `027e1f1`), last line `{"outcome":"success",...}`; the runtime steps of FR118 that batch B10 added (start and `postCreateCommand`, `docker info`, `make format-check`, `make start-deps` inside the container) passed in run 43151995703 (job 130963101979, head `da5ea9c`): build 164 s, `up` 7 s (the doctor printed `[OK]` for every tool), `docker info` under 1 s (`Server Version: 29.9.3`), `make format-check` 1 s, `make start-deps` through the inner engine 29 s (both backing services healthy) |
| NFR40 | every job that compiles Rust code or builds an image | the rerun of one unchanged commit and its result | the rerun of `a930b44` (Tests run 43114972119 and Load tests run 43114972020, attempt 2) violated the clause in one job: `Mutation shard 0/1` (job 130846136062) compiled 321 third-party crates in the scratch copy of cargo-mutants after a full cache match; every other job of the two workflows compiled 0 and rebuilt 0 unchanged layers. Batch B9 fixed it (cargo-mutants builds in the checkout with `MUTANTS_IN_PLACE=1`; renamed keys `mutation-in-place` and `bats-tests-in-place`). On head `95547ba` the shard ran in place (`--in-place`, no `Copied source tree` line, 8 of 8 caught as on `e9eef29`) and printed 321 cold, and G15 printed 20 cold, as expected before the merge. The rerun count after the first push to `main`, across all seven pull-request workflows, is still to be measured, by a rerun after that push; fourth recorded exception: up to 77 compile units for a shard whose packages lie outside the saver's selection |
