//! `service check-config`: validates the configuration of `serve` without opening a socket or
//! contacting a dependency.

use std::io::{self, Write};
use std::process::ExitCode;

use crate::cli::ServeArgs;
use crate::command::{self, REJECTED};
use crate::config::Config;

/// Prints `configuration OK` and returns 0, or prints one line `<VARIABLE>: <reason>` per problem
/// on standard error and returns 2. A failed write is reported as `error: <message>` with status 1.
pub fn run(args: ServeArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> ExitCode {
    match report(args, stdout, stderr) {
        Ok(status) => status,
        Err(error) => command::failure(&error, stderr),
    }
}

fn report(args: ServeArgs, stdout: &mut dyn Write, stderr: &mut dyn Write) -> io::Result<ExitCode> {
    let Err(errors) = Config::from_args(args) else {
        writeln!(stdout, "configuration OK")?;
        return Ok(ExitCode::SUCCESS);
    };
    for problem in errors.problems() {
        writeln!(stderr, "{}: {}", problem.variable, problem.reason)?;
    }
    Ok(ExitCode::from(REJECTED))
}
