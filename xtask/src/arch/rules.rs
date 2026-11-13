//! The rules: pure functions from the model and the workspace facts to violations.

use super::config::{Config, Layer};
use super::model::{Crate, Dependency, Outside, Reach, Workspace};

/// Which rule a violation breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    Uncovered,
    Ambiguous,
    Layer,
    Context,
    External,
    Transitive,
    Path,
    Lints,
}

impl Rule {
    /// Stable identifier printed in reports (`arch::<id>`).
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Uncovered => "uncovered",
            Self::Ambiguous => "ambiguous",
            Self::Layer => "layer",
            Self::Context => "context",
            Self::External => "external",
            Self::Transitive => "transitive",
            Self::Path => "path",
            Self::Lints => "lints",
        }
    }
}

/// One broken rule, attributed to the crate that must change.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Violation {
    pub(crate) rule: Rule,
    /// Manifest of the offending crate, relative to the workspace root.
    pub(crate) manifest: String,
    pub(crate) message: String,
    pub(crate) hint: String,
}

/// Checks every workspace crate against the configured layers.
pub(crate) fn check(config: &Config, workspace: &Workspace) -> Vec<Violation> {
    workspace
        .crates
        .iter()
        .flat_map(|member| check_crate(config, member))
        .chain(workspace.outside.iter().map(outside))
        .collect()
}

/// A local crate that the workspace uses but that is not a member is never assigned to a layer.
fn outside(crate_: &Outside) -> Violation {
    Violation {
        rule: Rule::Uncovered,
        manifest: crate_.manifest.clone(),
        message: format!(
            "`{}` is not assigned to any layer: it is a local crate that is not a member of the workspace",
            crate_.name
        ),
        hint: "add the crate to `members` of the workspace and name it `<context>-<layer>`, or add its name to the `crates` of a [[layer]]"
            .to_owned(),
    }
}

fn check_crate(config: &Config, member: &Crate) -> Vec<Violation> {
    let mut found: Vec<Violation> = check_lints(member).into_iter().collect();
    let layers = config.layers_of(&member.name);
    match layers.as_slice() {
        [layer] => found.extend(check_layer(config, layer, member)),
        [] => found.push(uncovered(member)),
        several => found.push(ambiguous(member, several)),
    }
    found
}

fn check_lints(member: &Crate) -> Option<Violation> {
    (!member.inherits_lints).then(|| {
        violation(
            Rule::Lints,
            member,
            format!(
                "`{}` does not inherit the workspace lint policy",
                member.name
            ),
            "add `[lints]` with `workspace = true` to its Cargo.toml".to_owned(),
        )
    })
}

fn uncovered(member: &Crate) -> Violation {
    violation(
        Rule::Uncovered,
        member,
        format!("`{}` is not assigned to any layer", member.name),
        "name the crate `<context>-<layer>` or add its name to the `crates` of a [[layer]]"
            .to_owned(),
    )
}

fn ambiguous(member: &Crate, layers: &[&Layer]) -> Violation {
    let names: Vec<&str> = layers.iter().map(|layer| layer.name.as_str()).collect();
    violation(
        Rule::Ambiguous,
        member,
        format!(
            "`{}` matches more than one layer: {}",
            member.name,
            names.join(", ")
        ),
        "make the `crates` patterns of the [[layer]] tables mutually exclusive".to_owned(),
    )
}

fn check_layer(config: &Config, layer: &Layer, member: &Crate) -> Vec<Violation> {
    let mut found: Vec<Violation> = member
        .dependencies
        .iter()
        .flat_map(|dependency| check_dependency(config, layer, member, dependency))
        .collect();
    found.extend(
        member
            .reach
            .iter()
            .filter(|reach| layer.forbids_reaching(&reach.name))
            .map(|reach| transitive(layer, member, reach)),
    );
    found.extend(check_path(layer, member));
    found
}

/// A workspace dependency is judged by the rules `layer` and `context`, a third-party one by
/// the rule `external`.
fn check_dependency(
    config: &Config,
    layer: &Layer,
    member: &Crate,
    dependency: &Dependency,
) -> Vec<Violation> {
    if dependency.internal {
        check_edge(config, layer, member, dependency)
            .into_iter()
            .chain(check_context(config, layer, member, dependency))
            .collect()
    } else {
        check_external(layer, member, dependency)
            .into_iter()
            .collect()
    }
}

/// A dependency on a workspace crate must lead to a layer that `depends_on` lists, or, for a
/// dev-dependency, that `dev_depends_on` lists. Targets without exactly one layer are reported by
/// the rules `uncovered` and `ambiguous` and are not judged here.
fn check_edge(
    config: &Config,
    layer: &Layer,
    member: &Crate,
    dependency: &Dependency,
) -> Option<Violation> {
    let targets = config.layers_of(&dependency.name);
    let [target] = targets.as_slice() else {
        return None;
    };
    let allowed = layer.depends_on.contains(&target.name)
        || (dependency.dev && layer.dev_depends_on.contains(&target.name));
    (!allowed).then(|| {
        let kind = if dependency.dev {
            "have a dev-dependency on"
        } else {
            "depend on"
        };
        violation(
            Rule::Layer,
            member,
            format!(
                "{} must not {kind} {}",
                describe(layer, &member.name),
                describe(target, &dependency.name)
            ),
            format!(
                "{}; declare a port (trait) in the inner layer and implement it in the outer one",
                allowance(layer)
            ),
        )
    })
}

/// A layered crate may depend on crates of its own context and of the shared contexts only.
/// Crates without a context (no `*` in the pattern that matched) are not judged.
fn check_context(
    config: &Config,
    layer: &Layer,
    member: &Crate,
    dependency: &Dependency,
) -> Option<Violation> {
    let targets = config.layers_of(&dependency.name);
    let [target] = targets.as_slice() else {
        return None;
    };
    let from = layer.context_of(&member.name)?;
    let to = target.context_of(&dependency.name)?;
    (from != to && !config.is_shared(to)).then(|| {
        violation(
            Rule::Context,
            member,
            format!(
                "{} must not depend on {}",
                describe(layer, &member.name),
                describe(target, &dependency.name)
            ),
            format!(
                "context `{from}` may depend on itself and on the shared contexts: [{}]; \
                 share what both contexts need through a shared crate",
                config.contexts.shared.join(", ")
            ),
        )
    })
}

/// A third-party dependency must be on the allow-list of the layer; a dev-dependency may also be
/// on the dev allow-list.
fn check_external(layer: &Layer, member: &Crate, dependency: &Dependency) -> Option<Violation> {
    if layer.allows_external(&dependency.name, dependency.dev) {
        return None;
    }
    let (kind, hint) = if dependency.dev {
        (
            "have a dev-dependency on",
            format!(
                "layer `{}` may use the third-party crates [{}] and, as dev-dependencies, [{}]",
                layer.name,
                layer.external.join(", "),
                layer.dev_external.join(", ")
            ),
        )
    } else {
        (
            "depend on",
            format!(
                "layer `{}` may use the third-party crates [{}]; add the crate to `external` of \
                 the layer only if the architecture allows it",
                layer.name,
                layer.external.join(", ")
            ),
        )
    };
    Some(violation(
        Rule::External,
        member,
        format!(
            "{} must not {kind} the third-party crate `{}`",
            describe(layer, &member.name),
            dependency.name
        ),
        hint,
    ))
}

fn transitive(layer: &Layer, member: &Crate, reach: &Reach) -> Violation {
    violation(
        Rule::Transitive,
        member,
        format!(
            "{} reaches `{}` through non-dev dependencies: `{}` -> `{}`",
            describe(layer, &member.name),
            reach.name,
            member.name,
            reach.path.join("` -> `")
        ),
        format!(
            "layer `{}` must not reach [{}]; remove or replace the dependency that pulls `{}` in",
            layer.name,
            layer.forbidden_transitive.join(", "),
            reach.name
        ),
    )
}

/// A crate of a layer with a `directory` sits at `crates/[<group>/]<context>/<directory>`; where
/// the layer has no context, any single name stands in for it.
fn check_path(layer: &Layer, member: &Crate) -> Option<Violation> {
    let directory = layer.directory.as_deref()?;
    let context = layer.context_of(&member.name);
    let mut parts: Vec<&str> = member.manifest.split('/').collect();
    parts.pop();
    let fits = match parts.as_slice() {
        ["crates", between @ .., last] if (1..=2).contains(&between.len()) => {
            *last == directory && context.is_none_or(|context| between.last() == Some(&context))
        }
        _ => false,
    };
    (!fits).then(|| {
        violation(
            Rule::Path,
            member,
            format!(
                "{} is not at `crates/[<group>/]{}/{directory}` but at `{}`",
                describe(layer, &member.name),
                context.unwrap_or("<name>"),
                parts.join("/")
            ),
            format!(
                "move the crate to `crates/[<group>/]{}/{directory}`",
                context.unwrap_or("<name>")
            ),
        )
    })
}

/// A crate as the diagnostics name it: `` `a-domain` (layer `domain`, context `a`) ``.
fn describe(layer: &Layer, name: &str) -> String {
    layer.context_of(name).map_or_else(
        || format!("`{name}` (layer `{}`)", layer.name),
        |context| format!("`{name}` (layer `{}`, context `{context}`)", layer.name),
    )
}

fn allowance(layer: &Layer) -> String {
    let normal = format!(
        "layer `{}` may depend on: [{}]",
        layer.name,
        layer.depends_on.join(", ")
    );
    if layer.dev_depends_on.is_empty() {
        normal
    } else {
        format!(
            "{normal} and, as dev-dependencies, on: [{}]",
            layer.dev_depends_on.join(", ")
        )
    }
}

fn violation(rule: Rule, member: &Crate, message: String, hint: String) -> Violation {
    Violation {
        rule,
        manifest: member.manifest.clone(),
        message,
        hint,
    }
}

#[cfg(test)]
mod tests;
