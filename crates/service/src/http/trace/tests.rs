use std::sync::{Arc, Mutex, PoisonError};

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderValue, Request};
use axum::middleware::from_fn;
use axum::routing::get;
use tower::{ServiceBuilder, ServiceExt as _};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::field::{Field, Visit};
use tracing::span::Attributes;
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt as _};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, Registry};

use super::*;
use crate::http::request_id::sanitize_request_id;

type Fields = Vec<(String, String)>;

#[derive(Debug, Clone)]
struct Entry {
    level: Level,
    message: String,
    fields: Fields,
    span_name: Option<String>,
    span_fields: Fields,
}

#[derive(Default)]
struct Collect(Fields);

impl Visit for Collect {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0.push((field.name().to_owned(), format!("{value:?}")));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.0.push((field.name().to_owned(), value.to_owned()));
    }
}

struct SpanFields(Fields);

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<Entry>>>);

impl<S> Layer<S> for Capture
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attributes: &Attributes<'_>, id: &tracing::Id, context: Context<'_, S>) {
        let mut collect = Collect::default();
        attributes.record(&mut collect);
        if let Some(span) = context.span(id) {
            span.extensions_mut().insert(SpanFields(collect.0));
        }
    }

    fn on_event(&self, event: &Event<'_>, context: Context<'_, S>) {
        let mut collect = Collect::default();
        event.record(&mut collect);
        let message = collect
            .0
            .iter()
            .find(|(name, _value)| name == "message")
            .map(|(_name, value)| value.clone())
            .unwrap_or_default();
        let fields = collect
            .0
            .into_iter()
            .filter(|(name, _value)| name != "message")
            .collect();
        let span = context.event_span(event);
        let entry = Entry {
            level: *event.metadata().level(),
            message,
            fields,
            span_name: span.as_ref().map(|span| span.name().to_owned()),
            span_fields: span
                .and_then(|span| {
                    span.extensions()
                        .get::<SpanFields>()
                        .map(|stored| stored.0.clone())
                })
                .unwrap_or_default(),
        };
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(entry);
    }
}

fn value<'a>(fields: &'a Fields, name: &str) -> &'a str {
    fields
        .iter()
        .find(|(field, _value)| field == name)
        .unwrap_or_else(|| panic!("no field {name} in {fields:?}"))
        .1
        .as_str()
}

fn router() -> Router {
    Router::new()
        .route("/items/{id}", get(|| std::future::ready("item")))
        .layer(
            ServiceBuilder::new()
                .layer(from_fn(sanitize_request_id))
                .layer(SetRequestIdLayer::new(REQUEST_ID_HEADER, MakeRequestUuid))
                .layer(PropagateRequestIdLayer::new(REQUEST_ID_HEADER))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(make_span)
                        .on_response(on_response)
                        .on_failure(()),
                ),
        )
}

async fn completed(path: &str, request_id: &'static str) -> Vec<Entry> {
    let capture = Capture::default();
    let subscriber = Registry::default().with(capture.clone());
    let guard = tracing::subscriber::set_default(subscriber);
    let mut request = Request::get(path).body(Body::empty()).unwrap();
    request
        .headers_mut()
        .insert(REQUEST_ID_HEADER, HeaderValue::from_static(request_id));
    let _response = router().oneshot(request).await.unwrap();
    drop(guard);
    let entries = capture
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    entries
        .into_iter()
        .filter(|entry| entry.message == "request completed")
        .collect()
}

#[tokio::test]
async fn logs_one_access_line_for_a_matched_route() {
    let entries = completed("/items/7?secret=1", "req-1").await;

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(entry.level, Level::INFO);
    assert_eq!(value(&entry.fields, "http.response.status_code"), "200");
    assert!(value(&entry.fields, "duration_ms").parse::<u64>().is_ok());
    assert_eq!(entry.span_name.as_deref(), Some("http.request"));
    assert_eq!(value(&entry.span_fields, "http.request.method"), "GET");
    assert_eq!(value(&entry.span_fields, "http.route"), "/items/{id}");
    assert_eq!(value(&entry.span_fields, "url.path"), "/items/7");
    assert_eq!(value(&entry.span_fields, "request_id"), "req-1");
}

#[tokio::test]
async fn logs_one_access_line_for_an_unmatched_path() {
    let entries = completed("/nowhere", "req-2").await;

    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    assert_eq!(value(&entry.fields, "http.response.status_code"), "404");
    assert_eq!(entry.span_name.as_deref(), Some("http.request"));
    assert_eq!(value(&entry.span_fields, "http.route"), "unmatched");
    assert_eq!(value(&entry.span_fields, "url.path"), "/nowhere");
    assert_eq!(value(&entry.span_fields, "request_id"), "req-2");
}

#[tokio::test]
async fn records_no_request_id_when_the_request_has_none() {
    let capture = Capture::default();
    let guard = tracing::subscriber::set_default(Registry::default().with(capture.clone()));
    let request = Request::get("/items/1").body(Body::empty()).unwrap();
    let _response = Router::new()
        .route("/items/{id}", get(|| std::future::ready("item")))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(make_span)
                .on_response(on_response)
                .on_failure(()),
        )
        .oneshot(request)
        .await
        .unwrap();
    drop(guard);

    let entries = capture
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let entry = entries
        .iter()
        .find(|entry| entry.message == "request completed")
        .unwrap();
    assert_eq!(value(&entry.span_fields, "request_id"), "");
}

#[test]
fn caps_a_huge_latency_at_the_largest_millisecond_count() {
    let capture = Capture::default();
    let guard = tracing::subscriber::set_default(Registry::default().with(capture.clone()));

    on_response(&Response::new(()), Duration::MAX, &Span::none());
    drop(guard);

    let entries = capture
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    assert_eq!(entries.len(), 1);
    assert_eq!(
        value(&entries[0].fields, "duration_ms"),
        u64::MAX.to_string()
    );
}
