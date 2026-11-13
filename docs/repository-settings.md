# Repository settings

A repository created from this template starts with the files, workflows and gates, but a few settings live in the hosting platform and cannot be committed. This document lists what a maintainer applies once, in the settings of the repository or of the organisation. Nothing here is checked by a gate except the job names of the pull-request workflows: `make validate-invariants` fails when a job of a pull-request workflow is missing from the tables below.

A service that adds pull-request workflows of its own lists their jobs in a second document, docs/repository-settings-service.md, in the same table format. The template does not ship that file, so a sync never touches it, while this document is replaced by the template's copy on every sync. `make validate-invariants` accepts a job that is listed in either document.

## Contents

- [Required status checks](#required-status-checks)
- [Merging](#merging)
- [Release identity](#release-identity)
- [Private vulnerability reporting](#private-vulnerability-reporting)
- [Review App](#review-app)
- [Secrets and variables](#secrets-and-variables)
- [Labels to create](#labels-to-create)
- [Code scanning](#code-scanning)
- [Governance follow-ups](#governance-follow-ups)

## Required status checks

Every pull-request workflow runs on pull requests to `main` and on pushes to `main`. The tables name every job by its id, with the check name that GitHub shows in brackets. Mark the jobs in the column "Required" as required status checks of the branch protection of `main`.

A job that is skipped counts as passing for a required check. This is intended for a documentation-only change: jobs that depend on the change detection (the "code" jobs) are skipped for a documentation-only change. Every job is a "code" job except the `changes` job itself, the aggregators `mutation-tests` and `bats` (they always run and pass when their matrix was skipped), and `secrets-scan`, `lint-repo`, `invariants`, `docs-check`, `title` and `validate-configuration`, which run for every change. The job `title` is also skipped for Dependabot pull requests and for pull requests labelled `template_sync`. A job that has nothing to do reports why in its log.

The `changes` job of each workflow is a required check too, under a name of its own ("Changes (Quality)", "Changes (Tests)" and so on). This is what makes a failure of the change detection block the merge: when `changes` fails or is cancelled, every code job that needs it is skipped, and the skipped jobs would count as passing, so the red `changes` check is the only signal left. The names are unique because a required check is matched by name: seven checks called "Changes" would all be satisfied by any one of them.

The matrix jobs have an aggregator job with a fixed name. The aggregator is the required check, because the names of the matrix jobs depend on the number of shards and groups: mark `mutation-tests` and `bats`, never `mutation-shard` and `bats-group`.

### `quality.yml` (Quality)

| Job | Check name | Gate | Required |
| --- | --- | --- | --- |
| `changes` | Changes (Quality) | none (decides which jobs run) | yes |
| `format-check` | Formatting check | G1 | yes |
| `static-analysis` | Static analysis | G2 | yes |
| `clippy-sarif-upload` | Static-analysis findings upload | none (publication) | no |
| `complexity` | Complexity limits | G3 | yes |
| `architecture` | Architecture boundaries | G4 | yes |
| `manifest` | Manifest and lock validation | G5 | yes |
| `crate-isolation` | Crate isolation | G29 | yes |
| `query-metadata` | Persistence query metadata | G30 | yes |
| `check-requirements` | Configuration check | G6 | yes |
| `dependency-policy` | Dependency policy | G7 | yes |
| `unused-dependencies` | Unused dependencies | G8 | yes |

### `tests.yml` (Tests)

| Job | Check name | Gate | Required |
| --- | --- | --- | --- |
| `changes` | Changes (Tests) | none | yes |
| `tests` | Tests and coverage | G11, G12, G14 | yes |
| `e2e-tests` | End-to-end tests | G13 | yes |
| `mutation-plan` | Mutation plan | none (sizes the matrix) | no |
| `mutation-shard` | Mutation shard k/n | G15 (one shard) | no |
| `mutation-tests` | Mutation tests | G15 (aggregator) | yes |
| `bats-group` | Bats: group | G16 (one group) | no |
| `bats` | Makefile tests (Bats) | G16 (aggregator) | yes |

### `contracts.yml` (Contracts)

| Job | Check name | Gate | Required |
| --- | --- | --- | --- |
| `changes` | Changes (Contracts) | none | yes |
| `contract-snapshots` | Contract snapshots | G17 | yes |
| `openapi-lint` | OpenAPI lint | G18 | yes |
| `schemathesis` | Schemathesis | G21 | yes |

### `change-control.yml` (Change control)

| Job | Check name | Gate | Required |
| --- | --- | --- | --- |
| `changes` | Changes (Change control) | none | yes |
| `title` | Pull-request title | G28 | yes |
| `validate-configuration` | Protected configuration | G25 | yes |
| `openapi-diff` | OpenAPI breaking changes | G19 | yes |
| `graphql-diff` | GraphQL breaking changes | G20 | yes |

### `load-tests.yml` (Load tests)

| Job | Check name | Gate | Required |
| --- | --- | --- | --- |
| `changes` | Changes (Load tests) | none | yes |
| `smoke-load-tests` | Smoke load tests | G22 | yes |
| `lint-load-tests` | Load-script lint | G24 | yes |

### `security.yml` (Security)

| Job | Check name | Gate | Required |
| --- | --- | --- | --- |
| `changes` | Changes (Security) | none | yes |
| `secrets-scan` | Secrets scan | G9 | yes |
| `image-scan` | Production-image scan | G10 | yes |
| `codeql` | CodeQL | none (publication) | no |

### `repository.yml` (Repository)

| Job | Check name | Gate | Required |
| --- | --- | --- | --- |
| `changes` | Changes (Repository) | none | yes |
| `lint-repo` | Repository lint | G23 | yes |
| `invariants` | Repository invariants | G31 | yes |
| `docs-check` | Documentation drift | G26 | yes |
| `architecture-model` | Architecture model | G27 | yes |

The jobs that are not required have a reason:

- `mutation-plan` sizes the matrix of the mutation run and `mutation-shard` is one slice of it; their aggregator `mutation-tests` fails when any shard fails.
- `bats-group` is one of six groups of the Bats suite; its aggregator `bats` fails when any group fails.
- `clippy-sarif-upload` and `codeql` publish findings to code scanning. They are not gates, and they are skipped where code scanning is not available (see [Code scanning](#code-scanning)).

The workflows `release.yml`, `template-sync-app.yml`, `template-sync-pat.yml` and `scheduled.yml` do not run on pull requests, so they have no status check. A failed scheduled run is a failed workflow run with GitHub's standard notification; no issue is opened automatically.

## Merging

- Allow squash merging and disable merge commits and rebase merging.
- Set the default commit message of a squash merge to "Pull request title and description". The check of the pull-request title (gate G28) validates the title in the form `type(#issue): subject`, and the release reads the squashed commit on `main`. The settings that start with "Pull request title" use the checked title as the subject; "Default message" does not, and then the release version is derived from text that nobody checked. With "Pull request title and description" a `BREAKING CHANGE:` footer written as the last paragraph of the pull-request description becomes the body of the squash commit, which the release reads.
- Require branches to be up to date before merging, so that the required checks ran on the merged result.
- Delete head branches after merge.

## Release identity

`release.yml` pushes a release commit and a tag to `main` with the token of a GitHub App. Branch protection on `main` must let that App push without a pull request: add the App to the list of identities that may bypass the pull-request requirement, or the equivalent allowance of your protection model. The release commit carries `[skip ci]`, so it starts no workflow. Creating the App, its permissions and its two secrets are described in `.github/AUTORELEASE.md`.

Without the App and its secrets the release job succeeds with a notice and nothing is released.

## Private vulnerability reporting

`SECURITY.md` tells reporters to use GitHub private vulnerability reporting on the repository. Enable it under Settings, Code security, "Private vulnerability reporting". Without it the instruction in `SECURITY.md` leads nowhere. Advisories are published as GitHub security advisories.

## Review App

`.coderabbit.yaml` configures the CodeRabbit review bot with the request-changes workflow (`reviews.request_changes_workflow: true`: the bot approves the pull request only once its comments are resolved). The file takes effect only once the CodeRabbit GitHub App is installed on the repository or its organisation; without the App the file is inert: no workflow step acts on its settings. The job `lint-repo` lints it like every YAML file, and the FR97 test below, which runs in the job `bats-group` (group `surface`), pins its content.

The file is the PHP template's `.coderabbit.yaml` unchanged (FR97). The test "FR97: .coderabbit.yaml is the PHP template file with the request-changes workflow" of `make_general.bats` fails when the file is missing, when the line `request_changes_workflow: true` is not under `reviews:`, or when the file's SHA-256 differs from that of the PHP template's file. Change the file only together with the PHP template, and the hash in the test with it.

## Secrets and variables

| Name | Kind | Used by | When absent |
| --- | --- | --- | --- |
| `VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY` | secrets | `release.yml`, `template-sync-app.yml` | the job succeeds with a notice; nothing is released or synchronised |
| `PERSONAL_ACCESS_TOKEN` | secret | `template-sync-pat.yml` | the job succeeds with a notice |
| `CODECOV_TOKEN` | secret | job `tests` | the upload step is skipped with a notice; the coverage gate is unaffected |
| `CODE_SCANNING_ENABLED` | repository variable | `clippy-sarif-upload`, `codeql` and the scheduled `code-scanning` job | unset: published in a public repository, skipped with a notice in a private one |
| `GITHUB_TOKEN` | automatic | every workflow | not a setting; it is read-only except where a job states otherwise |

No step reads a secret in a condition. A job that needs an optional secret writes a notice to its log when the secret is missing and skips the later steps, so a repository without any of these settings stays green. `GPG_PRIVATE_KEY` and `GPG_PASSPHRASE` are not used.

Pull requests from forks and from Dependabot run with a read-only token and without secrets; the steps that need more are skipped by condition.

## Labels to create

Create these five labels. The workflows read the first three by name.

| Label | Purpose |
| --- | --- |
| `approved-config-change` | a maintainer approves a change to protected configuration; without it the job `validate-configuration` fails |
| `approved-breaking-change` | a maintainer approves a breaking change of the REST or GraphQL contract; without it the jobs `openapi-diff` and `graphql-diff` fail |
| `template_sync` | marks the pull requests opened by template sync and exempts them from the title check; the sync action creates it when it is missing |
| `bug` | applied by the bug-report issue template |
| `enhancement` | applied by the feature-request issue template |

Only maintainers should be allowed to apply the two approval labels. A label approves a change for the pull request that carries it; removing the label re-evaluates the checks.

## Code scanning

Two jobs publish findings to code scanning: `clippy-sarif-upload` (the Clippy findings) and `codeql` (CodeQL for Rust and for the workflow files), and the scheduled run repeats both. The jobs that execute the code of a pull request never publish: they hand their findings to a separate job.

- Leave the CodeQL "default setup" switched off. The workflow `security.yml` runs CodeQL itself, and a second setup publishes the same findings twice and conflicts with the advanced configuration.
- In a public repository code scanning is always available and nothing needs to be set.
- In a private repository that has code scanning (GitHub Advanced Security), create the repository variable `CODE_SCANNING_ENABLED` with the value `true`.
- In a private repository without code scanning leave the variable unset, or set it to `false`. The publication steps are then skipped with a notice and the jobs stay green.
- Publication is also skipped, with a notice, for pull requests from forks and from Dependabot, because their token cannot write security events.

## Governance follow-ups

The template does not ship the following. Each is a decision for the owners of a repository, and each is listed here so that none is forgotten:

- CODEOWNERS, so that reviews are requested from the right people (Dependabot has no `reviewers` key any more).
- Rulesets, as an alternative or a complement to classic branch protection, including a rule for the release identity.
- Auto-merge for pull requests whose required checks passed.
- A stale bot for abandoned issues and pull requests.
- OSSF Scorecard, to publish a supply-chain score.
- Multi-architecture container images (the images of the template are built for the architecture of the runner).
- Image publishing with an SBOM and provenance attestation.
- Stored performance baselines and a trend comparison of the load tests.
- An automatic issue for a failed scheduled run.
- Automatic updates of the versions in `make/tool-versions.mk` (a maintainer reviews it, see `docs/operations.md`).
