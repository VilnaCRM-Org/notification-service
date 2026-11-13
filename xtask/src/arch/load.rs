//! Reads `architecture.toml` and turns `cargo metadata` into the workspace model.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::Path;

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand, Package};
use toml::Table;

use super::Failure;
use super::config::{self, Config};
use super::model::{Crate, Dependency, Outside, Reach, Workspace};

/// Reads and validates the configuration file.
pub(crate) fn config(path: &Path) -> Result<Config, Failure> {
    let text = fs::read_to_string(path).map_err(|source| Failure::Read {
        path: path.to_owned(),
        source,
    })?;
    let invalid = |source| Failure::Config {
        path: path.to_owned(),
        source,
    };
    let table = text.parse::<Table>().map_err(invalid)?;
    config::check_keys(&table).map_err(|unknown| Failure::UnknownKey {
        path: path.to_owned(),
        table: unknown.table,
        key: unknown.key,
    })?;
    table.try_into().map_err(invalid)
}

/// Runs `cargo metadata --locked` and extracts the facts the rules need.
pub(crate) fn workspace(manifest: &Path) -> Result<Workspace, Failure> {
    let metadata = MetadataCommand::new()
        .manifest_path(manifest)
        .other_options(vec!["--locked".to_owned()])
        .exec()?;
    let members: BTreeSet<&str> = metadata
        .workspace_packages()
        .into_iter()
        .map(|package| package.name.as_str())
        .collect();
    let edges = edges(&metadata);
    let mut crates: Vec<Crate> = metadata
        .workspace_packages()
        .into_iter()
        .map(|package| member(&metadata, &members, &edges, package))
        .collect();
    crates.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(Workspace {
        crates,
        outside: outside(&metadata),
    })
}

/// The packages without a registry or git source that sit below the workspace root and are not
/// members, sorted by name. A path dependency on a crate outside the root directory is a
/// third-party crate for the rules.
fn outside(metadata: &Metadata) -> Vec<Outside> {
    let mut found: Vec<Outside> = metadata
        .packages
        .iter()
        .filter(|package| package.source.is_none())
        .filter(|package| !metadata.workspace_members.contains(&package.id))
        .filter(|package| package.manifest_path.starts_with(&metadata.workspace_root))
        .map(|package| Outside {
            name: package.name.to_string(),
            manifest: relative(metadata, package),
        })
        .collect();
    found.sort_by(|left, right| left.name.cmp(&right.name));
    found
}

fn relative(metadata: &Metadata, package: &Package) -> String {
    package
        .manifest_path
        .strip_prefix(&metadata.workspace_root)
        .unwrap_or(&package.manifest_path)
        .to_string()
}

fn member(
    metadata: &Metadata,
    members: &BTreeSet<&str>,
    edges: &Edges,
    package: &Package,
) -> Crate {
    Crate {
        name: package.name.to_string(),
        manifest: relative(metadata, package),
        inherits_lints: inherits_lints(package.manifest_path.as_std_path()),
        dependencies: package
            .dependencies
            .iter()
            .map(|dependency| Dependency {
                name: dependency.name.clone(),
                dev: dependency.kind == DependencyKind::Development,
                internal: is_internal(dependency, members),
            })
            .collect(),
        reach: reach_from(&package.name, edges),
    }
}

/// Package name to the names of the packages it depends on through normal or build edges.
type Edges = BTreeMap<String, BTreeSet<String>>;

/// The non-dev edges of the resolved dependency graph, by package name.
fn edges(metadata: &Metadata) -> Edges {
    let names: BTreeMap<_, _> = metadata
        .packages
        .iter()
        .map(|package| (&package.id, package.name.as_str()))
        .collect();
    let mut edges = Edges::new();
    let nodes = metadata
        .resolve
        .iter()
        .flat_map(|resolve| &resolve.nodes)
        .filter_map(|node| Some((names.get(&node.id)?, node)));
    for (from, node) in nodes {
        let targets = node
            .deps
            .iter()
            .filter(|dep| {
                dep.dep_kinds
                    .iter()
                    .any(|info| info.kind != DependencyKind::Development)
            })
            .filter_map(|dep| names.get(&dep.pkg))
            .map(|name| (*name).to_owned());
        edges.entry((*from).to_owned()).or_default().extend(targets);
    }
    edges
}

/// Every package reachable from `start`, each with a shortest chain, sorted by name.
fn reach_from(start: &str, edges: &Edges) -> Vec<Reach> {
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([(start, Vec::<&str>::new())]);
    let mut found = Vec::new();
    while let Some((name, path)) = queue.pop_front() {
        for next in edges.get(name).into_iter().flatten() {
            if seen.insert(next.as_str()) {
                let mut chain = path.clone();
                chain.push(next);
                found.push(Reach {
                    name: next.clone(),
                    path: chain.iter().map(|&step| step.to_owned()).collect(),
                });
                queue.push_back((next, chain));
            }
        }
    }
    found.sort_by(|left, right| left.name.cmp(&right.name));
    found
}

/// Whether a dependency is a workspace member: a path dependency whose package is a member.
fn is_internal(dependency: &cargo_metadata::Dependency, members: &BTreeSet<&str>) -> bool {
    dependency.path.is_some() && members.contains(dependency.name.as_str())
}

/// Whether the manifest has `[lints] workspace = true`. Cargo has already parsed the file, so
/// "unreadable" can only mean "no".
fn inherits_lints(manifest: &Path) -> bool {
    let table = fs::read_to_string(manifest)
        .ok()
        .and_then(|text| text.parse::<Table>().ok());
    let inherited = table
        .as_ref()
        .and_then(|table| table.get("lints")?.get("workspace")?.as_bool());
    inherited == Some(true)
}

#[cfg(test)]
mod tests;
