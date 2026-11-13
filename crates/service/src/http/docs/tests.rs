use axum::Router;
use axum::body::Body;
use axum::http::header::{CONTENT_SECURITY_POLICY, CONTENT_TYPE, LOCATION};
use axum::http::{Request, Response, StatusCode};
use axum::routing::get;
use http_body_util::BodyExt as _;
use serde_json::Value;
use tower::ServiceExt as _;

use super::*;
use crate::http::security_headers;

fn router() -> Router {
    Router::new()
        .route("/api/docs", get(docs_page))
        .route("/api/docs/openapi.json", get(openapi_json))
        .route("/api/docs/{*file}", get(docs_asset))
        .layer(security_headers::layer())
}

async fn get_path(path: &str) -> Response<Body> {
    router()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

async fn body_of(response: Response<Body>) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

fn content_type(response: &Response<Body>) -> &str {
    response.headers()[CONTENT_TYPE].to_str().unwrap()
}

/// The values of every `href` and `src` attribute of the page.
fn references(page: &str) -> Vec<String> {
    let mut values = Vec::new();
    for attribute in ["href=\"", "src=\""] {
        for part in page.split(attribute).skip(1) {
            values.push(part.split('"').next().unwrap().to_owned());
        }
    }
    values
}

#[tokio::test]
async fn the_page_is_html_without_redirect_and_has_its_own_policy() {
    let response = get_path("/api/docs").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), "text/html; charset=utf-8");
    assert!(response.headers().get(LOCATION).is_none());
    assert_eq!(
        response
            .headers()
            .get_all(CONTENT_SECURITY_POLICY)
            .iter()
            .count(),
        1
    );
    assert_eq!(
        response.headers()[CONTENT_SECURITY_POLICY],
        DOCS_CONTENT_SECURITY_POLICY
    );
    assert_eq!(response.headers()["x-frame-options"], "DENY");
    let page = String::from_utf8(body_of(response).await).unwrap();
    assert!(page.contains("swagger-ui"));
}

#[tokio::test]
async fn the_page_references_no_third_party_host() {
    let page = String::from_utf8(body_of(get_path("/api/docs").await).await).unwrap();

    assert!(!page.contains("://"), "the page names a host");
    assert!(!page.contains("//cdn"), "the page names a host");
    for reference in references(&page) {
        assert!(reference.starts_with("/api/docs/"), "{reference}");
    }
}

#[test]
fn the_policy_allows_the_service_itself_and_nothing_else() {
    assert_eq!(
        DOCS_CONTENT_SECURITY_POLICY,
        "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'"
    );
}

/// FR8, AD-19: the assets come from the vendored crate, and a build without its feature stops instead of
/// downloading Swagger UI (the build script reads `SWAGGER_UI_DOWNLOAD_URL` only when the feature is off).
#[test]
fn the_swagger_ui_assets_come_from_the_vendored_crate() {
    let lock = include_str!("../../../../../Cargo.lock");
    let config = include_str!("../../../../../.cargo/config.toml");

    assert!(
        lock.contains("name = \"utoipa-swagger-ui-vendored\""),
        "the lock file has no utoipa-swagger-ui-vendored: the vendored feature is off"
    );
    // An active line: a commented-out guard stops nothing.
    assert!(
        config
            .lines()
            .any(|line| line.trim() == "SWAGGER_UI_DOWNLOAD_URL = \"vendored-only\""),
        ".cargo/config.toml does not stop a build that would download Swagger UI"
    );
}

#[tokio::test]
async fn every_asset_the_page_references_is_served() {
    let page = String::from_utf8(body_of(get_path("/api/docs").await).await).unwrap();
    let assets = references(&page);
    assert_eq!(assets.len(), 5);

    for asset in assets {
        let response = get_path(&asset).await;

        assert_eq!(response.status(), StatusCode::OK, "{asset}");
        let is_style = std::path::Path::new(&asset)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("css"));
        let kind = if is_style {
            "text/css"
        } else {
            "text/javascript"
        };
        assert!(content_type(&response).starts_with(kind), "{asset}");
        assert!(!body_of(response).await.is_empty(), "{asset}");
    }
}

#[tokio::test]
async fn the_start_up_script_points_at_the_json_document() {
    let response = get_path("/api/docs/swagger-initializer.js").await;

    let script = String::from_utf8(body_of(response).await).unwrap();
    assert!(script.contains("\"/api/docs/openapi.json\""), "{script}");
    assert!(!script.contains("{{config}}"));
}

#[tokio::test]
async fn the_json_document_is_the_openapi_document() {
    let response = get_path("/api/docs/openapi.json").await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(content_type(&response), "application/json");
    let served: Value = serde_json::from_slice(&body_of(response).await).unwrap();
    assert_eq!(served, serde_json::to_value(openapi::document()).unwrap());
    assert_eq!(served["openapi"], "3.1.0");
}

#[tokio::test]
async fn an_unknown_asset_is_the_404_problem() {
    let response = get_path("/api/docs/nope.js").await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(content_type(&response), "application/problem+json");
    let problem: Value = serde_json::from_slice(&body_of(response).await).unwrap();
    assert_eq!(problem["type"], "about:blank");
    assert_eq!(problem["title"], "Not Found");
    assert_eq!(problem["status"], 404);
}

#[tokio::test]
async fn a_sub_path_that_is_not_utf_8_is_the_400_problem() {
    let response = get_path("/api/docs/%FF").await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(content_type(&response), "application/problem+json");
    let problem: Value = serde_json::from_slice(&body_of(response).await).unwrap();
    assert_eq!(problem["type"], "about:blank");
    assert_eq!(problem["title"], "Bad Request");
    assert_eq!(problem["status"], 400);
}
