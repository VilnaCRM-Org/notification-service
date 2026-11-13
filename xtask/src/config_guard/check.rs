//! The checks of the guard (decision steps 2 to 6): protected files and directories, overriding
//! files, protected TOML tables, protected JSON values and required directories.

use std::collections::BTreeSet;

use serde_json::Value as Json;
use toml::Value as Toml;

use super::Failure;
use super::base::{BaseTree, Mode};
use super::list::{self, JsonValues, List, ManifestTable};

/// What one protected item turned out to be.
#[derive(Debug, Eq, PartialEq)]
enum Verdict {
    /// The base ref does not have the item, so it is new and allowed.
    New,
    /// The working tree has the same item as the base ref.
    Unchanged,
    /// The item differs or is gone; the text is the line to report.
    Modified(String),
}

impl Verdict {
    const fn exists_on_base(&self) -> bool {
        !matches!(self, Self::New)
    }

    fn line(&self) -> Option<&str> {
        match self {
            Self::Modified(line) => Some(line),
            Self::New | Self::Unchanged => None,
        }
    }
}

/// The lines the guard records.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Findings {
    /// Modifications and overrides, which the approval waives.
    pub(crate) modifications: Vec<String>,
    /// Missing required directories, which nothing waives.
    pub(crate) missing: Vec<String>,
}

/// Runs the checks in the order of the decision steps.
pub(crate) fn check(list: &List, tree: &dyn BaseTree) -> Result<Findings, Failure> {
    let files = files(list, tree)?;
    let tables = tables(list, tree)?;
    let values = values(list, tree)?;
    let guarded = [&files, &tables, &values]
        .into_iter()
        .any(|verdicts| verdicts.iter().any(Verdict::exists_on_base));
    let overrides = if guarded {
        overrides(list, tree)?
    } else {
        Vec::new()
    };
    let lines = |verdicts: &[Verdict]| -> Vec<String> {
        verdicts
            .iter()
            .filter_map(Verdict::line)
            .map(str::to_owned)
            .collect()
    };
    Ok(Findings {
        modifications: [lines(&files), overrides, lines(&tables), lines(&values)].concat(),
        missing: missing_directories(list, tree),
    })
}

/// Step 2: every listed file and every file below a listed directory, as the base ref has it.
fn files(list: &List, tree: &dyn BaseTree) -> Result<Vec<Verdict>, Failure> {
    let prefixes: Vec<String> = list
        .directories
        .iter()
        .map(|path| list::directory(path))
        .collect();
    let mut paths: BTreeSet<String> = list.files.iter().cloned().collect();
    let base = tree.base_paths()?;
    paths.extend(
        base.iter()
            .filter(|path| {
                prefixes.iter().any(|prefix| {
                    path.starts_with(prefix.as_str()) || is_directory_entry(path, prefix)
                })
            })
            .cloned(),
    );
    let mut verdicts: Vec<Verdict> = prefixes
        .iter()
        .filter(|prefix| base.iter().any(|path| path.starts_with(prefix.as_str())))
        .map(|prefix| directory(tree, prefix.trim_end_matches('/')))
        .collect();
    for path in &paths {
        verdicts.push(file(tree, path)?);
    }
    Ok(verdicts)
}

/// Whether a base path is the protected directory itself, recorded as a link or a file.
fn is_directory_entry(path: &str, prefix: &str) -> bool {
    path == prefix.trim_end_matches('/')
}

/// A protected directory that has files at the base ref must still be a directory: a link or a
/// file in its place would hide every file below it from the comparison.
fn directory(tree: &dyn BaseTree, path: &str) -> Verdict {
    match tree.work_mode(path) {
        Some(Mode::Link | Mode::Plain | Mode::Executable) => Verdict::Modified(modified(path)),
        Some(Mode::Directory) | None => Verdict::Unchanged,
    }
}

fn modified(path: &str) -> String {
    format!("Modification of locked configuration file is not allowed: {path}")
}

fn file(tree: &dyn BaseTree, path: &str) -> Result<Verdict, Failure> {
    let Some(base) = tree.base_file(path)? else {
        return Ok(Verdict::New);
    };
    let same = tree.work_file(path).as_ref() == Some(&base)
        && tree.work_mode(path) == tree.base_mode(path)?;
    Ok(if same {
        Verdict::Unchanged
    } else {
        Verdict::Modified(modified(path))
    })
}

/// Step 3: a new file named like a protected configuration file.
fn overrides(list: &List, tree: &dyn BaseTree) -> Result<Vec<String>, Failure> {
    let base: BTreeSet<String> = tree.base_paths()?.into_iter().collect();
    let exempt: Vec<String> = list
        .override_exempt
        .iter()
        .map(|path| list::directory(path))
        .collect();
    let paths: BTreeSet<String> = tree
        .work_paths()?
        .into_iter()
        .filter(|path| {
            list.override_names
                .iter()
                .any(|name| ends_with_name(path, name))
        })
        .filter(|path| !list.files.contains(path) && !base.contains(path))
        .filter(|path| {
            !exempt
                .iter()
                .any(|prefix| path.starts_with(prefix.as_str()))
        })
        .collect();
    Ok(paths
        .into_iter()
        .map(|path| format!("Overriding configuration file is not allowed: {path}"))
        .collect())
}

/// Whether `path` is `name` or ends with `/name`: a bare name matches in any directory, a name
/// with a `/` matches a path that ends with it, and neither matches in the middle of a component.
fn ends_with_name(path: &str, name: &str) -> bool {
    path == name || path.ends_with(&format!("/{name}"))
}

/// Step 4: each listed TOML table, compared as a parsed value.
fn tables(list: &List, tree: &dyn BaseTree) -> Result<Vec<Verdict>, Failure> {
    list.manifest_tables
        .iter()
        .map(|item| table(tree, item))
        .collect()
}

fn table(tree: &dyn BaseTree, item: &ManifestTable) -> Result<Verdict, Failure> {
    let Some(bytes) = tree.base_file(&item.file)? else {
        return Ok(Verdict::New);
    };
    let base = toml_document(&bytes).map_err(|reason| invalid(&item.file, reason))?;
    let Some(before) = table_at(&base, &item.table) else {
        return Ok(Verdict::New);
    };
    let work = tree
        .work_file(&item.file)
        .and_then(|bytes| toml_document(&bytes).ok());
    Ok(
        if work
            .as_ref()
            .and_then(|document| table_at(document, &item.table))
            == Some(before)
        {
            Verdict::Unchanged
        } else {
            Verdict::Modified(format!(
                "Modification of locked configuration table is not allowed: {} [{}]",
                item.file, item.table
            ))
        },
    )
}

fn toml_document(bytes: &[u8]) -> Result<Toml, String> {
    toml::from_str(&String::from_utf8_lossy(bytes))
        .map_err(|error: toml::de::Error| error.to_string())
}

/// The value at a dotted table path.
fn table_at<'doc>(document: &'doc Toml, path: &str) -> Option<&'doc Toml> {
    path.split('.')
        .try_fold(document, |value, key| value.get(key))
}

/// Step 5: each listed pointer of each listed JSON file, compared as a parsed value.
fn values(list: &List, tree: &dyn BaseTree) -> Result<Vec<Verdict>, Failure> {
    let mut verdicts = Vec::new();
    for item in &list.json_values {
        verdicts.extend(pointers(tree, item)?);
    }
    Ok(verdicts)
}

fn pointers(tree: &dyn BaseTree, item: &JsonValues) -> Result<Vec<Verdict>, Failure> {
    let base: Option<Json> = tree
        .base_file(&item.file)?
        .map(|bytes| {
            serde_json::from_slice(&bytes).map_err(|error| invalid(&item.file, error.to_string()))
        })
        .transpose()?;
    let work: Option<Json> = tree
        .work_file(&item.file)
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    Ok(item
        .pointers
        .iter()
        .map(|pointer| {
            let Some(before) = base.as_ref().and_then(|document| document.pointer(pointer)) else {
                return Verdict::New;
            };
            if work.as_ref().and_then(|document| document.pointer(pointer)) == Some(before) {
                Verdict::Unchanged
            } else {
                Verdict::Modified(format!(
                    "Modification of locked configuration value is not allowed: {} {pointer}",
                    item.file
                ))
            }
        })
        .collect())
}

/// Step 6: required directories that the working tree lacks.
fn missing_directories(list: &List, tree: &dyn BaseTree) -> Vec<String> {
    list.required_directories
        .iter()
        .filter(|path| !tree.work_dir(path))
        .map(|path| format!("Required directory is missing: {path}"))
        .collect()
}

fn invalid(file: &str, reason: String) -> Failure {
    Failure::InvalidBase {
        file: file.to_owned(),
        reason,
    }
}

#[cfg(test)]
mod tests;
