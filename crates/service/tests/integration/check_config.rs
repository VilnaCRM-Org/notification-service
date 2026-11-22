use std::fs::File;
use std::process::Stdio;

use rstest::rstest;

use crate::support::{Scratch, VALID_ENVIRONMENT, stderr, stdout};

const FULL_ENVIRONMENT: [(&str, &str); 31] = [
    ("APP_ENV", "dev"),
    ("APP_HOST", "127.0.0.1"),
    ("APP_PORT", "0"),
    ("LOG_LEVEL", "debug"),
    ("HEALTH_PROBE_TIMEOUT", "2s"),
    ("HTTP_REQUEST_TIMEOUT", "5s"),
    ("HTTP_MAX_BODY_BYTES", "2048"),
    ("SHUTDOWN_GRACE_PERIOD", "3s"),
    ("EVENT_TYPES", "user.created,order.paid"),
    ("KAFKA_BOOTSTRAP_SERVERS", "a:9092,[::1]:9093"),
    ("KAFKA_TOPIC", "notifications"),
    ("KAFKA_SECURITY_PROTOCOL", "sasl_ssl"),
    ("KAFKA_SASL_MECHANISM", "SCRAM-SHA-512"),
    ("KAFKA_SASL_USERNAME", "reader"),
    ("KAFKA_SASL_PASSWORD", "local-dev-only"),
    ("KAFKA_CLIENT_ID", "notification-service-a"),
    ("KAFKA_REQUEST_TIMEOUT", "5s"),
    ("KAFKA_CONNECT_TIMEOUT", "1s"),
    ("KAFKA_FETCH_MAX_WAIT", "250ms"),
    ("KAFKA_RETRY_DELAY", "2s"),
    ("EVENT_MAX_DATA_BYTES", "1024"),
    ("EVENT_WINDOW_MAX_EVENTS", "1000"),
    ("EVENT_WINDOW_MAX_BYTES", "4194304"),
    ("EVENT_WINDOW_START_EVENTS", "0"),
    ("EVENT_REPLAY_MAX_EVENTS", "5000"),
    ("EVENT_REPLAY_PERMITS", "2"),
    ("EVENT_TAIL_STALE_AFTER", "30s"),
    ("SUBSCRIBER_BUFFER", "64"),
    ("MAX_SUBSCRIBERS", "500"),
    ("STREAM_KEEP_ALIVE", "20s"),
    (
        "CORS_ALLOWED_ORIGINS",
        "https://app.example.com,http://localhost:3000",
    ),
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
        &["check-config", "--kafka-bootstrap-servers", "a:9092"],
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
            ("CORS_ALLOWED_ORIGINS", "ftp://user:hunter2@h/queue"),
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
    assert!(lines[2].starts_with("KAFKA_BOOTSTRAP_SERVERS: "));
    assert!(lines[3].starts_with("CORS_ALLOWED_ORIGINS: "));
    assert!(!errors.contains("hunter2"));
    assert!(!errors.contains("99999"));
    assert!(!errors.contains("staging"));
}

#[test]
fn an_origin_with_a_malformed_host_is_named_without_its_value() {
    let scratch = Scratch::new();
    let variables = [
        VALID_ENVIRONMENT[0],
        ("CORS_ALLOWED_ORIGINS", "http://exa mple:4566"),
    ];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    let errors = stderr(&output);
    assert_eq!(
        errors,
        "CORS_ALLOWED_ORIGINS: must be * or a comma-separated list of origins such as \
         https://app.example.com\n"
    );
    assert!(!errors.contains("exa mple"));
}

#[rstest]
#[case("0")]
#[case("49ms")]
#[case("6s")]
fn a_fetch_wait_outside_its_range_is_rejected(#[case] value: &str) {
    let scratch = Scratch::new();
    let variables = [VALID_ENVIRONMENT[0], ("KAFKA_FETCH_MAX_WAIT", value)];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(
        stderr(&output),
        "KAFKA_FETCH_MAX_WAIT: must be a duration from 50ms to 5s\n"
    );
}

const EVENT_TYPES_PROBLEM: &str =
    "EVENT_TYPES: must be a comma-separated list of 1 to 1000 event type names";

/// `count` distinct valid names, joined by commas.
fn distinct_names(count: usize) -> String {
    (0..count)
        .map(|index| format!("t{index}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[rstest]
#[case::empty_value("")]
#[case::empty_entry("secret.a,,secret.b")]
#[case::invalid_name("secret.a,Secret.B")]
#[case::a_thousand_and_one(&distinct_names(1_001))]
fn check_config_rejects_invalid_event_types_without_their_value(#[case] value: &str) {
    let scratch = Scratch::new();
    let variables = [VALID_ENVIRONMENT[0], ("EVENT_TYPES", value)];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), format!("{EVENT_TYPES_PROBLEM}\n"));
}

#[rstest]
#[case::empty_entry("secret.a,,secret.b")]
#[case::invalid_name("secret.a,Secret.B")]
fn serve_rejects_invalid_event_types_without_their_value(#[case] value: &str) {
    let scratch = Scratch::new();
    let variables = [VALID_ENVIRONMENT[0], ("EVENT_TYPES", value)];

    let output = scratch.run(&["serve"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stderr(&output), "");
    let printed = stdout(&output);
    let lines: Vec<serde_json::Value> = printed
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "{printed}");
    assert_eq!(lines[0]["message"], "invalid configuration");
    assert_eq!(lines[0]["variable"], "EVENT_TYPES");
    assert_eq!(
        lines[0]["reason"],
        "must be a comma-separated list of 1 to 1000 event type names"
    );
    assert!(!printed.contains("secret."));
    assert!(!printed.contains("Secret.B"));
}

#[test]
fn check_config_accepts_a_thousand_distinct_event_types() {
    let scratch = Scratch::new();
    let names = distinct_names(1_000);
    let variables = [VALID_ENVIRONMENT[0], ("EVENT_TYPES", names.as_str())];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "configuration OK\n");
}

#[rstest]
#[case::data_bytes(
    "EVENT_MAX_DATA_BYTES",
    "1048577",
    "must be an integer from 1 to 1048576"
)]
#[case::window_bytes(
    "EVENT_WINDOW_MAX_BYTES",
    "2097151",
    "must be an integer from 2097152 to 17179869184"
)]
#[case::subscribers("MAX_SUBSCRIBERS", "0", "must be an integer from 1 to 1000000")]
#[case::request_timeout("KAFKA_REQUEST_TIMEOUT", "31s", "must be a duration from 1s to 30s")]
#[case::keep_alive("STREAM_KEEP_ALIVE", "999ms", "must be a duration from 1s to 5m")]
#[case::client_id("KAFKA_CLIENT_ID", "", "must be 1 to 128 characters")]
#[case::topic("KAFKA_TOPIC", "a/b", "must be a Kafka topic name")]
#[case::servers(
    "KAFKA_BOOTSTRAP_SERVERS",
    "::1:9092",
    "must be a comma-separated list of host:port addresses"
)]
#[case::protocol(
    "KAFKA_SECURITY_PROTOCOL",
    "tls",
    "must be one of plaintext, ssl, sasl_plaintext, sasl_ssl"
)]
#[case::origins(
    "CORS_ALLOWED_ORIGINS",
    "https://app.example.com/",
    "must be * or a comma-separated list of origins such as https://app.example.com"
)]
fn check_config_names_one_invalid_stream_variable_without_its_value(
    #[case] variable: &str,
    #[case] value: &str,
    #[case] reason: &str,
) {
    let scratch = Scratch::new();
    let variables = [VALID_ENVIRONMENT[0], (variable, value)];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), format!("{variable}: {reason}\n"));
}

#[test]
fn check_config_reports_the_rules_between_kafka_variables_last() {
    let scratch = Scratch::new();
    let variables = [
        VALID_ENVIRONMENT[0],
        ("KAFKA_CONNECT_TIMEOUT", "4s"),
        ("KAFKA_REQUEST_TIMEOUT", "3s"),
        ("KAFKA_SECURITY_PROTOCOL", "sasl_ssl"),
        ("MAX_SUBSCRIBERS", "0"),
    ];

    let output = scratch.run(&["check-config"], &variables);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr(&output),
        "MAX_SUBSCRIBERS: must be an integer from 1 to 1000000\n\
         KAFKA_CONNECT_TIMEOUT: must not be longer than KAFKA_REQUEST_TIMEOUT\n\
         KAFKA_SASL_MECHANISM: is required when KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl\n\
         KAFKA_SASL_USERNAME: is required when KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl\n\
         KAFKA_SASL_PASSWORD: is required when KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl\n"
    );
}

#[test]
fn the_sasl_password_is_never_printed_by_check_config_or_serve() {
    let scratch = Scratch::new();
    let variables = [
        VALID_ENVIRONMENT[0],
        ("KAFKA_SECURITY_PROTOCOL", "sasl_ssl"),
        ("KAFKA_SASL_MECHANISM", "PLAIN"),
        ("KAFKA_SASL_USERNAME", "reader"),
        ("KAFKA_SASL_PASSWORD", "s3cret-value"),
        ("KAFKA_TOPIC", ".."),
    ];

    let check = scratch.run(&["check-config"], &variables);
    let serve = scratch.run(&["serve"], &variables);

    assert_eq!(check.status.code(), Some(2));
    assert_eq!(stderr(&check), "KAFKA_TOPIC: must be a Kafka topic name\n");
    assert_eq!(serve.status.code(), Some(2));
    let printed = stdout(&serve);
    let lines: Vec<serde_json::Value> = printed
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 1, "{printed}");
    assert_eq!(lines[0]["variable"], "KAFKA_TOPIC");
    for output in [&check, &serve] {
        assert!(!stdout(output).contains("s3cret-value"));
        assert!(!stderr(output).contains("s3cret-value"));
    }
}

#[test]
fn the_help_lists_every_flag_and_never_a_value() {
    let scratch = Scratch::new();
    let variables = [("KAFKA_SASL_PASSWORD", "topsecret"), ("APP_PORT", "4242")];

    let serve = scratch.run(&["serve", "--help"], &variables);
    let check = scratch.run(&["check-config", "--help"], &variables);
    let root = scratch.run(&["--help"], &variables);

    for output in [&serve, &check, &root] {
        assert_eq!(output.status.code(), Some(0));
        let text = stdout(output);
        assert!(!text.contains("topsecret"));
        assert!(!text.contains("4242"));
        assert!(!text.contains("DATABASE_"));
        assert!(!text.contains("migrate"));
    }
    let help = stdout(&serve);
    for flag in [
        "--app-env",
        "--app-host",
        "--app-port",
        "--log-level",
        "--health-probe-timeout",
        "--http-request-timeout",
        "--http-max-body-bytes",
        "--shutdown-grace-period",
        "--event-types",
        "--kafka-bootstrap-servers",
        "--kafka-sasl-password",
        "--stream-keep-alive",
        "--cors-allowed-origins",
    ] {
        assert!(help.contains(flag), "{flag} is missing from the help");
    }
    // The help wraps at the terminal width when clap has `wrap_help` (a workspace test build
    // unifies it in), so the label is matched with its whitespace collapsed.
    let words = help.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(words.contains("[env: KAFKA_BOOTSTRAP_SERVERS]"));
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
        "PORT_BASE=9000\nAPP_PORT=${PORT_BASE}\nKAFKA_BOOTSTRAP_SERVERS=localhost:9092\n",
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
        &[VALID_ENVIRONMENT[0], ("APP_PORT", "not-a-port")],
    );
    scratch.write(".env", "APP_PORT=not-a-port\n");
    let valid_in_the_process = scratch.run(
        &["check-config"],
        &[VALID_ENVIRONMENT[0], ("APP_PORT", "9001")],
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
    assert_eq!(stderr(&output), "KAFKA_BOOTSTRAP_SERVERS: is required\n");
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
