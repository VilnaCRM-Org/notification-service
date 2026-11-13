//! Human-readable (and GitHub-annotated) rendering of the verdict.

use super::config::Config;
use super::model::Workspace;
use super::rules::Violation;

/// Everything the report needs.
#[derive(Debug)]
pub(crate) struct Summary<'run> {
    violations: &'run [Violation],
    crates: usize,
    layers: usize,
}

impl<'run> Summary<'run> {
    pub(crate) const fn new(
        config: &Config,
        workspace: &Workspace,
        violations: &'run [Violation],
    ) -> Self {
        Self {
            violations,
            crates: workspace.crates.len(),
            layers: config.layers.len(),
        }
    }
}

/// One block per violation, then a one-line verdict, separated by blank lines.
pub(crate) fn render(summary: &Summary<'_>, github: bool) -> String {
    let mut parts: Vec<String> = summary
        .violations
        .iter()
        .map(|violation| block(violation, github))
        .collect();
    parts.push(verdict(summary));
    parts.join("\n\n")
}

fn verdict(summary: &Summary<'_>) -> String {
    let (crates, layers) = (summary.crates, summary.layers);
    if summary.violations.is_empty() {
        format!(
            "architecture: ok. All {crates} crate(s) are assigned to exactly one layer \
             ({layers} layer(s)); 0 violation(s)"
        )
    } else {
        format!(
            "architecture: FAILED. {} violation(s) in {crates} crate(s), {layers} layer(s) checked",
            summary.violations.len()
        )
    }
}

/// The compiler-style diagnostic of one violation, followed by its GitHub annotation when asked.
fn block(violation: &Violation, github: bool) -> String {
    let rule = violation.rule.id();
    let (manifest, message, hint) = (&violation.manifest, &violation.message, &violation.hint);
    let diagnostic = format!("error[arch::{rule}]: {message}\n  --> {manifest}\n   = help: {hint}");
    if github {
        let file = escape_property(manifest);
        let title = escape_property(&format!("Architecture ({rule})"));
        let data = escape_data(message);
        format!("{diagnostic}\n::error file={file},title={title}::{data}")
    } else {
        diagnostic
    }
}

/// The message of a workflow command: `%`, CR and LF are escaped (`%` first).
fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// A property value of a workflow command: the message escapes plus `:` and `,`.
fn escape_property(text: &str) -> String {
    escape_data(text).replace(':', "%3A").replace(',', "%2C")
}

#[cfg(test)]
mod tests;
