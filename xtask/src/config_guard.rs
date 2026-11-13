//! Protected-configuration guard: compares the working tree with a base ref against the list in
//! `protected-config.toml` and reports what changed without approval.
//!
//! The list is read from the base ref when the base ref holds it, so a change that removes an
//! entry from the list cannot hide the items it protected; a base ref without the list (the
//! bootstrap case) falls back to the list of the working tree, whose items are all new.

mod base;
mod check;
mod list;
#[cfg(test)]
mod memory;

use std::fs;
use std::io;
use std::path::Path;

use thiserror::Error;

pub(crate) use self::base::{BaseTree, Git};
use self::check::Findings;
use self::list::List;

/// The protected list, relative to the root of the repository.
const LIST_FILE: &str = "protected-config.toml";

/// Why the guard could not produce a verdict.
#[derive(Debug, Error)]
pub(crate) enum Failure {
    #[error("base ref {0} is not available")]
    BaseUnavailable(String),
    #[error("cannot run git: {0}")]
    Git(#[from] io::Error),
    #[error("`git {0}` failed")]
    Listing(String),
    #[error("cannot read {LIST_FILE}: {0}")]
    ReadList(io::Error),
    #[error("{LIST_FILE} is neither in the base ref nor in the working tree")]
    NoList,
    #[error("invalid {LIST_FILE}: {0}")]
    InvalidList(toml::de::Error),
    #[error("cannot parse {file} at the base ref: {reason}")]
    InvalidBase { file: String, reason: String },
}

/// The verdict of a run and the text that explains it.
#[derive(Debug)]
pub(crate) struct Outcome {
    /// Nothing is left that the guard refuses.
    pub(crate) clean: bool,
    /// The lines to print, without a trailing newline.
    pub(crate) report: String,
}

/// Compares the working tree in `root` with `base` and renders the verdict.
pub(crate) fn run(root: &Path, base: &str, approved: bool) -> Result<Outcome, Failure> {
    let tree = Git::open(root, base)?;
    evaluate(&tree, approved)
}

/// Prints the protected items of the list in `root`, one per line.
pub(crate) fn list(root: &Path) -> Result<Outcome, Failure> {
    let bytes = fs::read(root.join(LIST_FILE)).map_err(Failure::ReadList)?;
    Ok(Outcome {
        clean: true,
        report: list::parse(&bytes)?.items().join("\n"),
    })
}

/// Loads the list, runs every check and renders the verdict (decision steps 2 to 9).
fn evaluate(tree: &dyn BaseTree, approved: bool) -> Result<Outcome, Failure> {
    let list = load(tree)?;
    let findings = check::check(&list, tree)?;
    Ok(render(&list.approval_label, &findings, approved))
}

fn load(tree: &dyn BaseTree) -> Result<List, Failure> {
    let bytes = match tree.base_file(LIST_FILE)? {
        Some(bytes) => bytes,
        None => tree.work_file(LIST_FILE).ok_or(Failure::NoList)?,
    };
    list::parse(&bytes)
}

/// The header, the recorded lines and the verdict. Modifications and overrides are waived by the
/// approval; a missing required directory never is.
fn render(label: &str, findings: &Findings, approved: bool) -> Outcome {
    let marker = if approved { "[APPROVED] " } else { "" };
    let mut lines = vec!["Configuration Validation".to_owned()];
    lines.extend(
        findings
            .modifications
            .iter()
            .map(|line| format!("{marker}{line}")),
    );
    lines.extend(findings.missing.iter().cloned());
    let changed = !findings.modifications.is_empty();
    let clean = findings.missing.is_empty() && (approved || !changed);
    if clean {
        lines.push(if changed {
            format!("[OK] Configuration validation passed (approved by label {label})")
        } else {
            "[OK] Configuration validation passed".to_owned()
        });
    } else {
        lines.push("[FAIL] Configuration validation failed".to_owned());
        lines.push(format!("Approve with the label {label}"));
    }
    Outcome {
        clean,
        report: lines.join("\n"),
    }
}

#[cfg(test)]
mod tests;
