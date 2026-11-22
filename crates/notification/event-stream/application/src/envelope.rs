//! The envelope of a record value and the event object built from it (FR-18, FR-19).

use std::collections::HashSet;
use std::fmt;
use std::ops::Range;

use event_stream_domain::{EventType, Offset, StoredEvent};
use serde::Serialize;
use serde::de::{self, Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use serde_json::Value;
use uuid::Uuid;
use uuid::fmt::Hyphenated;

/// The length of `YYYY-MM-DDTHH:MM:SS`, after which an `occurredAt` may hold a fraction.
const SECONDS_END: usize = 19;

/// The separators of `YYYY-MM-DDTHH:MM:SS` at their byte positions; every other byte before
/// [`SECONDS_END`] is a digit.
const SEPARATORS: [(usize, u8); 5] = [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];

/// The first year of an `occurredAt`.
const FIRST_YEAR: u16 = 1970;

/// Why a record is not an event (FR-19). The text is the `reason` field of the log entry
/// `event record rejected`; it is fixed and never holds a member value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RejectReason {
    /// The record has no value.
    #[error("missing value")]
    Missing,
    /// The value is larger than `EVENT_MAX_DATA_BYTES`.
    #[error("value too large")]
    TooLarge,
    /// The value is not UTF-8, not JSON or not an object, or one of the four members is missing,
    /// has the wrong JSON type, or a member name occurs twice.
    #[error("not a JSON object with the envelope members")]
    Shape,
    /// `id` is not a UUID in the 36-character hyphenated form.
    #[error("invalid id")]
    Id,
    /// `type` breaks the rules of event type names.
    #[error("invalid type")]
    Type,
    /// `occurredAt` is not an RFC 3339 time in UTC from 1970 to 9999 ending with `Z` or `+00:00`.
    #[error("invalid occurredAt")]
    OccurredAt,
}

/// Turns the value of the record at `offset` into an event (FR-18), or names why it is not one
/// (FR-19).
///
/// The size is checked first, before the value is read; then the value must be UTF-8 and a JSON
/// object holding each of `id`, `type`, `occurredAt` and `data` once, with no member name twice
/// (other members are ignored); then `id` must be a hyphenated UUID in either case, `type` an event
/// type name and `occurredAt` a UTC time. The event object is written once by `serde_json`:
/// `{"id":..,"type":..,"occurredAt":..,"data":..}` in this order, the id in lower case,
/// `occurredAt` as written and `data` without white space outside strings.
///
/// # Errors
///
/// Returns the [`RejectReason`] of the first rule the value breaks, in the order above.
pub fn parse_event(
    offset: Offset,
    value: Option<&[u8]>,
    max_bytes: usize,
) -> Result<StoredEvent, RejectReason> {
    let value = value.ok_or(RejectReason::Missing)?;
    if value.len() > max_bytes {
        return Err(RejectReason::TooLarge);
    }
    let text = str::from_utf8(value).map_err(|_error| RejectReason::Shape)?;
    let envelope = serde_json::from_str::<Envelope>(text).map_err(|_error| RejectReason::Shape)?;
    envelope.into_event(offset)
}

/// The four members of a record value as they were read, before their rules are checked.
struct Envelope {
    id: String,
    r#type: String,
    occurred_at: String,
    data: Value,
}

impl Envelope {
    /// Checks the rules of `id`, `type` and `occurredAt` and writes the event object.
    fn into_event(self, offset: Offset) -> Result<StoredEvent, RejectReason> {
        let id = parse_id(&self.id)?;
        let event_type = EventType::new(self.r#type).map_err(|_error| RejectReason::Type)?;
        if !is_utc_time(&self.occurred_at) {
            return Err(RejectReason::OccurredAt);
        }
        let mut buffer = Uuid::encode_buffer();
        let object = EventObject {
            id: id.hyphenated().encode_lower(&mut buffer),
            r#type: event_type.as_str(),
            occurred_at: &self.occurred_at,
            data: &self.data,
        };
        // A struct of strings and a JSON value always serialises; the reason is never returned.
        let json = serde_json::to_string(&object)
            .ok()
            .ok_or(RejectReason::Shape)?;
        Ok(StoredEvent {
            sequence: offset.sequence(),
            id,
            event_type,
            json,
        })
    }
}

impl<'de> Deserialize<'de> for Envelope {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(EnvelopeVisitor)
    }
}

/// Reads the envelope object member by member, so that a member name that occurs twice is refused
/// whether or not it is one of the four (FR-18); a derived reader refuses only the four.
struct EnvelopeVisitor;

impl<'de> Visitor<'de> for EnvelopeVisitor {
    type Value = Envelope;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an envelope object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Envelope, A::Error> {
        let mut members = Members::default();
        let mut names = HashSet::new();
        while let Some(name) = map.next_key::<String>()? {
            members.read(&name, &mut map)?;
            if !names.insert(name) {
                return Err(de::Error::custom("a member name occurs twice"));
            }
        }
        members.into_envelope()
    }
}

/// The four members met so far.
#[derive(Default)]
struct Members {
    id: Option<String>,
    r#type: Option<String>,
    occurred_at: Option<String>,
    data: Option<Value>,
}

impl Members {
    /// Reads the value of the member `name`; the value of any other member is skipped.
    fn read<'de, A: MapAccess<'de>>(&mut self, name: &str, map: &mut A) -> Result<(), A::Error> {
        match name {
            "id" => self.id = Some(map.next_value()?),
            "type" => self.r#type = Some(map.next_value()?),
            "occurredAt" => self.occurred_at = Some(map.next_value()?),
            "data" => self.data = Some(map.next_value()?),
            _ => {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(())
    }

    /// The envelope, once each of the four members was met.
    fn into_envelope<E: de::Error>(self) -> Result<Envelope, E> {
        Ok(Envelope {
            id: self.id.ok_or_else(|| E::missing_field("id"))?,
            r#type: self.r#type.ok_or_else(|| E::missing_field("type"))?,
            occurred_at: self
                .occurred_at
                .ok_or_else(|| E::missing_field("occurredAt"))?,
            data: self.data.ok_or_else(|| E::missing_field("data"))?,
        })
    }
}

/// The event object of FR-18; `serde_json` writes the fields in this order.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EventObject<'a> {
    id: &'a str,
    r#type: &'a str,
    occurred_at: &'a str,
    data: &'a Value,
}

/// A UUID in the 36-character hyphenated form, letters in either case. `Uuid::try_parse` also
/// reads the simple, braced and URN forms, which have other lengths.
fn parse_id(text: &str) -> Result<Uuid, RejectReason> {
    if text.len() != Hyphenated::LENGTH {
        return Err(RejectReason::Id);
    }
    Uuid::try_parse(text).map_err(|_error| RejectReason::Id)
}

/// An RFC 3339 time in UTC: `YYYY-MM-DDTHH:MM:SS` under the rule of [`is_calendar_time`], then
/// nothing or a dot and one or more digits (any number of them), then `Z` or `+00:00`. Any other
/// text between the seconds and the suffix, a dot without digits and any other offset are refused.
fn is_utc_time(text: &str) -> bool {
    let Some(body) = text
        .strip_suffix('Z')
        .or_else(|| text.strip_suffix("+00:00"))
    else {
        return false;
    };
    let fraction_valid = match body.as_bytes().get(SECONDS_END..) {
        Some([]) => true,
        Some([b'.', digits @ ..]) => !digits.is_empty() && digits.iter().all(u8::is_ascii_digit),
        _ => false,
    };
    fraction_valid && is_calendar_time(body.as_bytes())
}

/// `YYYY-MM-DDTHH:MM:SS` from 1970 to 9999: digits and separators at their positions, a day of
/// the calendar (Gregorian leap years), hour 0 to 23, minute 0 to 59, second 0 to 60 (a leap
/// second is accepted at any minute, as before).
fn is_calendar_time(body: &[u8]) -> bool {
    let separated = SEPARATORS
        .iter()
        .all(|&(position, separator)| body.get(position) == Some(&separator));
    let date = digits(body, 0..4)
        .zip(digits(body, 5..7))
        .zip(digits(body, 8..10));
    let time = digits(body, 11..13)
        .zip(digits(body, 14..16))
        .zip(digits(body, 17..SECONDS_END));
    separated
        && date.is_some_and(|((year, month), day)| {
            year >= FIRST_YEAR && (1..=days_in_month(year, month)).contains(&day)
        })
        && time.is_some_and(|((hour, minute), second)| hour <= 23 && minute <= 59 && second <= 60)
}

/// The number that the digits at `range` of `body` write; `None` when a byte there is not an
/// ASCII digit (a sign, a letter, a byte of a wider character) or `range` lies outside `body`.
/// A field holds at most four digits, so the number stays at most 9999 and no step saturates.
fn digits(body: &[u8], range: Range<usize>) -> Option<u16> {
    body.get(range)?.iter().try_fold(0, |number: u16, &byte| {
        byte.is_ascii_digit().then(|| {
            number
                .saturating_mul(10)
                .saturating_add(u16::from(byte.saturating_sub(b'0')))
        })
    })
}

/// The days of `month` in `year`; 0 for a month outside 1 to 12.
const fn days_in_month(year: u16, month: u16) -> u16 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Divisible by 4 and not by 100, or divisible by 400.
const fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

#[cfg(test)]
mod tests;
