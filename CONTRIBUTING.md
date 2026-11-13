# Contributing

Thank you for helping. This guide explains how a change gets from an idea to the main branch.

## Issue first

Open an issue before you write code, so the work has a number and a discussion. Use the bug report or feature request template. The issue number is part of the branch name, the commit subjects and the pull-request title.

## Branch naming

Branch from `main` and name the branch after the type and the issue, for example `feat/12-health-endpoint` or `fix/7-empty-page`.

## Commit and pull-request title

Every commit subject and every pull-request title has the form:

```text
type(#issue): subject
```

- `type` is one of `feat`, `fix`, `refactor`, `build`, `chore`, `ci`, `docs`, `style`, `perf`, `test`.
- The scope is mandatory: `#` followed by the issue number.
- A colon and one space separate the scope from the subject.
- The match is case-insensitive.

Accepted: `feat(#12): add health endpoint`, `FIX(#7): handle empty page`.

Rejected: `feat: add x` (no scope), `feat(api): add x` (scope is not an issue number), `feature(#12): add x` (unknown type), `feat(#12):missing space` (no space after the colon), `chore(release): v1.2.0 [skip ci]` (the release job writes this one itself; it never goes through a pull request).

Pull requests are squash-merged, so the checked title becomes the commit subject on `main`. Dependabot and template-sync pull requests are exempt from the title check.

### Version rules

The release is computed from the commit subjects on `main` and the breaking-change footers of their bodies:

- `fix` raises the patch number.
- `feat` raises the minor number.
- A breaking change raises the major number, whatever the type; below `1.0.0` it raises the minor number instead.
- The other types do not trigger a release unless they carry a breaking change.

A breaking change is marked with a `BREAKING CHANGE:` footer: a line that starts with the upper-case token, a colon, a blank and a description, in the footer section of the commit message, the paragraphs at its end that each start with a footer such as `Refs: #12`. The same line elsewhere in the body, indented or inside a sentence is no footer. Pull requests are squash-merged with the setting "Pull request title and description" (see [`docs/repository-settings.md`](docs/repository-settings.md)), so the description becomes the body of the squash commit that the release reads: write the footer as the last paragraph of the pull-request description, after a blank line (see [`.github/AUTORELEASE.md`](.github/AUTORELEASE.md)). The title keeps the form `type(#issue): subject`, and the pull request carries the label `approved-breaking-change`.

## Hooks

Install the Git hooks once per clone:

```bash
make install-hooks
```

The hooks check the commit message and run the fast gates before a commit and the unit tests before a push. `make bootstrap` installs them as well.

## Pull requests

A pull request contains:

- a description of the change and `Closes #<issue>`;
- the type of change;
- how it was tested: the targets you ran, and the ones you did not run with the reason;
- regenerated contract snapshots when the API changed;
- an updated `workspace.dsl` when the architecture changed;
- updated documentation.

Run the verification list in the "Verification" section of [`AGENTS.md`](AGENTS.md) before you open the pull request. The list is kept in one place; do not copy it here.

### Approval labels

- `approved-config-change`: a maintainer adds it when the pull request changes an item listed in `protected-config.toml`. Request it in the description, with the reason. Never change a threshold or a lint setting to get a gate to pass.
- `approved-breaking-change`: a maintainer adds it when the pull request changes the API contract in a breaking way.

Without the label the change-control check fails.

## Review and merge

A maintainer reviews the pull request. Address comments with new commits; the squash merge keeps `main` linear and uses the pull-request title as the subject. Delete the branch after the merge.

## Security

Do not report vulnerabilities in an issue. Follow [`SECURITY.md`](SECURITY.md).
