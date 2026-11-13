//! Workspace tooling, run as `cargo xtask <command>`.

mod arch;
mod cli;
mod config_guard;
mod scaffold;

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    cli::main(
        std::env::args_os(),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}
