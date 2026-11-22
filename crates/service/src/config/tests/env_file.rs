//! The `.env` of the repository against FR-29: a line with the default of every variable of the
//! table that has one, the local broker in the required `KAFKA_BOOTSTRAP_SERVERS`, and no line for
//! the variables without a default.

use std::collections::BTreeMap;
use std::ffi::OsStr;

use clap::{Arg, Args as _, Command, FromArgMatches as _};
use rstest::rstest;

use super::*;

/// The `.env` of the repository, which `cargo run` and the local compose stack read.
const REPOSITORY_ENV_FILE: &str = include_str!("../../../../../.env");

/// The compose stack, whose Kafka service advertises the local broker to the host.
const COMPOSE_FILE: &str = include_str!("../../../../../docker-compose.yml");

/// The address of the local broker: the `EXTERNAL` listener of the compose stack.
const LOCAL_BROKER: &str = "localhost:9095";

/// The FR-29 variables that have a default, and the required `KAFKA_BOOTSTRAP_SERVERS`.
const DEFAULTED: [&str; 18] = [
    "KAFKA_BOOTSTRAP_SERVERS",
    "KAFKA_TOPIC",
    "KAFKA_SECURITY_PROTOCOL",
    "KAFKA_CLIENT_ID",
    "KAFKA_REQUEST_TIMEOUT",
    "KAFKA_CONNECT_TIMEOUT",
    "KAFKA_FETCH_MAX_WAIT",
    "KAFKA_RETRY_DELAY",
    "EVENT_MAX_DATA_BYTES",
    "EVENT_WINDOW_MAX_EVENTS",
    "EVENT_WINDOW_MAX_BYTES",
    "EVENT_WINDOW_START_EVENTS",
    "EVENT_REPLAY_MAX_EVENTS",
    "EVENT_REPLAY_PERMITS",
    "EVENT_TAIL_STALE_AFTER",
    "SUBSCRIBER_BUFFER",
    "MAX_SUBSCRIBERS",
    "STREAM_KEEP_ALIVE",
];

/// The FR-29 variables without a default.
const UNSET: [&str; 5] = [
    "EVENT_TYPES",
    "KAFKA_SASL_MECHANISM",
    "KAFKA_SASL_USERNAME",
    "KAFKA_SASL_PASSWORD",
    "CORS_ALLOWED_ORIGINS",
];

/// The variables an env file sets, read as `dotenvy` reads them: a later line wins.
fn env_file_variables(contents: &str) -> BTreeMap<String, String> {
    dotenvy::from_read_iter(contents.as_bytes())
        .map(Result::unwrap)
        .collect()
}

/// The arguments of `serve` when only `variable` and the required `KAFKA_BOOTSTRAP_SERVERS` (the
/// local broker, unless `variable` is that one) are set: the value reaches the field that clap binds
/// to the variable, and the process environment is not read.
fn serve_args_with(variable: &str, value: &str) -> ServeArgs {
    let command = ServeArgs::augment_args(Command::new("serve"));
    let flag = command
        .get_arguments()
        .find(|argument| argument.get_env() == Some(OsStr::new(variable)))
        .and_then(Arg::get_long)
        .unwrap()
        .to_owned();
    let assignment = format!("--{flag}={value}");
    let broker = format!("--kafka-bootstrap-servers={LOCAL_BROKER}");
    let required = (variable != "KAFKA_BOOTSTRAP_SERVERS").then_some(broker.as_str());
    let arguments = std::iter::once("serve")
        .chain(required)
        .chain(std::iter::once(assignment.as_str()));
    let matches = command
        .mut_args(|argument| argument.env(None::<&'static str>))
        .try_get_matches_from(arguments)
        .unwrap();
    ServeArgs::from_arg_matches(&matches).unwrap()
}

/// Whether the line `variable=value` alone leaves the Kafka and stream settings at their defaults,
/// with the local broker in `KAFKA_BOOTSTRAP_SERVERS`.
fn holds_its_default(variable: &str, value: &str) -> bool {
    let defaults = Config::from_args(valid()).unwrap();
    let mut kafka = defaults.kafka;
    LOCAL_BROKER.clone_into(&mut kafka.bootstrap_servers);
    Config::from_args(serve_args_with(variable, value))
        .is_ok_and(|config| config.kafka == kafka && config.streams == defaults.streams)
}

/// The FR-29 variables an env file gets wrong: first each variable with a default whose line is
/// missing or holds another value, then each variable without a default that has a line.
fn drift(contents: &str) -> Vec<&'static str> {
    let variables = env_file_variables(contents);
    let wrong = DEFAULTED.into_iter().filter(|variable| {
        !variables
            .get(*variable)
            .is_some_and(|value| holds_its_default(variable, value))
    });
    let present = UNSET
        .into_iter()
        .filter(|variable| variables.contains_key(*variable));
    wrong.chain(present).collect()
}

#[test]
fn the_env_file_holds_every_default_the_local_broker_and_no_unset_variable() {
    assert!(COMPOSE_FILE.contains(&format!("EXTERNAL://{LOCAL_BROKER}")));

    assert_eq!(drift(REPOSITORY_ENV_FILE), Vec::<&str>::new());
}

#[rstest]
#[case::a_changed_default(None, "STREAM_KEEP_ALIVE=20s", &["STREAM_KEEP_ALIVE"])]
#[case::an_invalid_value(None, "EVENT_REPLAY_PERMITS=0", &["EVENT_REPLAY_PERMITS"])]
#[case::the_default_broker(
    None,
    "KAFKA_BOOTSTRAP_SERVERS=localhost:9092",
    &["KAFKA_BOOTSTRAP_SERVERS"]
)]
#[case::a_missing_line(Some("KAFKA_TOPIC"), "", &["KAFKA_TOPIC"])]
#[case::an_event_types_line(None, "EVENT_TYPES=user.created", &["EVENT_TYPES"])]
#[case::a_sasl_mechanism_line(None, "KAFKA_SASL_MECHANISM=PLAIN", &["KAFKA_SASL_MECHANISM"])]
#[case::a_sasl_username_line(None, "KAFKA_SASL_USERNAME=reader", &["KAFKA_SASL_USERNAME"])]
#[case::a_sasl_password_line(None, "KAFKA_SASL_PASSWORD=x", &["KAFKA_SASL_PASSWORD"])]
#[case::a_cors_line(None, "CORS_ALLOWED_ORIGINS=*", &["CORS_ALLOWED_ORIGINS"])]
fn a_drifted_env_file_names_the_drifted_variable(
    #[case] removed: Option<&str>,
    #[case] added: &str,
    #[case] expected: &[&str],
) {
    let kept = REPOSITORY_ENV_FILE
        .lines()
        .filter(|line| removed.is_none_or(|variable| !line.starts_with(&format!("{variable}="))));
    let contents: Vec<&str> = kept.chain([added]).collect();

    assert_eq!(drift(&contents.join("\n")), expected);
}
