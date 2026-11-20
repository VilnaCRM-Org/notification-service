use rstest::rstest;

use super::*;

fn valid() -> ServeArgs {
    ServeArgs {
        aws_region: Some("us-east-1".to_owned()),
        ..ServeArgs::default()
    }
}

fn some(value: &str) -> Option<String> {
    Option::from(value.to_owned())
}

fn problems(args: ServeArgs) -> Vec<(&'static str, String)> {
    Config::from_args(args)
        .unwrap_err()
        .problems()
        .iter()
        .map(|problem| (problem.variable, problem.reason.clone()))
        .collect()
}

fn variables(args: ServeArgs) -> Vec<&'static str> {
    problems(args)
        .into_iter()
        .map(|(variable, _reason)| variable)
        .collect()
}

#[test]
fn the_defaults_are_the_documented_ones() {
    let config = Config::from_args(valid()).unwrap();

    assert_eq!(config.app_env, AppEnv::Prod);
    assert_eq!(config.host, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    assert_eq!(config.port, 8081);
    assert_eq!(config.log_level, "info");
    assert_eq!(config.sqs.region, "us-east-1");
    assert_eq!(config.sqs.endpoint_url, None);
    assert_eq!(config.sqs.connect_timeout, Duration::from_millis(500));
    assert_eq!(config.sqs.operation_timeout, Duration::from_secs(2));
    assert_eq!(config.sqs.max_attempts, 1);
    assert_eq!(config.health_probe_timeout, Duration::from_secs(3));
    assert_eq!(config.request_timeout, Duration::from_secs(10));
    assert_eq!(config.max_body_bytes, 65_536);
    assert_eq!(config.shutdown_grace_period, Duration::from_secs(10));
    assert_eq!(
        config.kafka,
        KafkaSettings {
            bootstrap_servers: "localhost:9092".to_owned(),
            topic: "notifications".to_owned(),
            client_id: "notification-service".to_owned(),
            security: KafkaSecurity::Plaintext,
            request_timeout: Duration::from_secs(3),
            connect_timeout: Duration::from_secs(2),
            fetch_wait: Duration::from_millis(500),
        }
    );
    assert_eq!(
        config.streams,
        StreamConfig {
            catalog: EventCatalog::default(),
            max_data_bytes: 65_536,
            window_max_events: 100_000,
            window_max_bytes: 67_108_864,
            start_events: 100_000,
            replay_max_events: 1_000_000,
            replay_permits: 4,
            retry_delay: Duration::from_secs(1),
            stale_after: Duration::from_secs(10),
            buffer: 256,
            max_subscribers: 100_000,
            keep_alive: Duration::from_secs(15),
        }
    );
    assert_eq!(config.allowed_origins, AllowedOrigins::NoOrigin);
}

#[test]
fn every_variable_accepts_a_valid_value() {
    let args = ServeArgs {
        app_env: some("dev"),
        app_host: some("127.0.0.1"),
        app_port: some("9000"),
        log_level: some("service=debug,warn"),
        aws_region: some("eu-west-1"),
        aws_endpoint_url_sqs: some("http://localhost:4566"),
        aws_access_key_id: some("key"),
        aws_secret_access_key: some("secret"),
        sqs_connect_timeout: some("300ms"),
        sqs_operation_timeout: some("1s"),
        sqs_max_attempts: some("3"),
        health_probe_timeout: some("4s"),
        http_request_timeout: some("1m"),
        http_max_body_bytes: some("2048"),
        shutdown_grace_period: some("15s"),
        event_types: some("user.created,order.paid"),
        kafka: KafkaArgs {
            bootstrap_servers: some("a:9092,b:9093"),
            topic: some("events.v1"),
            security_protocol: some("sasl_ssl"),
            sasl_mechanism: some("SCRAM-SHA-256"),
            sasl_username: some("reader"),
            sasl_password: some("s3cret-value"),
            client_id: some("node-a"),
            request_timeout: some("5s"),
            connect_timeout: some("5s"),
            fetch_max_wait: some("1s"),
            retry_delay: some("2s"),
        },
        streams: StreamArgs {
            max_data_bytes: some("1024"),
            window_max_events: some("10"),
            window_max_bytes: some("3000000"),
            window_start_events: some("0"),
            replay_max_events: some("20"),
            replay_permits: some("8"),
            tail_stale_after: some("30s"),
            subscriber_buffer: some("16"),
            max_subscribers: some("50"),
            keep_alive: some("20s"),
        },
        cors_allowed_origins: some("https://app.example.com"),
    };

    let config = Config::from_args(args).unwrap();

    assert_eq!(config.app_env, AppEnv::Dev);
    assert_eq!(config.host, IpAddr::from([127, 0, 0, 1]));
    assert_eq!(config.port, 9000);
    assert_eq!(config.log_level, "service=debug,warn");
    assert_eq!(config.sqs.region, "eu-west-1");
    assert_eq!(
        config.sqs.endpoint_url.as_deref(),
        Some("http://localhost:4566")
    );
    assert_eq!(config.sqs.connect_timeout, Duration::from_millis(300));
    assert_eq!(config.sqs.operation_timeout, Duration::from_secs(1));
    assert_eq!(config.sqs.max_attempts, 3);
    assert_eq!(config.health_probe_timeout, Duration::from_secs(4));
    assert_eq!(config.request_timeout, Duration::from_secs(60));
    assert_eq!(config.max_body_bytes, 2048);
    assert_eq!(config.shutdown_grace_period, Duration::from_secs(15));
    assert_eq!(
        config.streams.catalog,
        catalog(&["order.paid", "user.created"])
    );
    assert_eq!(
        config.kafka,
        KafkaSettings {
            bootstrap_servers: "a:9092,b:9093".to_owned(),
            topic: "events.v1".to_owned(),
            client_id: "node-a".to_owned(),
            security: KafkaSecurity::SaslSsl(SaslCredentials {
                mechanism: SaslMechanism::ScramSha256,
                username: "reader".to_owned(),
                password: "s3cret-value".to_owned(),
            }),
            request_timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(5),
            fetch_wait: Duration::from_secs(1),
        }
    );
    assert_eq!(config.streams.max_data_bytes, 1024);
    assert_eq!(config.streams.window_max_events, 10);
    assert_eq!(config.streams.window_max_bytes, 3_000_000);
    assert_eq!(config.streams.start_events, 0);
    assert_eq!(config.streams.replay_max_events, 20);
    assert_eq!(config.streams.replay_permits, 8);
    assert_eq!(config.streams.retry_delay, Duration::from_secs(2));
    assert_eq!(config.streams.stale_after, Duration::from_secs(30));
    assert_eq!(config.streams.buffer, 16);
    assert_eq!(config.streams.max_subscribers, 50);
    assert_eq!(config.streams.keep_alive, Duration::from_secs(20));
    assert_eq!(
        config.allowed_origins,
        AllowedOrigins::List(vec![HeaderValue::from_static("https://app.example.com")])
    );
}

#[rstest]
#[case("dev", AppEnv::Dev)]
#[case("test", AppEnv::Test)]
#[case("prod", AppEnv::Prod)]
fn app_env_accepts_its_three_values(#[case] raw: &str, #[case] expected: AppEnv) {
    let args = ServeArgs {
        app_env: some(raw),
        ..valid()
    };

    assert_eq!(Config::from_args(args).unwrap().app_env, expected);
}

#[rstest]
#[case("staging")]
#[case("")]
#[case("Dev")]
fn app_env_rejects_anything_else(#[case] raw: &str) {
    let args = ServeArgs {
        app_env: some(raw),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [("APP_ENV", "must be one of dev, test, prod".to_owned())]
    );
}

#[rstest]
#[case("::1")]
#[case("10.1.2.3")]
fn app_host_accepts_ip_addresses(#[case] raw: &str) {
    let args = ServeArgs {
        app_host: some(raw),
        ..valid()
    };

    assert_eq!(
        Config::from_args(args).unwrap().host,
        raw.parse::<IpAddr>().unwrap()
    );
}

#[rstest]
#[case("localhost")]
#[case("")]
#[case("256.1.1.1")]
fn app_host_rejects_what_is_not_an_ip_address(#[case] raw: &str) {
    let args = ServeArgs {
        app_host: some(raw),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [("APP_HOST", "must be an IP address".to_owned())]
    );
}

#[rstest]
#[case("0", 0)]
#[case("65535", 65535)]
fn app_port_accepts_the_whole_range(#[case] raw: &str, #[case] expected: u16) {
    let args = ServeArgs {
        app_port: some(raw),
        ..valid()
    };

    assert_eq!(Config::from_args(args).unwrap().port, expected);
}

#[rstest]
#[case("65536")]
#[case("99999")]
#[case("-1")]
#[case("eighty")]
#[case("")]
fn app_port_rejects_the_rest(#[case] raw: &str) {
    let args = ServeArgs {
        app_port: some(raw),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [("APP_PORT", "must be an integer from 0 to 65535".to_owned())]
    );
}

#[rstest]
#[case("debug")]
#[case("warn,service=trace")]
fn log_level_accepts_filter_directives(#[case] raw: &str) {
    let args = ServeArgs {
        log_level: some(raw),
        ..valid()
    };

    assert_eq!(Config::from_args(args).unwrap().log_level, raw);
}

#[rstest]
#[case("service=loud")]
#[case("info,=debug")]
fn log_level_rejects_invalid_directives(#[case] raw: &str) {
    let args = ServeArgs {
        log_level: some(raw),
        ..valid()
    };

    assert_eq!(variables(args), ["LOG_LEVEL"]);
}

#[test]
fn a_blank_required_variable_is_reported_as_empty() {
    let args = ServeArgs {
        aws_region: some("  "),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [("AWS_REGION", "must not be empty".to_owned())]
    );
}

#[test]
fn an_unparsable_duration_names_its_variable() {
    let args = ServeArgs {
        sqs_connect_timeout: some(""),
        sqs_operation_timeout: some("-2s"),
        health_probe_timeout: some("fast"),
        http_request_timeout: some("1 fortnight"),
        shutdown_grace_period: some("later"),
        ..valid()
    };

    let found = problems(args);

    assert_eq!(
        found
            .iter()
            .map(|(variable, _)| *variable)
            .collect::<Vec<_>>(),
        [
            "SQS_CONNECT_TIMEOUT",
            "SQS_OPERATION_TIMEOUT",
            "HEALTH_PROBE_TIMEOUT",
            "HTTP_REQUEST_TIMEOUT",
            "SHUTDOWN_GRACE_PERIOD",
        ]
    );
    assert!(
        found
            .iter()
            .all(|(_, reason)| reason == "must be a duration such as 500ms, 2s or 1m")
    );
}

/// `SHUTDOWN_GRACE_PERIOD` with `raw`: unranged and under no rule between variables, so every
/// value that reads as a duration reaches the configuration.
fn shutdown_grace_period(raw: &str) -> ServeArgs {
    ServeArgs {
        shutdown_grace_period: some(raw),
        ..valid()
    }
}

#[rstest]
#[case::millis_50("50ms", Duration::from_millis(50))]
#[case::millis_100("100ms", Duration::from_millis(100))]
#[case::millis_300("300ms", Duration::from_millis(300))]
#[case::millis_500("500ms", Duration::from_millis(500))]
#[case::seconds_1("1s", Duration::from_secs(1))]
#[case::seconds_2("2s", Duration::from_secs(2))]
#[case::seconds_3("3s", Duration::from_secs(3))]
#[case::seconds_4("4s", Duration::from_secs(4))]
#[case::seconds_5("5s", Duration::from_secs(5))]
#[case::seconds_10("10s", Duration::from_secs(10))]
#[case::seconds_15("15s", Duration::from_secs(15))]
#[case::seconds_20("20s", Duration::from_secs(20))]
#[case::seconds_30("30s", Duration::from_secs(30))]
#[case::millis_3001("3001ms", Duration::from_millis(3001))]
#[case::one_minute_never_one_month("1m", Duration::from_secs(60))]
#[case::minutes_2("2m", Duration::from_secs(120))]
#[case::minutes_5("5m", Duration::from_secs(300))]
#[case::minutes_10("10m", Duration::from_secs(600))]
#[case::bare_zero("0", Duration::ZERO)]
#[case::plus_sign("+2s", Duration::from_secs(2))]
#[case::comma_fraction("1,5s", Duration::from_millis(1500))]
#[case::clock_form("01:30:00", Duration::from_mins(90))]
#[case::dot_fraction("1.5s", Duration::from_millis(1500))]
#[case::minutes_then_seconds_after_a_space("1m 30s", Duration::from_secs(90))]
#[case::minutes_then_seconds("1m30s", Duration::from_secs(90))]
#[case::hours("1h", Duration::from_hours(1))]
#[case::microseconds("1500us", Duration::from_micros(1500))]
#[case::nanoseconds("750ns", Duration::from_nanos(750))]
#[case::long_name_of_minutes("2min", Duration::from_secs(120))]
#[case::long_name_of_seconds("30seconds", Duration::from_secs(30))]
fn a_duration_reads_as_its_documented_length(#[case] raw: &str, #[case] expected: Duration) {
    let config = Config::from_args(shutdown_grace_period(raw)).unwrap();

    assert_eq!(config.shutdown_grace_period, expected);
}

#[rstest]
#[case::days("1d")]
#[case::weeks("2w")]
#[case::months("1M")]
#[case::years("1y")]
#[case::smaller_unit_first("30s1m")]
#[case::repeated_unit("1h1h")]
#[case::leading_space(" 2s")]
#[case::space_inside_a_number("1 0s")]
#[case::no_break_space_before_the_unit("1\u{a0}s")]
#[case::empty("")]
#[case::negative("-2s")]
#[case::word("fast")]
#[case::unit_of_two_weeks("1 fortnight")]
#[case::number_without_a_unit("5")]
fn a_text_outside_the_duration_syntax_is_refused(#[case] raw: &str) {
    assert_eq!(
        problems(shutdown_grace_period(raw)),
        [(
            "SHUTDOWN_GRACE_PERIOD",
            "must be a duration such as 500ms, 2s or 1m".to_owned()
        )]
    );
}

#[rstest]
#[case::bare_zero("0")]
#[case::zero_seconds("0s")]
fn a_ranged_duration_refuses_zero_through_its_range(#[case] raw: &str) {
    let mut args = valid();
    args.kafka.retry_delay = some(raw);

    assert_eq!(
        problems(args),
        [(
            "KAFKA_RETRY_DELAY",
            "must be a duration from 100ms to 1m".to_owned()
        )]
    );
}

#[rstest]
#[case("http://localhost:4566")]
#[case("https://sqs.eu-west-1.amazonaws.com")]
#[case("http://localstack:4566/")]
#[case("http://[::1]:4566")]
#[case("http://[::1]")]
#[case("http://user:secret@localstack:4566")]
#[case("http://localhost:0")]
#[case("http://localhost:65535")]
fn the_sqs_endpoint_accepts_http_and_https_urls(#[case] raw: &str) {
    let args = ServeArgs {
        aws_endpoint_url_sqs: some(raw),
        ..valid()
    };

    assert_eq!(
        Config::from_args(args).unwrap().sqs.endpoint_url.as_deref(),
        Some(raw)
    );
}

#[rstest]
#[case("ftp://localhost")]
#[case("localhost:4566")]
#[case("http://")]
#[case("https://")]
#[case("")]
#[case("http:// ")]
#[case("http://exa mple:4566")]
#[case("http://:4566")]
#[case("http://localhost:abc")]
#[case("http://localhost:99999")]
#[case("http://127.0.0.1:4566x")]
#[case("http://abc:abc")]
#[case("http://localhost:")]
#[case("http://user@localhost:4566x")]
#[case("http://[::1]x:80")]
#[case("http://[::1]x")]
#[case("http://localhost:+80")]
#[case("http://localhost:-1")]
fn the_sqs_endpoint_rejects_everything_else(#[case] raw: &str) {
    let args = ServeArgs {
        aws_endpoint_url_sqs: some(raw),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [(
            "AWS_ENDPOINT_URL_SQS",
            "must be an http:// or https:// URL".to_owned()
        )]
    );
}

#[test]
fn aws_region_is_required() {
    let args = ServeArgs {
        aws_region: None,
        ..valid()
    };

    assert_eq!(problems(args), [("AWS_REGION", "is required".to_owned())]);
}

#[rstest]
#[case("1", 1)]
#[case("5", 5)]
fn sqs_max_attempts_accepts_its_bounds(#[case] raw: &str, #[case] expected: u32) {
    let args = ServeArgs {
        sqs_max_attempts: some(raw),
        ..valid()
    };

    assert_eq!(Config::from_args(args).unwrap().sqs.max_attempts, expected);
}

#[rstest]
#[case("0")]
#[case("6")]
#[case("x")]
fn sqs_max_attempts_rejects_values_outside_the_range(#[case] raw: &str) {
    let args = ServeArgs {
        sqs_max_attempts: some(raw),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [(
            "SQS_MAX_ATTEMPTS",
            "must be an integer from 1 to 5".to_owned()
        )]
    );
}

#[rstest]
#[case("1024", 1024)]
#[case("10485760", 10_485_760)]
fn http_max_body_bytes_accepts_its_bounds(#[case] raw: &str, #[case] expected: usize) {
    let args = ServeArgs {
        http_max_body_bytes: some(raw),
        ..valid()
    };

    assert_eq!(Config::from_args(args).unwrap().max_body_bytes, expected);
}

#[rstest]
#[case("1023")]
#[case("10485761")]
#[case("big")]
fn http_max_body_bytes_rejects_values_outside_the_range(#[case] raw: &str) {
    let args = ServeArgs {
        http_max_body_bytes: some(raw),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [(
            "HTTP_MAX_BODY_BYTES",
            "must be an integer from 1024 to 10485760".to_owned()
        )]
    );
}

#[test]
fn credentials_set_together_or_not_at_all_are_valid() {
    let both = ServeArgs {
        aws_access_key_id: some("key"),
        aws_secret_access_key: some("secret"),
        ..valid()
    };

    assert!(Config::from_args(both).is_ok());
    assert!(Config::from_args(valid()).is_ok());
}

#[test]
fn a_key_id_without_a_secret_names_the_missing_secret() {
    let args = ServeArgs {
        aws_access_key_id: some("key"),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [(
            "AWS_SECRET_ACCESS_KEY",
            "is required when AWS_ACCESS_KEY_ID is set".to_owned()
        )]
    );
}

#[test]
fn a_secret_without_a_key_id_names_the_missing_key_id() {
    let args = ServeArgs {
        aws_secret_access_key: some("secret"),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [(
            "AWS_ACCESS_KEY_ID",
            "is required when AWS_SECRET_ACCESS_KEY is set".to_owned()
        )]
    );
}

fn timeouts(operation: &str, probe: &str, request: &str) -> ServeArgs {
    ServeArgs {
        sqs_operation_timeout: some(operation),
        health_probe_timeout: some(probe),
        http_request_timeout: some(request),
        ..valid()
    }
}

#[test]
fn the_probe_timeout_may_equal_the_operation_timeout() {
    assert!(Config::from_args(timeouts("3s", "3s", "4s")).is_ok());
}

#[test]
fn the_probe_timeout_below_the_operation_timeout_is_reported() {
    assert_eq!(
        problems(timeouts("4s", "3s", "10s")),
        [(
            "HEALTH_PROBE_TIMEOUT",
            "must not be shorter than SQS_OPERATION_TIMEOUT".to_owned()
        )]
    );
}

#[test]
fn the_request_timeout_must_exceed_the_probe_timeout() {
    assert_eq!(
        problems(timeouts("2s", "3s", "3s")),
        [(
            "HTTP_REQUEST_TIMEOUT",
            "must be longer than HEALTH_PROBE_TIMEOUT".to_owned()
        )]
    );
    assert!(Config::from_args(timeouts("2s", "3s", "3001ms")).is_ok());
}

#[test]
fn a_timeout_above_its_prd_ceiling_is_accepted() {
    let args = ServeArgs {
        sqs_operation_timeout: some("1m"),
        health_probe_timeout: some("2m"),
        http_request_timeout: some("5m"),
        shutdown_grace_period: some("10m"),
        http_max_body_bytes: some("10485760"),
        ..valid()
    };

    let config = Config::from_args(args).unwrap();

    assert_eq!(config.sqs.operation_timeout, Duration::from_secs(60));
    assert_eq!(config.health_probe_timeout, Duration::from_secs(120));
    assert_eq!(config.shutdown_grace_period, Duration::from_secs(600));
}

#[test]
fn a_rule_between_two_variables_is_skipped_when_one_of_them_is_invalid() {
    let args = ServeArgs {
        health_probe_timeout: some("soon"),
        http_request_timeout: some("1s"),
        ..valid()
    };

    assert_eq!(variables(args), ["HEALTH_PROBE_TIMEOUT"]);
}

#[test]
fn every_problem_is_reported_in_one_run_in_the_order_of_the_table() {
    let args = ServeArgs {
        app_env: some("staging"),
        app_port: some("99999"),
        aws_region: None,
        aws_endpoint_url_sqs: some("ftp://user:hunter2@h/queue"),
        aws_access_key_id: some("key"),
        ..ServeArgs::default()
    };

    let errors = Config::from_args(args).unwrap_err();

    assert_eq!(
        errors
            .problems()
            .iter()
            .map(|problem| problem.variable)
            .collect::<Vec<_>>(),
        [
            "APP_ENV",
            "APP_PORT",
            "AWS_REGION",
            "AWS_ENDPOINT_URL_SQS",
            "AWS_SECRET_ACCESS_KEY"
        ]
    );
    assert_eq!(
        errors.to_string(),
        "the configuration is invalid (5 problem(s))"
    );
    assert!(
        errors
            .problems()
            .iter()
            .all(|problem| !problem.reason.contains("hunter2"))
    );
}

#[test]
fn event_types_is_reported_after_the_limits() {
    let args = ServeArgs {
        shutdown_grace_period: some("later"),
        event_types: some(""),
        ..valid()
    };

    assert_eq!(
        problems(args),
        [
            (
                "SHUTDOWN_GRACE_PERIOD",
                "must be a duration such as 500ms, 2s or 1m".to_owned()
            ),
            ("EVENT_TYPES", EVENT_TYPES_REASON.to_owned()),
        ]
    );
}

#[test]
fn event_types_is_reported_before_the_rules_between_variables() {
    let args = ServeArgs {
        event_types: some(""),
        ..timeouts("4s", "3s", "10s")
    };

    assert_eq!(
        problems(args),
        [
            ("EVENT_TYPES", EVENT_TYPES_REASON.to_owned()),
            (
                "HEALTH_PROBE_TIMEOUT",
                "must not be shorter than SQS_OPERATION_TIMEOUT".to_owned()
            ),
        ]
    );
}

fn catalog(names: &[&str]) -> EventCatalog {
    EventCatalog::new(
        names
            .iter()
            .map(|name| EventType::new((*name).to_owned()).unwrap())
            .collect(),
    )
}

fn with_event_types(raw: &str) -> ServeArgs {
    ServeArgs {
        event_types: some(raw),
        ..valid()
    }
}

/// `count` distinct valid names, joined by commas.
fn distinct_names(count: usize) -> String {
    (0..count)
        .map(|index| format!("t{index}"))
        .collect::<Vec<_>>()
        .join(",")
}

const EVENT_TYPES_REASON: &str = "must be a comma-separated list of 1 to 1000 event type names";

#[test]
fn event_types_keeps_each_name_once_in_byte_order() {
    let config = Config::from_args(with_event_types(
        "user.created,order.paid,order-paid,order_paid,order.paid",
    ))
    .unwrap();

    assert_eq!(
        config.streams.catalog,
        catalog(&["order-paid", "order.paid", "order_paid", "user.created"])
    );
}

#[rstest]
#[case::one("a", 1)]
#[case::a_thousand(&distinct_names(1_000), 1_000)]
#[case::a_thousand_and_a_repeat(&format!("{},t0", distinct_names(1_000)), 1_000)]
fn event_types_accepts_1_to_1000_distinct_names(#[case] raw: &str, #[case] count: usize) {
    let config = Config::from_args(with_event_types(raw)).unwrap();

    let seen: usize = (1..=10_u32)
        .map(|page| {
            config
                .streams
                .catalog
                .page(shared_domain::PageRequest::new(page, 100).unwrap())
                .len()
        })
        .sum();
    assert_eq!(seen, count);
}

#[rstest]
#[case::empty_value("")]
#[case::empty_entry("a,,b")]
#[case::trailing_comma("a,")]
#[case::leading_comma(",a")]
#[case::invalid_name("a,User.created")]
#[case::blank_name("a, b")]
#[case::too_long(&"a".repeat(129))]
#[case::a_thousand_and_one(&distinct_names(1_001))]
fn event_types_rejects_anything_else_without_echoing_it(#[case] raw: &str) {
    let reported = problems(with_event_types(raw));

    assert_eq!(reported, [("EVENT_TYPES", EVENT_TYPES_REASON.to_owned())]);
}

type Mutator = fn(&mut ServeArgs);

/// One row per variable of `ServeArgs`: the variable, a way to make it the only problem, and the
/// variable that the problem names.
const FIELDS: [(&str, Mutator, &str); 38] = [
    ("APP_ENV", |a| a.app_env = some("x"), "APP_ENV"),
    ("APP_HOST", |a| a.app_host = some("x"), "APP_HOST"),
    ("APP_PORT", |a| a.app_port = some("x"), "APP_PORT"),
    ("LOG_LEVEL", |a| a.log_level = some("x=y"), "LOG_LEVEL"),
    ("AWS_REGION", |a| a.aws_region = None, "AWS_REGION"),
    (
        "AWS_ENDPOINT_URL_SQS",
        |a| a.aws_endpoint_url_sqs = some("x"),
        "AWS_ENDPOINT_URL_SQS",
    ),
    (
        "AWS_ACCESS_KEY_ID",
        |a| a.aws_access_key_id = some("x"),
        "AWS_SECRET_ACCESS_KEY",
    ),
    (
        "AWS_SECRET_ACCESS_KEY",
        |a| a.aws_secret_access_key = some("x"),
        "AWS_ACCESS_KEY_ID",
    ),
    (
        "SQS_CONNECT_TIMEOUT",
        |a| a.sqs_connect_timeout = some("x"),
        "SQS_CONNECT_TIMEOUT",
    ),
    (
        "SQS_OPERATION_TIMEOUT",
        |a| a.sqs_operation_timeout = some("x"),
        "SQS_OPERATION_TIMEOUT",
    ),
    (
        "SQS_MAX_ATTEMPTS",
        |a| a.sqs_max_attempts = some("x"),
        "SQS_MAX_ATTEMPTS",
    ),
    (
        "HEALTH_PROBE_TIMEOUT",
        |a| a.health_probe_timeout = some("x"),
        "HEALTH_PROBE_TIMEOUT",
    ),
    (
        "HTTP_REQUEST_TIMEOUT",
        |a| a.http_request_timeout = some("x"),
        "HTTP_REQUEST_TIMEOUT",
    ),
    (
        "HTTP_MAX_BODY_BYTES",
        |a| a.http_max_body_bytes = some("x"),
        "HTTP_MAX_BODY_BYTES",
    ),
    (
        "SHUTDOWN_GRACE_PERIOD",
        |a| a.shutdown_grace_period = some("x"),
        "SHUTDOWN_GRACE_PERIOD",
    ),
    ("EVENT_TYPES", |a| a.event_types = some(""), "EVENT_TYPES"),
    (
        "KAFKA_BOOTSTRAP_SERVERS",
        |a| a.kafka.bootstrap_servers = some("x"),
        "KAFKA_BOOTSTRAP_SERVERS",
    ),
    ("KAFKA_TOPIC", |a| a.kafka.topic = some(""), "KAFKA_TOPIC"),
    (
        "KAFKA_SECURITY_PROTOCOL",
        |a| a.kafka.security_protocol = some("x"),
        "KAFKA_SECURITY_PROTOCOL",
    ),
    (
        "KAFKA_SASL_MECHANISM",
        |a| a.kafka.sasl_mechanism = some("PLAIN"),
        "KAFKA_SASL_MECHANISM",
    ),
    (
        "KAFKA_SASL_USERNAME",
        |a| a.kafka.sasl_username = some("u"),
        "KAFKA_SASL_USERNAME",
    ),
    (
        "KAFKA_SASL_PASSWORD",
        |a| a.kafka.sasl_password = some("p"),
        "KAFKA_SASL_PASSWORD",
    ),
    (
        "KAFKA_CLIENT_ID",
        |a| a.kafka.client_id = some(""),
        "KAFKA_CLIENT_ID",
    ),
    (
        "KAFKA_REQUEST_TIMEOUT",
        |a| a.kafka.request_timeout = some("x"),
        "KAFKA_REQUEST_TIMEOUT",
    ),
    (
        "KAFKA_CONNECT_TIMEOUT",
        |a| a.kafka.connect_timeout = some("x"),
        "KAFKA_CONNECT_TIMEOUT",
    ),
    (
        "KAFKA_FETCH_MAX_WAIT",
        |a| a.kafka.fetch_max_wait = some("x"),
        "KAFKA_FETCH_MAX_WAIT",
    ),
    (
        "KAFKA_RETRY_DELAY",
        |a| a.kafka.retry_delay = some("x"),
        "KAFKA_RETRY_DELAY",
    ),
    (
        "EVENT_MAX_DATA_BYTES",
        |a| a.streams.max_data_bytes = some("x"),
        "EVENT_MAX_DATA_BYTES",
    ),
    (
        "EVENT_WINDOW_MAX_EVENTS",
        |a| a.streams.window_max_events = some("x"),
        "EVENT_WINDOW_MAX_EVENTS",
    ),
    (
        "EVENT_WINDOW_MAX_BYTES",
        |a| a.streams.window_max_bytes = some("x"),
        "EVENT_WINDOW_MAX_BYTES",
    ),
    (
        "EVENT_WINDOW_START_EVENTS",
        |a| a.streams.window_start_events = some("x"),
        "EVENT_WINDOW_START_EVENTS",
    ),
    (
        "EVENT_REPLAY_MAX_EVENTS",
        |a| a.streams.replay_max_events = some("x"),
        "EVENT_REPLAY_MAX_EVENTS",
    ),
    (
        "EVENT_REPLAY_PERMITS",
        |a| a.streams.replay_permits = some("x"),
        "EVENT_REPLAY_PERMITS",
    ),
    (
        "EVENT_TAIL_STALE_AFTER",
        |a| a.streams.tail_stale_after = some("x"),
        "EVENT_TAIL_STALE_AFTER",
    ),
    (
        "SUBSCRIBER_BUFFER",
        |a| a.streams.subscriber_buffer = some("x"),
        "SUBSCRIBER_BUFFER",
    ),
    (
        "MAX_SUBSCRIBERS",
        |a| a.streams.max_subscribers = some("x"),
        "MAX_SUBSCRIBERS",
    ),
    (
        "STREAM_KEEP_ALIVE",
        |a| a.streams.keep_alive = some("x"),
        "STREAM_KEEP_ALIVE",
    ),
    (
        "CORS_ALLOWED_ORIGINS",
        |a| a.cors_allowed_origins = some("x"),
        "CORS_ALLOWED_ORIGINS",
    ),
];

#[test]
fn every_field_of_serve_args_is_consumed_by_from_args() {
    use clap::{Args as _, Command};

    let declared: Vec<String> = ServeArgs::augment_args(Command::new("serve"))
        .get_arguments()
        .filter_map(|argument| argument.get_env())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    let mut tested: Vec<&str> = FIELDS.iter().map(|(variable, _, _)| *variable).collect();
    tested.sort_unstable();
    let mut declared: Vec<&str> = declared.iter().map(String::as_str).collect();
    declared.sort_unstable();
    assert_eq!(declared, tested);

    for (variable, mutate, reported) in FIELDS {
        let mut args = valid();
        mutate(&mut args);
        assert_eq!(variables(args), [reported], "{variable} is not consumed");
    }
}

mod env_file;
mod kafka;
mod origins;
mod streams;
