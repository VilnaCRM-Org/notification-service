//! The composition root: configuration, command line, wiring of the layer crates, router assembly
//! and the process entry point `run`.

use std::process::ExitCode;

use clap::Parser as _;

use crate::cli::Cli;

pub mod bootstrap;
pub mod cli;
pub mod command;
pub mod config;
pub mod graphql;
pub mod http;
pub mod openapi;
pub mod telemetry;

/// Runs the service command line and maps the outcome to a process exit status.
///
/// `.env` is loaded first, while the process is still single-threaded; a variable of the process
/// environment wins over the file. Then the command line is parsed and the command runs.
#[must_use]
pub fn run() -> ExitCode {
    let _loaded = dotenvy::dotenv();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    match Cli::try_parse() {
        Ok(cli) => command::run(cli.command, &mut stdout, &mut stderr),
        Err(error) => command::usage_error(&error, &mut stdout, &mut stderr),
    }
}
