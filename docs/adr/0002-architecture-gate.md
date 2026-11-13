# 0002. Architecture gate

Planning decisions: AD-03 (bounded-context isolation and the architecture gate), AD-25 (protected configuration and guard semantics).

## Context

Cargo enforces that a crate uses only what its manifest lists, but not which crates it may list, nor that two bounded contexts stay apart. Adding a context must not require editing a rule file, and the rule files themselves must not be loosened silently to get a build to pass.

## Decision

`cargo xtask arch` reads `architecture.toml` and `cargo metadata --locked` and enforces eight rules: every crate matches exactly one layer pattern, a crate may depend only on the layers its layer allows, a layered crate may depend only on its own context and the shared kernel, a crate may use only the third-party crates its layer allows, forbidden crates must not be reachable through non-dev edges, a layered crate sits at `crates/[<group>/]<context>/<layer>`, and every manifest inherits the workspace lints. A local crate below the workspace root that the dependency graph uses but that is not a workspace member (for example one listed in `[workspace] exclude`) is reported by the first rule, because no layer or lint rule would see it. The bounded context of a crate is the text matched by `*` in its layer pattern, so contexts are derived from crate names and no context table exists. The exit status is 0 when clean, 1 on violations and 2 when the check could not run.

One file, `protected-config.toml`, lists every file, directory and file section that configures a gate. `cargo xtask config-guard --base <ref>` compares the working tree with the base ref. A modified, deleted or renamed protected file is a violation, and so is a change of its kind: the executable bit of a hook, a file or a directory turned into a symbolic link, or a link in place of a protected file or directory, or in a directory above a file that the base holds (the guard compares the entry type and mode of the base tree with the working tree and never follows links, so content reached through a link that is new at a path the base does not have is protected only as the link text, and such a new link is not reported, exactly like a new plain file, which is reported only when its name is in `override_names`); the lint table of the root manifest and the load-test budgets are compared as parsed values; a new file that would override a protected configuration file for part of the tree is a violation. A deliberate change is waived only when `CONFIG_CHANGE_APPROVED=true`, which CI sets when the pull request carries the label `approved-config-change`. The same list feeds the assistant hook and the guidance.

## Consequences

Adding a context changes no line of `architecture.toml`. The checker and the guard are template-owned code under the same test, coverage and mutation gates as the rest. The gate does not see `std` use inside a Domain crate; wall-clock and socket APIs are banned by clippy instead, and further domain purity is a review rule. The guard makes every threshold change visible in review, at the cost of a label on each such pull request.

Alternatives rejected: explicit context tables, module-level checkers (they miss violations, need nightly or carry an unsuitable licence), a per-layer allow-list of module names, a shell guard that cannot be held to the coverage gate, and protecting the whole root manifest.
