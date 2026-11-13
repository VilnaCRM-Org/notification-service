//! The commands of the service binary and the dispatch from the parsed command line to them.
//!
//! This module and its children are the only code of the crate that writes to standard output or
//! standard error.

use std::convert::Infallible;
use std::io::Write;
use std::process::ExitCode;

use crate::cli::{Command, ServeArgs};
use crate::config::Config;
use crate::graphql::schema;
use crate::openapi;

pub mod check_config;
pub mod export;
pub mod healthcheck;
pub mod migrate;
pub mod serve;

/// Exit status of a command that failed while running.
const FAILURE: u8 = 1;

/// Exit status of a command that was rejected: an invalid configuration or a usage error.
pub(crate) const REJECTED: u8 = 2;

/// Runs the parsed command and returns its exit status.
pub fn run(command: Command, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode {
    match command {
        Command::CheckConfig(args) => check_config::run(args, stdout, stderr),
        Command::Serve(args) => start_service(args),
        Command::Migrate(args) => migrate::run(&args, stdout, stderr),
        Command::OpenapiExport(args) => export::run(openapi::yaml, &args, stdout, stderr),
        Command::GraphqlExport(args) => {
            export::run(|| Ok::<_, Infallible>(schema::sdl()), &args, stdout, stderr)
        }
        Command::Healthcheck(args) => healthcheck::run(&args),
    }
}

/// Prints what clap reports for an unparsable command line and returns its exit status.
///
/// Help and version requests are not failures: clap prints them to standard output and the status
/// is 0. Everything else goes to standard error with status 2.
pub fn usage_error(
    error: &clap::Error,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> ExitCode {
    let rendered = error.render();
    let written = if error.use_stderr() {
        write!(stderr, "{rendered}")
    } else {
        write!(stdout, "{rendered}")
    };
    written.map_or_else(
        |write_error| failure(&write_error, stderr),
        |()| ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(REJECTED)),
    )
}

/// Reports a failure of a command as one line `error: <message>` on standard error.
pub(crate) fn failure(error: &dyn std::error::Error, stderr: &mut dyn Write) -> ExitCode {
    let _written = writeln!(stderr, "error: {error}");
    ExitCode::from(FAILURE)
}

/// Starts the service, or reports the invalid configuration as JSON log entries.
fn start_service(args: ServeArgs) -> ExitCode {
    match Config::from_args(args) {
        Ok(config) => serve::run(&config),
        Err(errors) => serve::reject(&errors),
    }
}
