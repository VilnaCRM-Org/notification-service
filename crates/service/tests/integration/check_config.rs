use std::fs::File;
use std::process::Stdio;

use rstest::rstest;

use crate::support::{Scratch, VALID_ENVIRONMENT, stderr, stdout};

const FULL_ENVIRONMENT: [(&str, &str); 19] = [
    ("APP_ENV", "dev"),
    ("APP_HOST", "127.0.0.1"),
    ("APP_PORT", "0"),
    ("LOG_LEVEL", "debug"),
    (
        "DATABASE_URL",
        "postgres://app:local-dev-only@127.0.0.1:5432/app",
    ),
    ("DATABASE_MAX_CONNECTIONS", "5"),
    ("DATABASE_ACQUIRE_TIMEOUT", "1s"),
    ("DATABASE_STATEMENT_TIMEOUT", "2s"),
    ("AWS_REGION", "us-east-1"),
    ("AWS_ENDPOINT_URL_SQS", "http://127.0.0.1:1"),
    ("AWS_ACCESS_KEY_ID", "test"),
    ("AWS_SECRET_ACCESS_KEY", "test"),
    ("SQS_CONNECT_TIMEOUT", "300ms"),
    ("SQS_OPERATION_TIMEOUT", "1s"),
    ("SQS_MAX_ATTEMPTS", "2"),
    ("HEALTH_PROBE_TIMEOUT", "2s"),
    ("HTTP_REQUEST_TIMEOUT", "5s"),
    ("HTTP_MAX_BODY_BYTES", "2048"),
    ("SHUTDOWN_GRACE_PERIOD", "3s"),
];

#[test]
fn a_valid_environment_exits_0_and_prints_configuration_ok() {
    let scratch = Scratch::new();

    let output = scratch.run(&["check-config"], &VALID_ENVIRONMENT);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "configuration OK\n");
    assert_eq!(stderr(&output), "");
}

#[test]
fn every_variable_of_the_table_is_accepted_together() {
    let scratch = Scratch::new();

    let output = scratch.run(&["check-config"], &FULL_ENVIRONMENT);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "configuration OK\n");
}

#[test]
fn flags_replace_variables() {
    let scratch = Scratch::new();

    let output = scratch.run(
        &[
            "check-config",
            "--database-url",
            "postgres://u@h/d",
            "--aws-region",
            "eu-west-1",
        ],
        &[],
    );

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn every_invalid_or_missing_variable_is_named_in_one_run_without_its_value() {
    let scratch = Scratch::new();

    let output = scratch.run(
        &["check-config"],
        &[
            ("DATABASE_URL", "mysql://user:hunter2@h/db"),
            ("APP_PORT", "99999"),
            ("APP_ENV", "staging"),
        ],
    );

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    let errors = stderr(&output);
    let lines: Vec<&str> = errors.lines().collect();
    assert_eq!(lines.len(), 4, "{errors}");
    assert!(lines[0].starts_with("APP_ENV: "));
    assert!(lines[1].starts_with("APP_PORT: "));
    assert!(lines[2].starts_with("DATABASE_URL: "));
    assert!(lines[3].starts_with("AWS_REGION: "));
    assert!(!errors.contains("hunter2"));
    assert!(!errors.contains("99999"));
    assert!(!errors.contains("staging"));
}

#[test]
fn an_sqs_endpoint_with_a_malformed_host_is_named_without_its_value() {
    let scratch = Scratch::new();
    let variables = [
        VALID_ENVIRONMENT[0],
        VALID_ENVIRONMENT[1],
        ("AWS_ENDPOINT_URL_SQS", "http://exa mple:4566"),
    ];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    let errors = stderr(&output);
    assert_eq!(
        errors,
        "AWS_ENDPOINT_URL_SQS: must be an http:// or https:// URL\n"
    );
    assert!(!errors.contains("exa mple"));
}

#[rstest]
#[case("0")]
#[case("500us")]
#[case("30d")]
fn a_statement_timeout_outside_its_range_is_rejected(#[case] value: &str) {
    let scratch = Scratch::new();
    let variables = [
        VALID_ENVIRONMENT[0],
        VALID_ENVIRONMENT[1],
        ("DATABASE_STATEMENT_TIMEOUT", value),
    ];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        "DATABASE_STATEMENT_TIMEOUT: must be a duration from 1ms to 24d\n"
    );
}

#[test]
fn a_key_id_without_a_secret_names_the_missing_variable() {
    let scratch = Scratch::new();
    let variables = [
        VALID_ENVIRONMENT[0],
        VALID_ENVIRONMENT[1],
        ("AWS_ACCESS_KEY_ID", "key"),
    ];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "AWS_SECRET_ACCESS_KEY: is required when AWS_ACCESS_KEY_ID is set\n"
    );
}

#[test]
fn a_secret_without_a_key_id_names_the_missing_variable() {
    let scratch = Scratch::new();
    let variables = [
        VALID_ENVIRONMENT[0],
        VALID_ENVIRONMENT[1],
        ("AWS_SECRET_ACCESS_KEY", "secret"),
    ];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "AWS_ACCESS_KEY_ID: is required when AWS_SECRET_ACCESS_KEY is set\n"
    );
}

#[test]
fn both_credentials_set_or_both_unset_is_valid() {
    let scratch = Scratch::new();
    let both = [
        VALID_ENVIRONMENT[0],
        VALID_ENVIRONMENT[1],
        ("AWS_ACCESS_KEY_ID", "key"),
        ("AWS_SECRET_ACCESS_KEY", "secret"),
    ];

    assert_eq!(scratch.run(&["check-config"], &both).status.code(), Some(0));
    assert_eq!(
        scratch
            .run(&["check-config"], &VALID_ENVIRONMENT)
            .status
            .code(),
        Some(0)
    );
}

#[test]
fn the_help_lists_every_flag_and_never_a_value() {
    let scratch = Scratch::new();
    let variables = [
        ("DATABASE_URL", "postgres://u:topsecret@h/db"),
        ("AWS_SECRET_ACCESS_KEY", "credentialvalue"),
        ("APP_PORT", "4242"),
    ];

    let serve = scratch.run(&["serve", "--help"], &variables);
    let root = scratch.run(&["--help"], &variables);

    for output in [&serve, &root] {
        assert_eq!(output.status.code(), Some(0));
        let text = stdout(output);
        assert!(!text.contains("topsecret"));
        assert!(!text.contains("credentialvalue"));
        assert!(!text.contains("4242"));
    }
    let help = stdout(&serve);
    for flag in [
        "--app-env",
        "--app-host",
        "--app-port",
        "--log-level",
        "--database-url",
        "--database-max-connections",
        "--database-acquire-timeout",
        "--database-statement-timeout",
        "--aws-region",
        "--aws-endpoint-url-sqs",
        "--sqs-connect-timeout",
        "--sqs-operation-timeout",
        "--sqs-max-attempts",
        "--health-probe-timeout",
        "--http-request-timeout",
        "--http-max-body-bytes",
        "--shutdown-grace-period",
    ] {
        assert!(help.contains(flag), "{flag} is missing from the help");
    }
    assert!(help.contains("[env: DATABASE_URL]"));
    assert!(!help.contains("AWS_ACCESS_KEY_ID"));
    assert!(!help.contains("AWS_SECRET_ACCESS_KEY"));
    assert!(stdout(&root).contains("check-config"));
}

#[test]
fn the_version_is_the_package_version() {
    let scratch = Scratch::new();

    let output = scratch.run(&["--version"], &[]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        format!("service {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[rstest]
#[case(&["frobnicate"])]
#[case(&["check-config", "--frobnicate"])]
#[case(&[])]
fn a_usage_error_exits_2_with_the_text_on_stderr(#[case] arguments: &[&str]) {
    let scratch = Scratch::new();

    let output = scratch.run(arguments, &VALID_ENVIRONMENT);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).contains("Usage:"));
}

#[test]
fn the_env_file_of_the_working_directory_is_loaded() {
    let scratch = Scratch::new();
    scratch.write(
        ".env",
        "DB_USER=u\nDATABASE_URL=postgres://${DB_USER}:p@h/d\nAWS_REGION=us-east-1\n",
    );

    let output = scratch.run(&["check-config"], &[]);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "configuration OK\n");
}

#[test]
fn the_process_environment_wins_over_the_env_file() {
    let scratch = Scratch::new();
    scratch.write(".env", "APP_PORT=9000\n");

    let invalid_in_the_process = scratch.run(
        &["check-config"],
        &[
            VALID_ENVIRONMENT[0],
            VALID_ENVIRONMENT[1],
            ("APP_PORT", "not-a-port"),
        ],
    );
    scratch.write(".env", "APP_PORT=not-a-port\n");
    let valid_in_the_process = scratch.run(
        &["check-config"],
        &[
            VALID_ENVIRONMENT[0],
            VALID_ENVIRONMENT[1],
            ("APP_PORT", "9001"),
        ],
    );

    assert_eq!(invalid_in_the_process.status.code(), Some(2));
    assert!(stderr(&invalid_in_the_process).starts_with("APP_PORT: "));
    assert_eq!(valid_in_the_process.status.code(), Some(0));
}

#[test]
fn a_directory_without_an_env_file_has_no_configuration() {
    let scratch = Scratch::new();
    assert!(!scratch.path().join(".env").exists());

    let output = scratch.run(&["check-config"], &[]);

    assert_eq!(output.status.code(), Some(2));
    let errors = stderr(&output);
    assert!(errors.starts_with("DATABASE_URL: is required\n"));
    assert!(errors.contains("AWS_REGION: is required\n"));
}

#[test]
fn a_failed_write_of_the_success_line_exits_1() {
    let scratch = Scratch::new();
    let mut command = scratch.service(&["check-config"], &VALID_ENVIRONMENT);
    command.stdout(Stdio::from(
        File::options().write(true).open("/dev/full").unwrap(),
    ));

    let output = command.output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with("error: "));
}

#[test]
fn a_failed_write_of_a_problem_line_exits_1() {
    let scratch = Scratch::new();
    let mut command = scratch.service(&["check-config"], &[]);
    command.stderr(Stdio::from(
        File::options().write(true).open("/dev/full").unwrap(),
    ));

    let output = command.output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
}

#[test]
fn a_failed_write_of_the_help_exits_1() {
    let scratch = Scratch::new();
    let mut command = scratch.service(&["--help"], &[]);
    command.stdout(Stdio::from(
        File::options().write(true).open("/dev/full").unwrap(),
    ));

    let output = command.output().unwrap();

    assert_eq!(output.status.code(), Some(1));
}
