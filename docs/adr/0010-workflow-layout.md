# 0010. Workflow layout

Planning decisions: DD-08 (workflow layout), DD-13 (behaviour when secrets are absent).

## Context

Required status checks must stay meaningful for pull requests that change only documentation, and a repository that has not configured optional secrets must still be green. The PHP workflows grouped gates inconsistently and failed when the release secrets were missing.

## Decision

Eleven workflow files group gates by what they share: cache, services and trigger types. Every gate is its own job and therefore its own named status check. The one exception is the tests job, which runs the unit and integration suites once and reports three gates, naming the failing gate in its summary. Steps are written inline, with no composite action and no reusable workflow. Each pull-request workflow starts with a job that detects whether anything other than documentation changed; the other jobs, apart from a short list that must always run, are skipped for a documentation-only change. A skipped job counts as a passing check, where a path filter on the workflow would leave a required check pending. Jobs that depend on labels or on the title live in `change-control.yml`, the only workflow that also reacts to edited and labelled events.

No step reads a secret in a condition. The first step of a job that needs an optional secret maps it to a variable, writes a notice to the log and sets an output; later steps depend on that output. This covers the coverage token, the release App and the personal access token of the sync. Code-scanning uploads are skipped on forks and by default in private repositories, and a repository variable switches them.

## Consequences

Matrix jobs, such as the mutation shards and the Bats groups, have an aggregator job with a fixed name that is the required check. `docs/repository-settings.md` lists the required check names, and the invariants gate fails when a job name is missing from it. A job in which everything was skipped is green and says why.

Alternatives rejected: one workflow per gate, one job with many steps, a third-party path filter action, `continue-on-error` for optional steps, and path filters on the workflow itself.
