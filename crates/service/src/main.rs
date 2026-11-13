//! Binary entry point of the service: one call into the library.

use std::process::ExitCode;

fn main() -> ExitCode {
    service::run()
}
