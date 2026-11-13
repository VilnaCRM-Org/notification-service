//! Architecture boundary check: the Deptrac equivalent for a Cargo workspace.
//!
//! Layers are declared in `architecture.toml`; the facts come from `cargo metadata`, so the check
//! needs no compilation. The rules are pure functions over those two inputs.

mod config;
mod load;
mod model;
mod report;
mod rules;

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Command-line options of the `arch` command.
#[derive(Debug)]
pub(crate) struct Options {
    pub(crate) config: PathBuf,
    pub(crate) manifest: PathBuf,
    pub(crate) github: bool,
}

/// Why the check could not produce a verdict.
#[derive(Debug, Error)]
pub(crate) enum Failure {
    #[error("cannot read {}: {source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("invalid {}: {source}", path.display())]
    Config {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("invalid {}: unknown key `{key}` in {table}", path.display())]
    UnknownKey {
        path: PathBuf,
        table: &'static str,
        key: String,
    },
    #[error("cannot read the workspace: {0}")]
    Metadata(#[from] cargo_metadata::Error),
}

/// The verdict of a run and the report that explains it.
#[derive(Debug)]
pub(crate) struct Outcome {
    /// No rule is broken.
    pub(crate) clean: bool,
    /// Diagnostics, annotations and the one-line summary, without a trailing newline.
    pub(crate) report: String,
}

/// Loads the model and the workspace, applies the rules and renders the report.
pub(crate) fn run(options: &Options) -> Result<Outcome, Failure> {
    let config = load::config(&options.config)?;
    let workspace = load::workspace(&options.manifest)?;
    let violations = rules::check(&config, &workspace);
    let summary = report::Summary::new(&config, &workspace, &violations);
    Ok(Outcome {
        clean: violations.is_empty(),
        report: report::render(&summary, options.github),
    })
}

#[cfg(test)]
mod tests;
