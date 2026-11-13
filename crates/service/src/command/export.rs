//! `service openapi-export` and `service graphql-export`: write a document to a file or to
//! standard output (AD-17).
//!
//! Both commands run before any configuration is read.

use std::io::Write;
use std::process::ExitCode;

use crate::cli::ExportArgs;
use crate::command;

/// Produces the document and writes it to `--output`, or to `stdout` when the option is absent.
///
/// A failure of the document, of the file or of standard output is reported as one line
/// `error: <message>` on `stderr` with status 1.
pub fn run<D, E>(
    document: D,
    args: &ExportArgs,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> ExitCode
where
    D: FnOnce() -> Result<String, E>,
    E: std::error::Error + 'static,
{
    match write(document, args, stdout) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => command::failure(error.as_ref(), stderr),
    }
}

fn write<D, E>(
    document: D,
    args: &ExportArgs,
    stdout: &mut dyn Write,
) -> Result<(), Box<dyn std::error::Error>>
where
    D: FnOnce() -> Result<String, E>,
    E: std::error::Error + 'static,
{
    let text = document()?;
    match &args.output {
        Some(path) => std::fs::write(path, text)?,
        None => stdout
            .write_all(text.as_bytes())
            .and_then(|()| stdout.flush())?,
    }
    Ok(())
}

#[cfg(test)]
mod tests;
