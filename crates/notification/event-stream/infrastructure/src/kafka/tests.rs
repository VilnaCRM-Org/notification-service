use std::time::Duration;

use super::*;

fn credentials(mechanism: SaslMechanism) -> SaslCredentials {
    SaslCredentials {
        mechanism,
        username: "reader".to_owned(),
        password: "s3cret-value".to_owned(),
    }
}

fn settings(security: KafkaSecurity) -> KafkaSettings {
    KafkaSettings {
        bootstrap_servers: "localhost:9092".to_owned(),
        topic: "notifications".to_owned(),
        client_id: "notification-service".to_owned(),
        security,
        request_timeout: Duration::from_secs(3),
        connect_timeout: Duration::from_secs(2),
        fetch_wait: Duration::from_millis(500),
    }
}

#[test]
fn the_debug_output_of_the_credentials_redacts_the_password() {
    let printed = format!("{:?}", credentials(SaslMechanism::ScramSha512));

    assert_eq!(
        printed,
        r#"SaslCredentials { mechanism: ScramSha512, username: "reader", password: <redacted> }"#
    );
}

#[test]
fn the_debug_output_of_the_settings_never_holds_the_password() {
    for security in [
        KafkaSecurity::SaslPlaintext(credentials(SaslMechanism::Plain)),
        KafkaSecurity::SaslSsl(credentials(SaslMechanism::ScramSha256)),
    ] {
        let printed = format!("{:?}", settings(security));

        assert!(!printed.contains("s3cret-value"), "{printed}");
        assert!(printed.contains("password: <redacted>"), "{printed}");
        assert!(printed.contains(r#"username: "reader""#), "{printed}");
    }
}

#[test]
fn the_redacted_output_is_the_same_for_every_password() {
    let mut other = credentials(SaslMechanism::Plain);
    other.password = "another".to_owned();

    assert_eq!(
        format!("{other:?}"),
        format!("{:?}", credentials(SaslMechanism::Plain))
    );
    assert_ne!(other, credentials(SaslMechanism::Plain));
}

#[test]
fn the_settings_without_credentials_print_every_field() {
    let printed = format!("{:?}", settings(KafkaSecurity::Plaintext));

    assert_eq!(
        printed,
        "KafkaSettings { bootstrap_servers: \"localhost:9092\", topic: \"notifications\", \
         client_id: \"notification-service\", security: Plaintext, request_timeout: 3s, \
         connect_timeout: 2s, fetch_wait: 500ms }"
    );
}
