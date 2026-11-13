use super::*;

#[test]
fn a_statement_timeout_is_sent_in_whole_milliseconds() {
    assert_eq!(
        statement_timeout_millis(Duration::from_millis(1500)),
        "1500"
    );
    assert_eq!(statement_timeout_millis(MIN_STATEMENT_TIMEOUT), "1");
    assert_eq!(
        statement_timeout_millis(MAX_STATEMENT_TIMEOUT),
        "2073600000"
    );
}

#[test]
fn a_zero_or_sub_millisecond_statement_timeout_never_disables_the_limit() {
    assert_eq!(statement_timeout_millis(Duration::ZERO), "1");
    assert_eq!(statement_timeout_millis(Duration::from_micros(500)), "1");
}

#[test]
fn a_statement_timeout_above_the_maximum_is_lowered_to_it() {
    assert_eq!(
        statement_timeout_millis(Duration::from_hours(30 * 24)),
        "2073600000"
    );
    assert_eq!(statement_timeout_millis(Duration::MAX), "2073600000");
}

#[tokio::test]
async fn an_unparsable_url_is_rejected_without_network_access() {
    let settings = PostgresSettings {
        url: "not a url".to_owned(),
        max_connections: 1,
        acquire_timeout: Duration::from_millis(100),
        statement_timeout: Duration::from_secs(1),
    };

    let result = connect_lazy(&settings);

    assert!(matches!(result, Err(sqlx::Error::Configuration(_))));
}

#[tokio::test]
async fn a_valid_url_gives_a_pool_that_has_not_connected() {
    let settings = PostgresSettings {
        url: "postgres://app:unused@127.0.0.1:1/app".to_owned(),
        max_connections: 3,
        acquire_timeout: Duration::from_millis(100),
        statement_timeout: Duration::from_secs(1),
    };

    let pool = connect_lazy(&settings).unwrap();

    assert_eq!(pool.size(), 0);
    assert_eq!(pool.options().get_max_connections(), 3);
    assert_eq!(
        pool.options().get_acquire_timeout(),
        Duration::from_millis(100)
    );
}
