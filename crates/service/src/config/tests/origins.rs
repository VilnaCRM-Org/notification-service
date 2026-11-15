//! `CORS_ALLOWED_ORIGINS` and [`AllowedOrigins::allows`].

use rstest::rstest;

use super::*;

const REASON: &str =
    "must be * or a comma-separated list of origins such as https://app.example.com";

fn origins(raw: &str) -> Result<AllowedOrigins, Vec<(&'static str, String)>> {
    let args = ServeArgs {
        cors_allowed_origins: some(raw),
        ..valid()
    };
    match Config::from_args(args.clone()) {
        Ok(config) => Ok(config.allowed_origins),
        Err(_) => Err(problems(args)),
    }
}

fn value(origin: &str) -> HeaderValue {
    HeaderValue::from_str(origin).unwrap()
}

#[test]
fn a_star_allows_every_origin() {
    assert_eq!(origins("*"), Ok(AllowedOrigins::Any));
}

#[test]
fn a_list_keeps_its_origins_in_order() {
    assert_eq!(
        origins("https://app.example.com,http://localhost:3000"),
        Ok(AllowedOrigins::List(vec![
            value("https://app.example.com"),
            value("http://localhost:3000"),
        ]))
    );
}

#[rstest]
#[case::https("https://app.example.com")]
#[case::http_with_port("http://localhost:3000")]
#[case::ipv4("http://127.0.0.1:8080")]
#[case::ipv6("http://[::1]:8080")]
#[case::digits_and_hyphen("https://a-1.example.com")]
fn a_browser_origin_is_accepted(#[case] raw: &str) {
    assert_eq!(origins(raw), Ok(AllowedOrigins::List(vec![value(raw)])));
}

#[rstest]
#[case::star_in_a_list("*,https://app.example.com")]
#[case::star_after_an_origin("https://app.example.com,*")]
#[case::empty_value("")]
#[case::empty_entry("https://a.example.com,,https://b.example.com")]
#[case::trailing_comma("https://app.example.com,")]
#[case::upper_case_host("https://App.example.com")]
#[case::upper_case_scheme("HTTPS://app.example.com")]
#[case::path("https://app.example.com/app")]
#[case::trailing_slash("https://app.example.com/")]
#[case::query("https://app.example.com?a=1")]
#[case::fragment("https://app.example.com#top")]
#[case::user_information("https://user@app.example.com")]
#[case::other_scheme("ftp://app.example.com")]
#[case::no_scheme("app.example.com")]
#[case::no_host("https://")]
#[case::port_without_host("https://:8080")]
#[case::empty_port("https://app.example.com:")]
#[case::port_too_high("https://app.example.com:65536")]
#[case::space("https://app example.com")]
fn anything_else_is_rejected_without_echoing_it(#[case] raw: &str) {
    assert_eq!(
        origins(raw),
        Err(vec![("CORS_ALLOWED_ORIGINS", REASON.to_owned())])
    );
}

#[test]
fn no_origin_allows_nothing() {
    assert!(!AllowedOrigins::NoOrigin.allows(&value("https://app.example.com")));
}

#[test]
fn any_allows_everything() {
    assert!(AllowedOrigins::Any.allows(&value("https://app.example.com")));
    assert!(AllowedOrigins::Any.allows(&value("null")));
}

#[test]
fn a_list_allows_its_entries_byte_for_byte() {
    let list = AllowedOrigins::List(vec![
        value("https://app.example.com"),
        value("http://localhost:3000"),
    ]);

    assert!(list.allows(&value("https://app.example.com")));
    assert!(list.allows(&value("http://localhost:3000")));
    assert!(!list.allows(&value("https://app.example.com:443")));
    assert!(!list.allows(&value("https://APP.example.com")));
    assert!(!list.allows(&value("http://app.example.com")));
    assert!(!AllowedOrigins::List(Vec::new()).allows(&value("http://localhost:3000")));
}
