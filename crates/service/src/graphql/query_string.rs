//! The GET form of the `GraphQL` path (FR-4): the query string read into the JSON object that the
//! POST form carries, or the problem of the first rule it breaks.
//!
//! The query string is decoded by a strict parser and not by `axum::extract::Query`, whose
//! decoder keeps an invalid `%` sequence and replaces invalid UTF-8; FR-4 answers both with the
//! 400 problem. The rules run in the order of FR-4: the 414 of a query string longer than
//! `HTTP_MAX_BODY_BYTES` as received, then the 400 of a query string that cannot be read, a known
//! parameter given twice or a missing `query`, then the 422 of `variables` or `extensions` that is
//! not a JSON object. No problem and no log entry holds a value of the request; the 400 texts name
//! only the four known parameters (NFR-8).
//!
//! On an HTTP/1.1 connection a query string over 65 521 bytes never reaches these rules: the HTTP
//! server (hyper) refuses a request-target over 65 534 bytes, which `/api/graphql?` and 65 521
//! bytes fill, with its own bare 414 (no body, no `Content-Type`, none of the CORS or `Vary`
//! headers) before the service sees the request, a limit that cannot be configured. So the GET
//! form reads a query string up to the smaller of `HTTP_MAX_BODY_BYTES` and 65 521 bytes, and the
//! 414 problem of this module answers only a query string over `HTTP_MAX_BODY_BYTES` set below
//! 65 521 (maintainer decision M6).

use axum::http::StatusCode;
use serde_json::{Map, Value};
use shared_application::http::{Problem, Violation};

use crate::graphql::schema::ApiSchema;

/// The bound of a GET query string that the service checks, `HTTP_MAX_BODY_BYTES`, set in the
/// schema data by the bootstrap. Above 65 521 bytes the HTTP server refuses the request first on
/// an HTTP/1.1 connection, so the query string read is at most the smaller of the two (decision
/// M6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueryStringLimit {
    /// The longest query string read, in bytes, as received (FR-4), when it is not above the
    /// 65 521 bytes that the HTTP server lets through.
    pub(crate) max_bytes: usize,
}

/// The schema was built without the `QueryStringLimit` data, which bounds the query string of the
/// GET form: a wiring fault that only a test can build.
#[derive(Debug, thiserror::Error)]
#[error("the GraphQL schema has no QueryStringLimit data for the GET form")]
struct MissingQueryStringLimit;

/// The prefix of every 400 text, the wording of the existing query-string rejections.
const UNREADABLE: &str = "Failed to deserialize query string";

/// The parameters of the GET form whose values are JSON objects, in the order they are checked.
const OBJECT_PARAMETERS: [&str; 2] = ["variables", "extensions"];

/// The limit from the schema data; its absence is a wiring fault: the 500 problem, whose error
/// chain is logged once.
pub(crate) fn limit(schema: &ApiSchema) -> Result<QueryStringLimit, Problem> {
    schema
        .data::<QueryStringLimit>()
        .copied()
        .ok_or_else(|| Problem::internal(&MissingQueryStringLimit))
}

/// The GET form (FR-4): the JSON object `{query, operationName, variables, extensions}` that the
/// POST form would carry, with the members that are present, or the problem of the first rule that
/// fails. A request without a query string reads as an empty one.
///
/// The first rule is the 414 problem of a query string longer than `max_bytes`, as received. The
/// query string read is at most the smaller of `HTTP_MAX_BODY_BYTES` and 65 521 bytes: on an
/// HTTP/1.1 connection, a request-target over 65 534 bytes gets the bare 414 of the HTTP server
/// before the service sees it, so this function never reads one (decision M6).
pub(crate) fn read(raw: Option<&str>, max_bytes: usize) -> Result<Map<String, Value>, Problem> {
    let raw = raw.unwrap_or_default();
    if raw.len() > max_bytes {
        return Err(
            Problem::new(StatusCode::URI_TOO_LONG).with_detail("The query string is too long.")
        );
    }
    let parameters = Parameters::parse(raw)?;
    parameters.into_body()
}

/// The values of the four known parameters, decoded.
#[derive(Debug, Default)]
struct Parameters {
    /// `query`, the document.
    query: Option<String>,
    /// `operationName`.
    operation_name: Option<String>,
    /// `variables`, not yet parsed.
    variables: Option<String>,
    /// `extensions`, not yet parsed.
    extensions: Option<String>,
}

impl Parameters {
    /// Reads every piece of `raw` in order: the pieces between `&`, empty ones skipped, each split
    /// at its first `=`. The first piece that cannot be decoded or repeats a known parameter is the
    /// 400 problem; an unknown parameter is ignored once decoded.
    fn parse(raw: &str) -> Result<Self, Problem> {
        let mut parameters = Self::default();
        for piece in raw.split('&').filter(|piece| !piece.is_empty()) {
            let (name, value) = piece.split_once('=').unwrap_or((piece, ""));
            let name = decode(name).ok_or_else(|| undecodable(None))?;
            let slot = parameters.slot(&name);
            let known = slot.as_ref().map(|(known, _)| *known);
            let value = decode(value).ok_or_else(|| undecodable(known))?;
            if let Some((known, slot)) = slot
                && slot.replace(value).is_some()
            {
                return Err(bad_request(&format!("duplicate field `{known}`")));
            }
        }
        Ok(parameters)
    }

    /// The wire name and the value slot of the known parameter `name`; `None` for any other name.
    fn slot(&mut self, name: &str) -> Option<(&'static str, &mut Option<String>)> {
        match name {
            "query" => Some(("query", &mut self.query)),
            "operationName" => Some(("operationName", &mut self.operation_name)),
            "variables" => Some(("variables", &mut self.variables)),
            "extensions" => Some(("extensions", &mut self.extensions)),
            _ => None,
        }
    }

    /// The JSON object of the parameters: the 400 problem without `query`, then the 422 problem of
    /// the first of `variables` and `extensions` that is present and not a JSON object.
    fn into_body(self) -> Result<Map<String, Value>, Problem> {
        let query = self
            .query
            .ok_or_else(|| bad_request("missing field `query`"))?;
        let mut body = Map::new();
        body.insert("query".to_owned(), Value::String(query));
        if let Some(operation_name) = self.operation_name {
            body.insert("operationName".to_owned(), Value::String(operation_name));
        }
        for (name, text) in OBJECT_PARAMETERS
            .into_iter()
            .zip([self.variables, self.extensions])
        {
            if let Some(text) = text {
                body.insert(name.to_owned(), Value::Object(object(name, &text)?));
            }
        }
        Ok(body)
    }
}

/// The JSON object of the parameter `name`, or the 422 problem with its one `format` violation.
fn object(name: &str, text: &str) -> Result<Map<String, Value>, Problem> {
    serde_json::from_str(text).map_err(|_error| {
        Problem::validation_failed(vec![Violation {
            property_path: name.to_owned(),
            message: format!("{name} must be a JSON-encoded object"),
            code: "format".to_owned(),
        }])
    })
}

/// The 400 problem `Failed to deserialize query string: <text>`.
fn bad_request(text: &str) -> Problem {
    Problem::new(StatusCode::BAD_REQUEST).with_detail(format!("{UNREADABLE}: {text}"))
}

/// The 400 problem of a name or a value that cannot be decoded; it names the parameter only when
/// it is one of the four known ones, so an unknown parameter is never echoed.
fn undecodable(known: Option<&str>) -> Problem {
    let text = "invalid percent-encoding or UTF-8";
    bad_request(&known.map_or_else(|| text.to_owned(), |name| format!("{text} in `{name}`")))
}

/// `text` decoded as `application/x-www-form-urlencoded`: `+` is a space, `%` must be followed by
/// two hexadecimal digits, and the bytes must be UTF-8; `None` otherwise.
fn decode(text: &str) -> Option<String> {
    let mut decoded = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let [byte, tail @ ..] = rest {
        rest = match (*byte, tail) {
            (b'+', _) => {
                decoded.push(b' ');
                tail
            }
            (b'%', [high, low, after @ ..]) => {
                decoded.push(hex_pair(*high, *low)?);
                after
            }
            (b'%', _) => return None,
            (other, _) => {
                decoded.push(other);
                tail
            }
        };
    }
    String::from_utf8(decoded).ok()
}

/// The byte written by the two hexadecimal digits `high` and `low`; `None` when either is not one
/// (`u8::from_str_radix` alone would accept a leading `+`).
fn hex_pair(high: u8, low: u8) -> Option<u8> {
    if !(high.is_ascii_hexdigit() && low.is_ascii_hexdigit()) {
        return None;
    }
    let digits = [high, low];
    u8::from_str_radix(str::from_utf8(&digits).ok()?, 16).ok()
}

#[cfg(test)]
mod tests;
