use clap::error::ErrorKind;
use clap::{Arg, CommandFactory as _};
use rstest::rstest;

use super::*;

fn parse(arguments: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("service").chain(arguments.iter().copied()))
}

fn serve_arguments() -> Vec<Arg> {
    ServeArgs::augment_args(clap::Command::new("serve"))
        .get_arguments()
        .cloned()
        .collect()
}

fn subcommand_help(name: &str) -> String {
    let mut command = Cli::command();
    command
        .find_subcommand_mut(name)
        .unwrap()
        .render_long_help()
        .to_string()
}

#[test]
fn serve_takes_every_setting_as_a_flag() {
    let cli = parse(&[
        "serve",
        "--app-env",
        "dev",
        "--app-host",
        "127.0.0.1",
        "--app-port",
        "9000",
        "--log-level",
        "debug",
        "--aws-region",
        "eu-west-1",
        "--aws-endpoint-url-sqs",
        "http://localhost:4566",
        "--sqs-connect-timeout",
        "300ms",
        "--sqs-operation-timeout",
        "3s",
        "--sqs-max-attempts",
        "2",
        "--health-probe-timeout",
        "4s",
        "--http-request-timeout",
        "5s",
        "--http-max-body-bytes",
        "2048",
        "--shutdown-grace-period",
        "6s",
        "--event-types",
        "user.created,order.paid",
        "--kafka-bootstrap-servers",
        "a:9092",
        "--kafka-topic",
        "events",
        "--kafka-security-protocol",
        "sasl_ssl",
        "--kafka-sasl-mechanism",
        "PLAIN",
        "--kafka-sasl-username",
        "reader",
        "--kafka-sasl-password",
        "secret",
        "--kafka-client-id",
        "node",
        "--kafka-request-timeout",
        "4s",
        "--kafka-connect-timeout",
        "1s",
        "--kafka-fetch-max-wait",
        "100ms",
        "--kafka-retry-delay",
        "2s",
        "--event-max-data-bytes",
        "1024",
        "--event-window-max-events",
        "10",
        "--event-window-max-bytes",
        "3000000",
        "--event-window-start-events",
        "5",
        "--event-replay-max-events",
        "20",
        "--event-replay-permits",
        "3",
        "--event-tail-stale-after",
        "20s",
        "--subscriber-buffer",
        "8",
        "--max-subscribers",
        "9",
        "--stream-keep-alive",
        "30s",
        "--cors-allowed-origins",
        "*",
    ])
    .unwrap();

    let Command::Serve(args) = cli.command else {
        panic!("expected serve");
    };
    assert_eq!(args.app_env.as_deref(), Some("dev"));
    assert_eq!(args.app_host.as_deref(), Some("127.0.0.1"));
    assert_eq!(args.app_port.as_deref(), Some("9000"));
    assert_eq!(args.log_level.as_deref(), Some("debug"));
    assert_eq!(args.aws_region.as_deref(), Some("eu-west-1"));
    assert_eq!(
        args.aws_endpoint_url_sqs.as_deref(),
        Some("http://localhost:4566")
    );
    assert_eq!(args.sqs_connect_timeout.as_deref(), Some("300ms"));
    assert_eq!(args.sqs_operation_timeout.as_deref(), Some("3s"));
    assert_eq!(args.sqs_max_attempts.as_deref(), Some("2"));
    assert_eq!(args.health_probe_timeout.as_deref(), Some("4s"));
    assert_eq!(args.http_request_timeout.as_deref(), Some("5s"));
    assert_eq!(args.http_max_body_bytes.as_deref(), Some("2048"));
    assert_eq!(args.shutdown_grace_period.as_deref(), Some("6s"));
    assert_eq!(args.event_types.as_deref(), Some("user.created,order.paid"));
    let kafka = &args.kafka;
    assert_eq!(kafka.bootstrap_servers.as_deref(), Some("a:9092"));
    assert_eq!(kafka.topic.as_deref(), Some("events"));
    assert_eq!(kafka.security_protocol.as_deref(), Some("sasl_ssl"));
    assert_eq!(kafka.sasl_mechanism.as_deref(), Some("PLAIN"));
    assert_eq!(kafka.sasl_username.as_deref(), Some("reader"));
    assert_eq!(kafka.sasl_password.as_deref(), Some("secret"));
    assert_eq!(kafka.client_id.as_deref(), Some("node"));
    assert_eq!(kafka.request_timeout.as_deref(), Some("4s"));
    assert_eq!(kafka.connect_timeout.as_deref(), Some("1s"));
    assert_eq!(kafka.fetch_max_wait.as_deref(), Some("100ms"));
    assert_eq!(kafka.retry_delay.as_deref(), Some("2s"));
    let streams = &args.streams;
    assert_eq!(streams.max_data_bytes.as_deref(), Some("1024"));
    assert_eq!(streams.window_max_events.as_deref(), Some("10"));
    assert_eq!(streams.window_max_bytes.as_deref(), Some("3000000"));
    assert_eq!(streams.window_start_events.as_deref(), Some("5"));
    assert_eq!(streams.replay_max_events.as_deref(), Some("20"));
    assert_eq!(streams.replay_permits.as_deref(), Some("3"));
    assert_eq!(streams.tail_stale_after.as_deref(), Some("20s"));
    assert_eq!(streams.subscriber_buffer.as_deref(), Some("8"));
    assert_eq!(streams.max_subscribers.as_deref(), Some("9"));
    assert_eq!(streams.keep_alive.as_deref(), Some("30s"));
    assert_eq!(args.cors_allowed_origins.as_deref(), Some("*"));
}

#[test]
fn the_credentials_are_accepted_as_hidden_flags() {
    let cli = parse(&[
        "serve",
        "--aws-access-key-id",
        "key",
        "--aws-secret-access-key",
        "secret",
    ])
    .unwrap();

    let Command::Serve(args) = cli.command else {
        panic!("expected serve");
    };
    assert_eq!(args.aws_access_key_id.as_deref(), Some("key"));
    assert_eq!(args.aws_secret_access_key.as_deref(), Some("secret"));
}

#[test]
fn check_config_takes_the_arguments_of_serve() {
    let cli = parse(&["check-config", "--app-port", "9000"]).unwrap();

    let Command::CheckConfig(args) = cli.command else {
        panic!("expected check-config");
    };
    assert_eq!(args.app_port.as_deref(), Some("9000"));
}

#[rstest]
#[case("openapi-export")]
#[case("graphql-export")]
fn the_exports_take_an_optional_output_path(#[case] name: &str) {
    let to_stdout = parse(&[name]).unwrap();
    let to_file = parse(&[name, "--output", "out.txt"]).unwrap();

    let (Command::OpenapiExport(stdout) | Command::GraphqlExport(stdout)) = to_stdout.command
    else {
        panic!("expected an export");
    };
    let (Command::OpenapiExport(file) | Command::GraphqlExport(file)) = to_file.command else {
        panic!("expected an export");
    };
    assert_eq!(stdout.output, None);
    assert_eq!(file.output, Some(PathBuf::from("out.txt")));
}

#[test]
fn the_export_commands_are_told_apart() {
    assert!(matches!(
        parse(&["openapi-export"]).unwrap().command,
        Command::OpenapiExport(_)
    ));
    assert!(matches!(
        parse(&["graphql-export"]).unwrap().command,
        Command::GraphqlExport(_)
    ));
}

#[test]
fn healthcheck_takes_a_port_and_a_host() {
    let cli = parse(&["healthcheck", "--port", "9100", "--host", "10.0.3.7"]).unwrap();

    let Command::Healthcheck(args) = cli.command else {
        panic!("expected healthcheck");
    };
    assert_eq!(args.port.as_deref(), Some("9100"));
    assert_eq!(args.host.as_deref(), Some("10.0.3.7"));
}

#[rstest]
#[case(&["frobnicate"], ErrorKind::InvalidSubcommand)]
#[case(&["migrate"], ErrorKind::InvalidSubcommand)]
#[case(&["serve", "--frobnicate"], ErrorKind::UnknownArgument)]
#[case(&["--frobnicate"], ErrorKind::UnknownArgument)]
#[case(&[], ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand)]
fn a_usage_error_has_exit_status_2(#[case] arguments: &[&str], #[case] kind: ErrorKind) {
    let error = parse(arguments).unwrap_err();

    assert_eq!(error.kind(), kind);
    assert_eq!(error.exit_code(), 2);
}

#[rstest]
#[case("--help")]
#[case("--version")]
fn help_and_version_are_not_failures(#[case] flag: &str) {
    let error = parse(&[flag]).unwrap_err();

    assert_eq!(error.exit_code(), 0);
    assert!(!error.use_stderr());
}

#[test]
fn the_version_is_the_package_version() {
    let error = parse(&["--version"]).unwrap_err();

    assert_eq!(
        error.to_string().trim(),
        format!("service {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn every_setting_is_bound_to_a_variable_whose_value_is_never_printed() {
    let arguments = serve_arguments();

    assert!(
        arguments
            .iter()
            .all(|argument| argument.get_env().is_some())
    );
    assert!(arguments.iter().all(Arg::is_hide_env_values_set));
}

#[test]
fn serve_declares_exactly_the_variables_of_the_configuration_table() {
    let mut variables: Vec<String> = serve_arguments()
        .iter()
        .filter_map(|argument| argument.get_env())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    variables.sort();

    let mut expected = vec![
        "APP_ENV",
        "APP_HOST",
        "APP_PORT",
        "AWS_ACCESS_KEY_ID",
        "AWS_ENDPOINT_URL_SQS",
        "AWS_REGION",
        "AWS_SECRET_ACCESS_KEY",
        "CORS_ALLOWED_ORIGINS",
        "EVENT_MAX_DATA_BYTES",
        "EVENT_REPLAY_MAX_EVENTS",
        "EVENT_REPLAY_PERMITS",
        "EVENT_TAIL_STALE_AFTER",
        "EVENT_TYPES",
        "EVENT_WINDOW_MAX_BYTES",
        "EVENT_WINDOW_MAX_EVENTS",
        "EVENT_WINDOW_START_EVENTS",
        "HEALTH_PROBE_TIMEOUT",
        "HTTP_MAX_BODY_BYTES",
        "HTTP_REQUEST_TIMEOUT",
        "KAFKA_BOOTSTRAP_SERVERS",
        "KAFKA_CLIENT_ID",
        "KAFKA_CONNECT_TIMEOUT",
        "KAFKA_FETCH_MAX_WAIT",
        "KAFKA_REQUEST_TIMEOUT",
        "KAFKA_RETRY_DELAY",
        "KAFKA_SASL_MECHANISM",
        "KAFKA_SASL_PASSWORD",
        "KAFKA_SASL_USERNAME",
        "KAFKA_SECURITY_PROTOCOL",
        "KAFKA_TOPIC",
        "LOG_LEVEL",
        "MAX_SUBSCRIBERS",
        "SHUTDOWN_GRACE_PERIOD",
        "SQS_CONNECT_TIMEOUT",
        "SQS_MAX_ATTEMPTS",
        "SQS_OPERATION_TIMEOUT",
        "STREAM_KEEP_ALIVE",
        "SUBSCRIBER_BUFFER",
    ];
    expected.sort_unstable();
    assert_eq!(variables, expected);
}

#[test]
fn only_the_two_credentials_are_hidden_from_the_help() {
    let mut hidden: Vec<String> = serve_arguments()
        .iter()
        .filter(|argument| argument.is_hide_set())
        .map(|argument| argument.get_id().to_string())
        .collect();
    hidden.sort();

    assert_eq!(hidden, ["aws_access_key_id", "aws_secret_access_key"]);
}

#[test]
fn the_serve_help_lists_every_variable_but_the_credentials() {
    let help = subcommand_help("serve");

    for argument in serve_arguments().iter().filter(|a| !a.is_hide_set()) {
        let flag = format!("--{}", argument.get_long().unwrap());
        assert!(help.contains(&flag), "{flag} is missing");
        let variable = argument.get_env().unwrap().to_string_lossy();
        assert!(help.contains(&format!("[env: {variable}]")));
    }
    assert!(!help.contains("AWS_ACCESS_KEY_ID"));
    assert!(!help.contains("AWS_SECRET_ACCESS_KEY"));
}

#[test]
fn the_healthcheck_settings_are_bound_to_their_variables() {
    let healthcheck = Cli::command();
    let healthcheck = healthcheck.find_subcommand("healthcheck").unwrap();

    let port = healthcheck
        .get_arguments()
        .find(|argument| argument.get_id() == "port")
        .unwrap();
    let host = healthcheck
        .get_arguments()
        .find(|argument| argument.get_id() == "host")
        .unwrap();
    assert_eq!(port.get_env().unwrap(), "APP_PORT");
    assert!(port.is_hide_env_values_set());
    assert_eq!(host.get_env().unwrap(), "APP_HOST");
    assert!(host.is_hide_env_values_set());
}
