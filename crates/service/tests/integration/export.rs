use std::fs::File;
use std::process::Stdio;

use rstest::rstest;

use crate::support::{Scratch, stderr};

#[rstest]
#[case("openapi-export", "openapi: 3.1.0\n")]
#[case("graphql-export", "\"Input of the create mutation.\"\n")]
fn twenty_runs_with_an_empty_environment_are_byte_identical(
    #[case] command: &str,
    #[case] beginning: &str,
) {
    let scratch = Scratch::new();

    let first = scratch.run(&[command], &[]);

    assert_eq!(first.status.code(), Some(0));
    assert_eq!(stderr(&first), "");
    assert!(
        String::from_utf8(first.stdout.clone())
            .unwrap()
            .starts_with(beginning)
    );
    for _ in 0..19 {
        let next = scratch.run(&[command], &[]);

        assert_eq!(next.status.code(), Some(0));
        assert_eq!(next.stdout, first.stdout);
    }
}

#[rstest]
#[case("openapi-export")]
#[case("graphql-export")]
fn the_output_file_equals_standard_output(#[case] command: &str) {
    let scratch = Scratch::new();
    let on_stdout = scratch.run(&[command], &[]);

    let to_file = scratch.run(&[command, "--output", "document.out"], &[]);

    assert_eq!(to_file.status.code(), Some(0));
    assert_eq!(to_file.stdout, Vec::<u8>::new());
    assert_eq!(stderr(&to_file), "");
    assert_eq!(
        std::fs::read(scratch.path().join("document.out")).unwrap(),
        on_stdout.stdout
    );
}

#[rstest]
#[case("openapi-export")]
#[case("graphql-export")]
fn a_path_that_cannot_be_written_exits_1_with_one_error_line(#[case] command: &str) {
    let scratch = Scratch::new();

    let output = scratch.run(&[command, "--output", "no-such-directory/spec"], &[]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, Vec::<u8>::new());
    let message = stderr(&output);
    assert!(message.starts_with("error: "), "{message}");
    assert_eq!(message.lines().count(), 1);
}

#[rstest]
#[case("openapi-export")]
#[case("graphql-export")]
fn an_output_error_on_standard_output_exits_1(#[case] command: &str) {
    let scratch = Scratch::new();
    let mut child = scratch.service(&[command], &[]);
    child.stdout(Stdio::from(
        File::options().write(true).open("/dev/full").unwrap(),
    ));

    let output = child.output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.starts_with("error: "), "{message}");
}
