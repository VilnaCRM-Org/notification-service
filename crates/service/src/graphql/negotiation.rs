//! The transport a request to the `GraphQL` path asks for (FR-2).
//!
//! `Accept` is read as media ranges with weights, not compared as one string: every field of the
//! request, joined in order, split at the commas, each range split at the semicolons. Weights are
//! counted in integer thousandths, so no float is compared.

use axum::http::HeaderMap;
use axum::http::header::ACCEPT;

/// The weight of a range without a `q` parameter, in thousandths.
const FULL_WEIGHT: u16 = 1000;

/// True when `Accept` ranks `text/event-stream` above both JSON types (FR-2, POST rule).
///
/// Wildcard ranges count for neither side, so no client that sends `*/*` changes transport. The
/// answer is `s > j`: `j` is never negative, so `s > 0` follows and needs no test of its own.
pub(crate) fn negotiates_event_stream(headers: &HeaderMap) -> bool {
    let ranks = ranks(headers);
    ranks.event_stream > ranks.json
}

/// The highest weight, in thousandths, of each kind of range that the rule compares; a kind that
/// is not listed weighs 0.
#[derive(Debug, Default, Clone, Copy)]
struct Ranks {
    /// `text/event-stream`.
    event_stream: u16,
    /// `application/json` or `application/graphql-response+json`.
    json: u16,
}

/// The kind of a media range.
#[derive(Debug, Clone, Copy)]
enum Kind {
    /// `text/event-stream`.
    EventStream,
    /// One of the two JSON types.
    Json,
    /// Any other range, wildcards included.
    Other,
}

impl Ranks {
    /// The ranks once a range of `kind` with `weight` is read.
    fn with(self, kind: Kind, weight: u16) -> Self {
        match kind {
            Kind::EventStream => Self {
                event_stream: self.event_stream.max(weight),
                ..self
            },
            Kind::Json => Self {
                json: self.json.max(weight),
                ..self
            },
            Kind::Other => self,
        }
    }
}

/// The ranks of every `Accept` field; a field that is not visible ASCII text counts as absent.
fn ranks(headers: &HeaderMap) -> Ranks {
    headers
        .get_all(ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(range)
        .fold(Ranks::default(), |ranks, (kind, weight)| {
            ranks.with(kind, weight)
        })
}

/// The kind and the weight of one media range; `None` when its `q` is not a valid qvalue, which
/// ignores the range. Parameters other than `q` are ignored.
fn range(text: &str) -> Option<(Kind, u16)> {
    let mut parts = text.split(';');
    let kind = kind(parts.next().unwrap_or_default().trim());
    let weight = parts
        .map(|parameter| parameter.split_once('=').unwrap_or((parameter, "")))
        .find(|(name, _value)| name.trim().eq_ignore_ascii_case("q"))
        .map_or(Some(FULL_WEIGHT), |(_name, value)| qvalue(value.trim()))?;
    Some((kind, weight))
}

/// The kind of `media`, compared without regard to letter case.
const fn kind(media: &str) -> Kind {
    if media.eq_ignore_ascii_case("text/event-stream") {
        Kind::EventStream
    } else if media.eq_ignore_ascii_case("application/json")
        || media.eq_ignore_ascii_case("application/graphql-response+json")
    {
        Kind::Json
    } else {
        Kind::Other
    }
}

/// An RFC 9110 qvalue in thousandths, 0 to 1000: `0` with at most three decimals, or `1` whose
/// decimals are zeros.
fn qvalue(text: &str) -> Option<u16> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if fraction.len() > 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let thousandths: u16 = format!("{fraction:0<3}").parse().ok()?;
    match whole {
        "0" => Some(thousandths),
        "1" if thousandths == 0 => Some(FULL_WEIGHT),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
