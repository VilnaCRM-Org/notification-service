//! The Kafka variables: each parser, the rule between the two time-outs and the SASL rule.

use rstest::rstest;

use super::*;

fn with_kafka(mutate: impl FnOnce(&mut KafkaArgs)) -> ServeArgs {
    let mut args = valid();
    mutate(&mut args.kafka);
    args
}

fn kafka(args: ServeArgs) -> KafkaSettings {
    Config::from_args(args).unwrap().kafka
}

const SERVERS_REASON: &str = "must be a comma-separated list of host:port addresses";
const TOPIC_REASON: &str = "must be a Kafka topic name";
const PROTOCOL_REASON: &str = "must be one of plaintext, ssl, sasl_plaintext, sasl_ssl";
const MECHANISM_REASON: &str = "must be one of PLAIN, SCRAM-SHA-256, SCRAM-SHA-512";
const REQUIRED: &str = "is required when KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl";
const FORBIDDEN: &str =
    "must not be set unless KAFKA_SECURITY_PROTOCOL is sasl_plaintext or sasl_ssl";

/// `count` distinct `host:port` entries joined by commas.
fn servers(count: usize) -> String {
    (0..count)
        .map(|index| format!("broker{index}:9092"))
        .collect::<Vec<_>>()
        .join(",")
}

/// A DNS name of exactly `length` characters made of labels of at most 63 characters.
fn dns_name(length: usize) -> String {
    let mut name = String::new();
    while name.len() < length {
        if !name.is_empty() {
            name.push('.');
        }
        let label = length.saturating_sub(name.len()).min(63);
        name.push_str(&"a".repeat(label));
    }
    name
}

#[rstest]
#[case::two("a:9092,b:9093")]
#[case::ipv4("10.0.0.1:9092")]
#[case::ipv6("[::1]:9092")]
#[case::lowest_port("a:1")]
#[case::highest_port("a:65535")]
#[case::sixteen(&servers(16))]
#[case::hyphen_inside("my-broker.example.com:9092")]
#[case::label_of_63(&format!("{}:9092", "a".repeat(63)))]
#[case::name_of_253(&format!("{}:9092", dns_name(253)))]
fn bootstrap_servers_accepts_host_port_lists(#[case] raw: &str) {
    let settings = kafka(with_kafka(|a| a.bootstrap_servers = some(raw)));

    assert_eq!(settings.bootstrap_servers, raw);
}

#[rstest]
#[case::empty("")]
#[case::empty_entry("a:9092,,b:9092")]
#[case::trailing_comma("a:9092,")]
#[case::missing_port("a")]
#[case::empty_port("a:")]
#[case::port_zero("a:0")]
#[case::port_too_high("a:65536")]
#[case::signed_port("a:+1")]
#[case::unbracketed_ipv6("::1:9092")]
#[case::bracketed_garbage("[x]:9092")]
#[case::unclosed_bracket("[::1:9092")]
#[case::empty_host(":9092")]
#[case::seventeen(&servers(17))]
#[case::leading_hyphen("-a:9092")]
#[case::trailing_hyphen("a-:9092")]
#[case::empty_label("a..b:9092")]
#[case::underscore("a_b:9092")]
#[case::label_of_64(&format!("{}:9092", "a".repeat(64)))]
#[case::name_of_254(&format!("{}:9092", dns_name(254)))]
fn bootstrap_servers_rejects_anything_else(#[case] raw: &str) {
    let reported = problems(with_kafka(|a| a.bootstrap_servers = some(raw)));

    assert_eq!(
        reported,
        [("KAFKA_BOOTSTRAP_SERVERS", SERVERS_REASON.to_owned())]
    );
}

#[test]
fn the_dns_name_helper_builds_the_requested_length() {
    assert_eq!(dns_name(253).len(), 253);
    assert_eq!(dns_name(254).len(), 254);
    assert!(dns_name(254).split('.').all(|label| label.len() <= 63));
}

#[rstest]
#[case::one("a")]
#[case::every_allowed_character("Az09._-")]
#[case::longest(&"t".repeat(249))]
#[case::three_dots("...")]
#[case::dot_prefix(".a")]
fn topic_accepts_kafka_topic_names(#[case] raw: &str) {
    let settings = kafka(with_kafka(|a| a.topic = some(raw)));

    assert_eq!(settings.topic, raw);
}

#[rstest]
#[case::empty("")]
#[case::too_long(&"t".repeat(250))]
#[case::dot(".")]
#[case::two_dots("..")]
#[case::slash("a/b")]
#[case::space("a b")]
#[case::non_ascii("topic\u{e9}")]
fn topic_rejects_anything_else(#[case] raw: &str) {
    let reported = problems(with_kafka(|a| a.topic = some(raw)));

    assert_eq!(reported, [("KAFKA_TOPIC", TOPIC_REASON.to_owned())]);
}

#[rstest]
#[case::one("c")]
#[case::longest(&"c".repeat(128))]
#[case::longest_in_characters(&"\u{e9}".repeat(128))]
fn client_id_accepts_1_to_128_characters(#[case] raw: &str) {
    let settings = kafka(with_kafka(|a| a.client_id = some(raw)));

    assert_eq!(settings.client_id, raw);
}

#[rstest]
#[case::empty("")]
#[case::too_long(&"c".repeat(129))]
fn client_id_rejects_an_empty_or_too_long_value(#[case] raw: &str) {
    let reported = problems(with_kafka(|a| a.client_id = some(raw)));

    assert_eq!(
        reported,
        [("KAFKA_CLIENT_ID", "must be 1 to 128 characters".to_owned())]
    );
}

type DurationField = fn(&mut KafkaArgs, &str);

const REQUEST: DurationField = |a, v| a.request_timeout = some(v);
const CONNECT: DurationField = |a, v| a.connect_timeout = some(v);
const FETCH: DurationField = |a, v| a.fetch_max_wait = some(v);
const RETRY: DurationField = |a, v| a.retry_delay = some(v);

#[rstest]
#[case::request_low(REQUEST, "1s", "KAFKA_REQUEST_TIMEOUT")]
#[case::request_high(REQUEST, "30s", "KAFKA_REQUEST_TIMEOUT")]
#[case::connect_low(CONNECT, "100ms", "KAFKA_CONNECT_TIMEOUT")]
#[case::fetch_low(FETCH, "50ms", "KAFKA_FETCH_MAX_WAIT")]
#[case::fetch_high(FETCH, "5s", "KAFKA_FETCH_MAX_WAIT")]
#[case::retry_low(RETRY, "100ms", "KAFKA_RETRY_DELAY")]
#[case::retry_high(RETRY, "1m", "KAFKA_RETRY_DELAY")]
fn each_kafka_duration_accepts_its_bounds(
    #[case] field: DurationField,
    #[case] raw: &str,
    #[case] variable: &str,
) {
    let config = Config::from_args(with_kafka(|a| {
        a.connect_timeout = some("100ms");
        field(a, raw);
    }))
    .unwrap();

    let expected = humantime::parse_duration(raw).unwrap();
    let actual = match variable {
        "KAFKA_REQUEST_TIMEOUT" => config.kafka.request_timeout,
        "KAFKA_CONNECT_TIMEOUT" => config.kafka.connect_timeout,
        "KAFKA_FETCH_MAX_WAIT" => config.kafka.fetch_wait,
        _ => config.streams.retry_delay,
    };
    assert_eq!(actual, expected, "{variable}");
}

#[test]
fn the_connect_time_out_accepts_30_seconds_with_a_request_time_out_of_30_seconds() {
    let settings = kafka(with_kafka(|a| {
        a.connect_timeout = some("30s");
        a.request_timeout = some("30s");
    }));

    assert_eq!(settings.connect_timeout, Duration::from_secs(30));
    assert_eq!(settings.request_timeout, Duration::from_secs(30));
}

#[rstest]
#[case::request_low(REQUEST, "999ms", "KAFKA_REQUEST_TIMEOUT", "1s to 30s")]
#[case::request_high(REQUEST, "31s", "KAFKA_REQUEST_TIMEOUT", "1s to 30s")]
#[case::request_text(REQUEST, "soon", "KAFKA_REQUEST_TIMEOUT", "1s to 30s")]
#[case::connect_low(CONNECT, "99ms", "KAFKA_CONNECT_TIMEOUT", "100ms to 30s")]
#[case::connect_high(CONNECT, "31s", "KAFKA_CONNECT_TIMEOUT", "100ms to 30s")]
#[case::fetch_low(FETCH, "49ms", "KAFKA_FETCH_MAX_WAIT", "50ms to 5s")]
#[case::fetch_high(FETCH, "5001ms", "KAFKA_FETCH_MAX_WAIT", "50ms to 5s")]
#[case::retry_low(RETRY, "99ms", "KAFKA_RETRY_DELAY", "100ms to 1m")]
#[case::retry_high(RETRY, "61s", "KAFKA_RETRY_DELAY", "100ms to 1m")]
fn each_kafka_duration_rejects_a_value_outside_its_range(
    #[case] field: DurationField,
    #[case] raw: &str,
    #[case] variable: &str,
    #[case] range: &str,
) {
    let reported = problems(with_kafka(|a| field(a, raw)));

    assert_eq!(
        reported,
        [(variable, format!("must be a duration from {range}"))]
    );
}

#[test]
fn a_connect_time_out_longer_than_the_request_time_out_is_reported_after_the_variables() {
    let args = ServeArgs {
        cors_allowed_origins: some("x"),
        ..with_kafka(|a| {
            a.connect_timeout = some("4s");
            a.request_timeout = some("3s");
            a.topic = some("");
        })
    };

    assert_eq!(
        problems(args),
        [
            ("KAFKA_TOPIC", TOPIC_REASON.to_owned()),
            (
                "CORS_ALLOWED_ORIGINS",
                "must be * or a comma-separated list of origins such as https://app.example.com"
                    .to_owned()
            ),
            (
                "KAFKA_CONNECT_TIMEOUT",
                "must not be longer than KAFKA_REQUEST_TIMEOUT".to_owned()
            ),
        ]
    );
}

#[test]
fn a_connect_time_out_equal_to_the_request_time_out_is_accepted() {
    let settings = kafka(with_kafka(|a| {
        a.connect_timeout = some("3s");
        a.request_timeout = some("3s");
    }));

    assert_eq!(settings.connect_timeout, Duration::from_secs(3));
}

#[test]
fn the_time_out_rule_is_skipped_when_one_time_out_is_invalid() {
    let invalid_request = with_kafka(|a| {
        a.connect_timeout = some("20s");
        a.request_timeout = some("31s");
    });
    let invalid_connect = with_kafka(|a| {
        a.connect_timeout = some("31s");
        a.request_timeout = some("1s");
    });

    assert_eq!(variables(invalid_request), ["KAFKA_REQUEST_TIMEOUT"]);
    assert_eq!(variables(invalid_connect), ["KAFKA_CONNECT_TIMEOUT"]);
}

#[rstest]
#[case::plaintext("plaintext", KafkaSecurity::Plaintext)]
#[case::ssl("ssl", KafkaSecurity::Ssl)]
fn the_protocols_without_sasl_need_no_credentials(
    #[case] raw: &str,
    #[case] expected: KafkaSecurity,
) {
    let settings = kafka(with_kafka(|a| a.security_protocol = some(raw)));

    assert_eq!(settings.security, expected);
}

fn sasl(protocol: &str, mechanism: &str) -> ServeArgs {
    with_kafka(|a| {
        a.security_protocol = some(protocol);
        a.sasl_mechanism = some(mechanism);
        a.sasl_username = some("reader");
        a.sasl_password = some("s3cret-value");
    })
}

fn credentials(mechanism: SaslMechanism) -> SaslCredentials {
    SaslCredentials {
        mechanism,
        username: "reader".to_owned(),
        password: "s3cret-value".to_owned(),
    }
}

#[rstest]
#[case::plain_over_plaintext(
    "sasl_plaintext",
    "PLAIN",
    KafkaSecurity::SaslPlaintext(credentials(SaslMechanism::Plain))
)]
#[case::scram_256_over_ssl(
    "sasl_ssl",
    "SCRAM-SHA-256",
    KafkaSecurity::SaslSsl(credentials(SaslMechanism::ScramSha256))
)]
#[case::scram_512_over_ssl(
    "sasl_ssl",
    "SCRAM-SHA-512",
    KafkaSecurity::SaslSsl(credentials(SaslMechanism::ScramSha512))
)]
#[case::scram_512_over_plaintext(
    "sasl_plaintext",
    "SCRAM-SHA-512",
    KafkaSecurity::SaslPlaintext(credentials(SaslMechanism::ScramSha512))
)]
fn a_sasl_protocol_carries_the_three_credentials(
    #[case] protocol: &str,
    #[case] mechanism: &str,
    #[case] expected: KafkaSecurity,
) {
    assert_eq!(kafka(sasl(protocol, mechanism)).security, expected);
}

#[rstest]
#[case::upper_case("PLAINTEXT")]
#[case::unknown("tls")]
#[case::empty("")]
fn an_unknown_protocol_is_rejected(#[case] raw: &str) {
    let reported = problems(with_kafka(|a| a.security_protocol = some(raw)));

    assert_eq!(
        reported,
        [("KAFKA_SECURITY_PROTOCOL", PROTOCOL_REASON.to_owned())]
    );
}

#[rstest]
#[case::lower_case("plain")]
#[case::unknown("GSSAPI")]
fn an_unknown_mechanism_is_rejected(#[case] raw: &str) {
    let reported = problems(sasl("sasl_ssl", raw));

    assert_eq!(
        reported,
        [("KAFKA_SASL_MECHANISM", MECHANISM_REASON.to_owned())]
    );
}

#[rstest]
#[case::sasl_plaintext("sasl_plaintext")]
#[case::sasl_ssl("sasl_ssl")]
fn a_sasl_protocol_requires_the_three_variables_after_the_others(#[case] protocol: &str) {
    let args = ServeArgs {
        cors_allowed_origins: some("x"),
        ..with_kafka(|a| a.security_protocol = some(protocol))
    };

    assert_eq!(
        problems(args),
        [
            (
                "CORS_ALLOWED_ORIGINS",
                "must be * or a comma-separated list of origins such as https://app.example.com"
                    .to_owned()
            ),
            ("KAFKA_SASL_MECHANISM", REQUIRED.to_owned()),
            ("KAFKA_SASL_USERNAME", REQUIRED.to_owned()),
            ("KAFKA_SASL_PASSWORD", REQUIRED.to_owned()),
        ]
    );
}

#[test]
fn only_the_missing_sasl_variable_is_required() {
    let args = with_kafka(|a| {
        a.security_protocol = some("sasl_ssl");
        a.sasl_mechanism = some("PLAIN");
        a.sasl_password = some("s3cret-value");
    });

    assert_eq!(
        problems(args),
        [("KAFKA_SASL_USERNAME", REQUIRED.to_owned())]
    );
}

#[rstest]
#[case::plaintext("plaintext")]
#[case::ssl("ssl")]
fn the_sasl_variables_must_be_unset_without_a_sasl_protocol(#[case] protocol: &str) {
    let all = sasl(protocol, "PLAIN");
    let one = with_kafka(|a| {
        a.security_protocol = some(protocol);
        a.sasl_password = some("s3cret-value");
    });

    assert_eq!(
        problems(all),
        [
            ("KAFKA_SASL_MECHANISM", FORBIDDEN.to_owned()),
            ("KAFKA_SASL_USERNAME", FORBIDDEN.to_owned()),
            ("KAFKA_SASL_PASSWORD", FORBIDDEN.to_owned()),
        ]
    );
    assert_eq!(
        problems(one),
        [("KAFKA_SASL_PASSWORD", FORBIDDEN.to_owned())]
    );
}

#[test]
fn the_default_protocol_forbids_a_sasl_variable() {
    let args = with_kafka(|a| a.sasl_username = some("reader"));

    assert_eq!(
        problems(args),
        [("KAFKA_SASL_USERNAME", FORBIDDEN.to_owned())]
    );
}

#[test]
fn an_empty_credential_is_reported_once_with_a_sasl_protocol() {
    let args = with_kafka(|a| {
        a.security_protocol = some("sasl_plaintext");
        a.sasl_mechanism = some("PLAIN");
        a.sasl_username = some(" ");
        a.sasl_password = some("");
    });

    assert_eq!(
        problems(args),
        [
            ("KAFKA_SASL_USERNAME", "must not be empty".to_owned()),
            ("KAFKA_SASL_PASSWORD", "must not be empty".to_owned()),
        ]
    );
}

#[test]
fn the_sasl_rule_is_skipped_when_the_protocol_is_invalid() {
    let args = with_kafka(|a| {
        a.security_protocol = some("sasl");
        a.sasl_username = some("reader");
    });

    assert_eq!(variables(args), ["KAFKA_SECURITY_PROTOCOL"]);
}

#[test]
fn the_password_is_never_printed() {
    let args = ServeArgs {
        cors_allowed_origins: some("x"),
        ..sasl("plaintext", "PLAIN")
    };

    let errors = Config::from_args(args).unwrap_err();

    assert!(!format!("{errors:?}").contains("s3cret-value"));
    assert!(!errors.to_string().contains("s3cret-value"));
    let settings = kafka(sasl("sasl_ssl", "PLAIN"));
    let printed = format!("{settings:?}");
    assert!(!printed.contains("s3cret-value"), "{printed}");
    assert!(printed.contains("<redacted>"), "{printed}");
}

#[test]
fn no_problem_line_prints_a_value() {
    let args = ServeArgs {
        kafka: KafkaArgs {
            bootstrap_servers: some("host-value"),
            topic: some("topic/value"),
            security_protocol: some("protocol-value"),
            sasl_mechanism: some("mechanism-value"),
            sasl_username: some("username-value"),
            sasl_password: some("s3cret-value"),
            client_id: some(""),
            request_timeout: some("request-value"),
            connect_timeout: some("connect-value"),
            fetch_max_wait: some("fetch-value"),
            retry_delay: some("retry-value"),
        },
        ..valid()
    };

    let reported = problems(args);

    assert_eq!(reported.len(), 9, "{reported:?}");
    for (variable, reason) in reported {
        assert!(!reason.contains("value"), "{variable}: {reason}");
    }
}
