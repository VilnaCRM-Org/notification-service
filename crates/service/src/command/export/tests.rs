use std::convert::Infallible;
use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;

use super::*;

#[derive(Debug)]
struct DocumentFailed;

impl fmt::Display for DocumentFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the document failed")
    }
}

impl std::error::Error for DocumentFailed {}

/// A writer that accepts every byte and fails when flushed, or fails at once.
#[derive(Debug)]
struct BrokenWriter {
    fail_write: bool,
}

impl Write for BrokenWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            Err(io::Error::other("write failed"))
        } else {
            Ok(buffer.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("flush failed"))
    }
}

/// A file name in the system temporary directory that is removed when the value is dropped.
#[derive(Debug)]
struct TempFile(PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        Self(std::env::temp_dir().join(format!("service-export-{}-{name}", std::process::id())))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        // A file that cannot be removed is left to the operating system's temporary cleanup.
        let _removed = std::fs::remove_file(&self.0);
    }
}

fn text() -> impl Fn() -> Result<String, Infallible> + Copy {
    || Ok("document\n".to_owned())
}

fn args(output: Option<PathBuf>) -> ExportArgs {
    ExportArgs { output }
}

#[test]
fn without_an_output_path_the_document_goes_to_standard_output() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let status = run(text(), &args(None), &mut stdout, &mut stderr);

    assert_eq!(status, ExitCode::SUCCESS);
    assert_eq!(String::from_utf8(stdout).unwrap(), "document\n");
    assert_eq!(stderr, Vec::<u8>::new());
}

#[test]
fn with_an_output_path_the_document_goes_to_the_file_only() {
    let file = TempFile::new("written");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let status = run(
        text(),
        &args(Some(file.0.clone())),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, ExitCode::SUCCESS);
    assert_eq!(std::fs::read_to_string(&file.0).unwrap(), "document\n");
    assert_eq!(stdout, Vec::<u8>::new());
    assert_eq!(stderr, Vec::<u8>::new());
}

#[test]
fn a_path_that_cannot_be_written_is_one_error_line_with_status_1() {
    let missing = TempFile::new("no-such-directory").0.join("spec");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let status = run(text(), &args(Some(missing)), &mut stdout, &mut stderr);

    assert_eq!(status, ExitCode::from(1));
    let message = String::from_utf8(stderr).unwrap();
    assert!(message.starts_with("error: "), "{message}");
    assert_eq!(message.lines().count(), 1);
    assert_eq!(stdout, Vec::<u8>::new());
}

#[test]
fn a_document_that_cannot_be_produced_is_reported_and_nothing_is_written() {
    let file = TempFile::new("not-written");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let status = run(
        || Err::<String, _>(DocumentFailed),
        &args(Some(file.0.clone())),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, ExitCode::from(1));
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "error: the document failed\n"
    );
    assert!(!file.0.exists());
    assert_eq!(stdout, Vec::<u8>::new());
}

#[test]
fn a_failed_write_to_standard_output_is_status_1() {
    let mut stdout = BrokenWriter { fail_write: true };
    let mut stderr = Vec::new();

    let status = run(text(), &args(None), &mut stdout, &mut stderr);

    assert_eq!(status, ExitCode::from(1));
    assert_eq!(String::from_utf8(stderr).unwrap(), "error: write failed\n");
}

#[test]
fn a_failed_flush_of_standard_output_is_status_1() {
    let mut stdout = BrokenWriter { fail_write: false };
    let mut stderr = Vec::new();

    let status = run(text(), &args(None), &mut stdout, &mut stderr);

    assert_eq!(status, ExitCode::from(1));
    assert_eq!(String::from_utf8(stderr).unwrap(), "error: flush failed\n");
}
