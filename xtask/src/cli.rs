//! Command line: parses the arguments, runs the command and maps its outcome to an exit status.
//!
//! Exit statuses: 0 the command found nothing to report, 1 it reported violations, 2 it could not
//! run (usage error, unreadable input, failed output). This is the only module that writes output,
//! and the only one that reads the process environment (through clap).

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};

use crate::{arch, config_guard, scaffold};

/// Exit status of a command that could not run, as opposed to one that found violations.
const COULD_NOT_RUN: u8 = 2;

/// Workspace tooling, run as `cargo xtask <command>`.
#[derive(Debug, Parser)]
#[command(name = "xtask", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Check the workspace against the layer model of architecture.toml
    Arch(ArchArgs),
    /// Check the protected configuration against a base ref, or list the protected items
    ConfigGuard(GuardArgs),
    /// Create the three layer crates of a bounded context and register them in the workspace
    NewContext(NewContextArgs),
}

#[derive(Debug, Args)]
struct ArchArgs {
    /// The layer model
    #[arg(long, value_name = "FILE", default_value = "architecture.toml")]
    config: PathBuf,
    /// The manifest of the workspace to check
    #[arg(long, value_name = "FILE", default_value = "Cargo.toml")]
    manifest_path: PathBuf,
    /// Also print one GitHub annotation per violation (set by GitHub Actions)
    #[arg(long = "github-actions", env = "GITHUB_ACTIONS")]
    github: bool,
}

#[derive(Debug, Args)]
struct GuardArgs {
    /// The git ref that the working tree is compared with
    #[arg(long, value_name = "REF", default_value = "origin/main")]
    base: String,
    /// Print the protected items, one per line, instead of checking
    #[arg(long)]
    list: bool,
    /// Waive modifications and overrides (CI sets it when the pull request has the approval label)
    #[arg(
        long = "config-change-approved",
        env = "CONFIG_CHANGE_APPROVED",
        value_parser = approval,
        num_args = 0..=1,
        default_missing_value = "true",
        default_value = "false",
    )]
    approved: bool,
}

/// Reads the approval: `true` waives, `false` and the empty string (an unset CI variable) do not,
/// anything else is a usage error.
fn approval(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "" | "false" => Ok(false),
        other => Err(format!("`{other}` is not `true` or `false`")),
    }
}

#[derive(Debug, Args)]
struct NewContextArgs {
    /// The group directory below `crates/`, in kebab-case
    group: String,
    /// The bounded context, in kebab-case; its name is unique in the workspace
    context: String,
}

/// Runs the command named by `args` (the program name first) and returns its exit status.
pub(crate) fn main<I, T>(args: I, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match Cli::try_parse_from(args) {
        Ok(cli) => execute(cli.command, stdout, stderr),
        Err(error) => usage(&error, stdout, stderr),
    }
}

/// Prints a usage error to stderr (status 2) or the help text to stdout (status 0).
fn usage(error: &clap::Error, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode {
    let text = error.render().to_string();
    if error.use_stderr() {
        finish(
            writeln!(stderr, "{}", text.trim_end()),
            ExitCode::from(COULD_NOT_RUN),
        )
    } else {
        finish(writeln!(stdout, "{}", text.trim_end()), ExitCode::SUCCESS)
    }
}

fn execute(command: Command, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode {
    match command {
        Command::Arch(args) => architecture(args, stdout, stderr),
        Command::ConfigGuard(args) => guard(&args, stdout, stderr),
        Command::NewContext(args) => new_context(&args, stdout, stderr),
    }
}

fn architecture(args: ArchArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode {
    let options = arch::Options {
        config: args.config,
        manifest: args.manifest_path,
        github: args.github,
    };
    match arch::run(&options) {
        Ok(outcome) => {
            let code = if outcome.clean {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            finish(writeln!(stdout, "{}", outcome.report), code)
        }
        Err(failure) => {
            let reason = failure.to_string();
            finish(
                writeln!(
                    stderr,
                    "architecture check could not run: {}",
                    reason.trim_end()
                ),
                ExitCode::from(COULD_NOT_RUN),
            )
        }
    }
}

/// The guard works on the repository in the current directory, as `cargo xtask` is run from its root.
fn guard(args: &GuardArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode {
    let root = Path::new(".");
    let result = if args.list {
        config_guard::list(root)
    } else {
        config_guard::run(root, &args.base, args.approved)
    };
    match result {
        Ok(outcome) => {
            let code = if outcome.clean {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
            finish(writeln!(stdout, "{}", outcome.report), code)
        }
        Err(failure) => finish(
            writeln!(stderr, "configuration guard could not run: {failure}"),
            ExitCode::from(COULD_NOT_RUN),
        ),
    }
}

/// Scaffolds in the workspace in the current directory. A refusal exits with status 1 and changes no
/// file; a workspace that cannot be read or written exits with status 2.
fn new_context(args: &NewContextArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode {
    match scaffold::run(Path::new("."), &args.group, &args.context) {
        Ok(report) => finish(writeln!(stdout, "{report}"), ExitCode::SUCCESS),
        Err(scaffold::Failure::Refused(refusal)) => finish(
            writeln!(stderr, "new-context refused: {refusal}"),
            ExitCode::FAILURE,
        ),
        Err(scaffold::Failure::CouldNotRun(error)) => finish(
            writeln!(stderr, "new-context could not run: {error}"),
            ExitCode::from(COULD_NOT_RUN),
        ),
    }
}

/// A command whose output cannot be written has not reported anything: it never passes.
fn finish(written: io::Result<()>, code: ExitCode) -> ExitCode {
    written.map_or_else(|_error| ExitCode::from(COULD_NOT_RUN), |()| code)
}

#[cfg(test)]
mod tests;
