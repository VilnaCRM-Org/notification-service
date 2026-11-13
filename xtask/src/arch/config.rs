//! Schema of `architecture.toml`, pattern matching and context extraction.

use serde::Deserialize;
use toml::{Table, Value};

/// Keys of the top level of the file.
const TOP_KEYS: [&str; 2] = ["contexts", "layer"];
/// Keys of the `[contexts]` table.
const CONTEXTS_KEYS: [&str; 1] = ["shared"];
/// Keys of a `[[layer]]` table.
const LAYER_KEYS: [&str; 8] = [
    "name",
    "crates",
    "directory",
    "depends_on",
    "dev_depends_on",
    "external",
    "dev_external",
    "forbidden_transitive",
];

/// The part of the file that the rules read.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Config {
    /// The `[contexts]` table.
    #[serde(default)]
    pub(crate) contexts: Contexts,
    /// Layers, matched against workspace crate names.
    #[serde(default, rename = "layer")]
    pub(crate) layers: Vec<Layer>,
}

/// Which bounded contexts every context may depend on besides itself.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Contexts {
    /// The shared kernel: contexts that every other context may depend on.
    #[serde(default)]
    pub(crate) shared: Vec<String>,
}

/// One architectural layer and what it may depend on.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct Layer {
    pub(crate) name: String,
    /// Crate-name patterns (`*` matches any run of characters, once).
    pub(crate) crates: Vec<String>,
    /// Name of the directory that holds a crate of this layer; switches on the rule `path`.
    #[serde(default)]
    pub(crate) directory: Option<String>,
    /// Layers this layer may depend on.
    #[serde(default)]
    pub(crate) depends_on: Vec<String>,
    /// Layers this layer may depend on in addition, as dev-dependencies only.
    #[serde(default)]
    pub(crate) dev_depends_on: Vec<String>,
    /// Third-party crates that a crate of this layer may depend on (patterns).
    #[serde(default)]
    pub(crate) external: Vec<String>,
    /// Third-party crates that are allowed in addition, as dev-dependencies only (patterns).
    #[serde(default)]
    pub(crate) dev_external: Vec<String>,
    /// Crates that must not be reachable through non-dev dependencies (patterns).
    #[serde(default)]
    pub(crate) forbidden_transitive: Vec<String>,
}

/// A key that the schema does not know.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UnknownKey {
    /// The table that holds the key, as written in the file.
    pub(crate) table: &'static str,
    pub(crate) key: String,
}

/// Rejects every key that the schema does not know, in any table.
pub(crate) fn check_keys(table: &Table) -> Result<(), UnknownKey> {
    find_unknown(table, &TOP_KEYS, "the top level")?;
    if let Some(contexts) = table.get("contexts").and_then(Value::as_table) {
        find_unknown(contexts, &CONTEXTS_KEYS, "[contexts]")?;
    }
    let layers = table
        .get("layer")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    layers
        .filter_map(Value::as_table)
        .try_for_each(|layer| find_unknown(layer, &LAYER_KEYS, "[[layer]]"))
}

fn find_unknown(table: &Table, known: &[&str], name: &'static str) -> Result<(), UnknownKey> {
    let mut keys = table.keys().filter(|key| !known.contains(&key.as_str()));
    keys.next().map_or(Ok(()), |key| {
        Err(UnknownKey {
            table: name,
            key: key.clone(),
        })
    })
}

impl Config {
    /// Whether every context may depend on the context `name`.
    pub(crate) fn is_shared(&self, name: &str) -> bool {
        self.contexts.shared.iter().any(|shared| shared == name)
    }

    /// Every layer whose patterns match the crate `name`.
    pub(crate) fn layers_of(&self, name: &str) -> Vec<&Layer> {
        self.layers
            .iter()
            .filter(|layer| layer.covers(name))
            .collect()
    }
}

impl Layer {
    /// Whether one of the crate patterns matches `name`.
    pub(crate) fn covers(&self, name: &str) -> bool {
        self.crates
            .iter()
            .any(|pattern| pattern_matches(pattern, name))
    }

    /// Whether a crate of this layer may depend on the third-party crate `name`: it is in
    /// `external`, or in `dev_external` when the dependency is a dev-dependency.
    pub(crate) fn allows_external(&self, name: &str, dev: bool) -> bool {
        listed(&self.external, name) || (dev && listed(&self.dev_external, name))
    }

    /// Whether a crate of this layer must not reach the crate `name` through non-dev edges.
    pub(crate) fn forbids_reaching(&self, name: &str) -> bool {
        listed(&self.forbidden_transitive, name)
    }

    /// The bounded context of the crate `name`: the text that the `*` of the first matching
    /// pattern matched (`some-module-domain` against `*-domain` gives `some-module`).
    pub(crate) fn context_of<'name>(&self, name: &'name str) -> Option<&'name str> {
        self.crates
            .iter()
            .find_map(|pattern| capture(pattern, name))
            .filter(|context| !context.is_empty())
    }
}

/// Whether one of the `patterns` matches `name`.
fn listed(patterns: &[String], name: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| pattern_matches(pattern, name))
}

/// Whether `name` matches `pattern`, where one `*` matches any run of characters.
pub(crate) fn pattern_matches(pattern: &str, name: &str) -> bool {
    pattern
        .split_once('*')
        .map_or(pattern == name, |_| capture(pattern, name).is_some())
}

/// The text that the `*` of `pattern` matches in `name`; `None` without a `*` or without a match.
pub(crate) fn capture<'name>(pattern: &str, name: &'name str) -> Option<&'name str> {
    let (prefix, suffix) = pattern.split_once('*')?;
    name.strip_prefix(prefix)?.strip_suffix(suffix)
}

#[cfg(test)]
mod tests;
