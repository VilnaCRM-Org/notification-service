# 0011. Release flow

Planning decisions: DD-11 (release flow), DD-12 (Dependabot commit prefixes).

## Context

Releases should follow from the commit history without a release pull request, and dependency updates must not cut a release each. The release tool names its own commit in a form that breaks the commit convention of the organisation, and the PHP flow depended on secrets that most forks lack.

## Decision

Every push to `main` runs `scripts/release.sh`, which uses cocogitto: the version comes from conventional commits, the changelog from a custom template that prints no links, and the manifest and lock file from a pre-bump hook. The script then amends the tool's commit to `chore(release): vX.Y.Z [skip ci]`, moves the tag, pushes and creates the GitHub release. The identity is a GitHub App; without its secrets the release is skipped with a notice. While the version is `0.y.z` a breaking change raises the minor number. cocogitto ignores a breaking-change footer below `1.0.0`, and from `1.0.0` on its own reading also marks some lines outside the footer section as breaking, some indented ones included, so the script decides on breaking changes itself among the commits cocogitto reads in the range of the bump, with the footer rule that `CONTRIBUTING.md` states (a `!` before the colon, or the footer in the footer section at the end of the message, as the conventional-commit definition states) and asks for the minor bump below `1.0.0` and the major bump from `1.0.0` on; a breaking commit of any type is released. The first release is the manifest version, and `1.0.0` is cut once by a maintainer through the manual run.

Dependabot commits use the prefix `build` for cargo, docker and compose, `ci` for actions, and `chore` for the toolchain file and the npm package, always with the scope. None of these types bumps a version.

## Consequences

Dependency updates are released with the next feature or fix, not on their own. The changelog template holds no repository name, so it synchronises downstream unchanged. Dependabot titles do not match the organisation's title form and are exempt from the title check. The release rules are documented in `.github/AUTORELEASE.md`.

Alternatives rejected: a conventional-changelog action with unverified manifest rewriting, release pull requests, keeping the tool's own commit name, and the `fix` prefix for cargo updates.
