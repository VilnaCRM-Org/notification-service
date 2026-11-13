# Creating a service from the template

This guide turns a copy of the template into your own service. The template is built so that very little has to change: the repository name appears in one place of the workspace manifest, and everything else is derived.

## 1. Create the repository

Use the GitHub "Use this template" button, or clone the template and push it to a new repository. Name the checkout directory after the service. Make derives the project name from the directory name (lower-cased, characters outside `a-z0-9_-` replaced by `-`) and uses it for the Compose project and the image names (`<project>-app:dev`, `<project>-app:prod`, `<project>-load`), so no other file names the project. On a GitHub runner the checkout directory equals the repository name, so CI needs no edit either.

## 2. Set the package identity

Edit `[workspace.package]` in the root `Cargo.toml` and change these values:

| Key | What it feeds |
| --- | --- |
| `description` | the service name; the OpenAPI title is `<description> API` |
| `repository` | the repository URL; the OpenAPI contact URL |
| `version` | the version of your first release: set it to `0.1.0` or to the first version you want. A repository created from the template copies the manifest as the template last released it, and none of its tags; refresh `Cargo.lock` afterwards with `cargo update --workspace` |

These values and the checkout directory name are what a new service changes for its identity. After the edit, regenerate the contract snapshots (`make generate-openapi-spec`) so the committed OpenAPI snapshot carries the new title, and commit the result.

## 3. Update the service-owned files

These files name the template and belong to the service from the start; the template sync never touches them:

- `bmalph/config.json`: set `name` and `description`.
- The badges in `README.md`: point each badge at your repository, and rewrite the introduction to describe your service.
- The planning bundle under `specs/`: replace it with your own planning documents, or remove it.
- `CHANGELOG.md`: reset it to the cocogitto skeleton and remove the release sections of the template, so that your changelog starts with your first release.

## 4. Keep or remove the example context

The template ships one example bounded context under the directory crates/company-subdomain with its migration, snapshots and end-to-end features. Keep it while you learn the layout, then remove the crates, the three paths in `[workspace].members`, the `[workspace.dependencies]` entries, the example migration, the example feature files, the wiring in the `service` crate and the components in `workspace.dsl`. Run `make ci` afterwards: every gate must pass without the example. The Bats suite never depends on the example context, so it keeps passing.

## 5. Add a bounded context

```bash
make new-context group=<group> context=<context>
```

The command creates `crates/<group>/<context>/{domain,application,infrastructure}`, registers the three crates in the workspace, refreshes `Cargo.lock` and prints the next steps. Both names must be lower-case words separated by hyphens; the names `shared`, `service`, `xtask`, `test-support` and `e2e-tests` are reserved. The command refuses, before it writes anything, a context name that already exists: as a directory directly below `crates/` or in any group, as a registered member whose package is named `<context>-<layer>`, or as a layer directory inside a directory of that name. The new crates pass the architecture and crate-isolation gates without an edit to a protected file.

Then wire the context into the service:

1. Add `[workspace.dependencies]` path entries for the new crates when the first crate depends on them.
2. Write the code layer by layer (domain, application, infrastructure) and put its fakes in `test-support`.
3. Wire it in the `service` crate: adapters in `Adapters`, handlers in `bootstrap::router`, a state field in `AppState`, its router in `openapi::api_router` and its GraphQL roots in `graphql::schema`.
4. Add the components to `workspace.dsl`; until then the architecture-model gate fails.
5. Add a migration with `make create-migration name=<snake_case_description>`, apply it with `make migrate`, run `make sqlx-prepare` and commit `.sqlx/` with the migration.
6. Regenerate the contract snapshots with `make generate-openapi-spec` and `make generate-graphql-spec` in the same change.

`architecture.toml` and every other protected item stay untouched, except for the labelled change below.

## 6. Where a service adds its own Make targets

Never edit the template's Makefile for service targets, because the sync would overwrite the edit. Define them in the file make/service.mk, which the `Makefile` loads with `-include`, and document them in docs/make-targets-service.md. The template ships neither file, so a sync cannot change them. Likewise, list the jobs of the pull-request workflows that your service adds in docs/repository-settings-service.md, in the table format of docs/repository-settings.md and with a column that says whether each job is a required check. The template ships no such file, so a sync cannot change it, and the repository invariants (rule (d) of `make validate-invariants`) accept a job that is listed in either document. The documentation check reads the target reference of the template and this second reference, and requires every target to appear in `make help` and in one of the two documents.

## 7. Template-owned files a service extends

A service adds to these files, and the template sync ignores them so that the additions survive. When the template changes one of them, apply the change by hand from the sync guide.

| File | What a service adds |
| --- | --- |
| `.env`, `.env.test`, `docs/environment.md` | the variables of its own adapters and their reference entries |
| `docs/adr/` | its own decision records |
| `.github/dependabot.yml` | ecosystems and ignore rules |
| `.claude/skills/README.md` | the index lines of its own skills |
| `architecture.toml` | third-party crates on a layer's allow-list (a labelled change) |
| `deny.toml`, `.trivyignore`, `.gitleaksignore`, `_typos.toml`, `.cargo/mutants.toml` | dated exceptions: advisories and licences, image findings, scan fingerprints, accepted words, equivalent mutants |
| `crates/internal/health-check/domain/src/value_object/dependency.rs` and `crates/internal/health-check/infrastructure/src/probe.rs` | a dependency and the module declaration of its probe; a probe that needs a new client crate also changes `crates/internal/health-check/infrastructure/Cargo.toml`, which the template synchronises, so list that file by its full path in your `.templatesyncignore` in the same pull request, or the next sync removes the dependency |
| `crates/service/Cargo.toml` and the bootstrap, command-line, configuration, routes, OpenAPI and GraphQL schema modules of that crate | wiring, commands, configuration, routes and schema roots of its contexts |
| `tests/support/`, `tests/e2e/` | test doubles, world and step definitions |
| `crates/service/src/http/router/tests.rs` and `crates/service/tests/integration/{bootstrap,migrate,serve,panic}.rs` | the tests that construct `Adapters` and call the routes, the table and the migration of the example context; keep them in step with your wiring |
| files changed through an extension seam (see `docs/extension-seams.md`) | the change the seam describes; the template does not ignore these files, so add each synchronised path you change, in full from the root of the repository, to your `.templatesyncignore` in the same pull request and apply later template changes to it by hand |

`Makefile` and `docs/make-targets.md` are synchronised, not ignored; a service adds targets through the fragment of section 6 instead.

## 8. Secrets, variables and labels to create

| Name | Kind | Used by | When absent |
| --- | --- | --- | --- |
| `VILNACRM_APP_ID`, `VILNACRM_APP_PRIVATE_KEY` | secrets | the release workflow and the App flavour of the template sync | the job succeeds with a notice; nothing is released or synchronised |
| `PERSONAL_ACCESS_TOKEN` | secret | the token flavour of the template sync | the job succeeds with a notice |
| `CODECOV_TOKEN` | secret | the tests workflow | the upload is skipped with a notice; the coverage gate is unaffected |
| `CODE_SCANNING_ENABLED` | repository variable | code-scanning publication | unset: published in a public repository, skipped in a private one; `true` enables it in a private repository that has code scanning; `false` switches it off |
| `approved-config-change`, `approved-breaking-change` | labels | the change-control workflow | the gate fails on a protected or a breaking change |
| `template_sync` | label | the sync workflows | created by the sync action |
| `bug`, `enhancement` | labels | the issue templates | GitHub defaults |

The release App needs Contents, Pull requests and Workflows (read and write) and Metadata (read), and must be allowed to push to `main` in the branch protection. `docs/repository-settings.md` lists the required status checks and the other repository settings.

## 9. Choose one sync flavour

The template ships two workflows that merge template changes into your repository through a pull request labelled `template_sync`:

| Flavour | Credentials | Schedule | Guide |
| --- | --- | --- | --- |
| App | `VILNACRM_APP_ID` and `VILNACRM_APP_PRIVATE_KEY` | Mondays | `TEMPLATE_SYNC_APP.md` in the `.github` directory |
| Token | `PERSONAL_ACCESS_TOKEN` (fine-grained: Contents, Pull requests and Workflows read and write; Metadata read) | the first of each month | `TEMPLATE_SYNC_PAT.md` in the `.github` directory |

Keep one and delete the other workflow (`template-sync-app.yml` or `template-sync-pat.yml` in `.github/workflows`) and its guide, and add the deleted paths to `.templatesyncignore`: the sync would otherwise restore them. Write each with its directory, because every line of that file is a git pathspec from the root of the repository and a bare file name is a file at the root only (the guides describe the format). This document and both guides name the two flavours by bare file name, and the guide that stays is synchronised from the template, so nothing needs editing for the documentation check to pass after the deletion. When both App secrets exist the token flavour skips itself with a notice. A sync that changes protected configuration needs the label `approved-config-change`. The guides also list the template changes that do not propagate because their files are ignored, and say that a file the template deletes or renames is not removed from your repository.

## 10. Check the result

```bash
make doctor
make bootstrap
make ci
```

`make ci` runs every gate; the ordered list is in the verification section of [`AGENTS.md`](../AGENTS.md#verification). A service that passes it is as strict as the template.
