//! Tests of the envelope parser: the event object of FR-18 and the reject reasons of FR-19.

use event_stream_domain::{EventType, Offset, Sequence, StoredEvent};
use rstest::rstest;
use serde::Serialize;
use serde::de::Expected;
use uuid::Uuid;

use super::{EnvelopeVisitor, RejectReason, parse_event};

const MAX_BYTES: usize = 65_536;
const ID: &str = "8F14E45F-CEEA-467F-A8F4-3D1B5C4E1A2B";
const LOWER_ID: &str = "8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b";
const TYPE: &str = "user.created";
const AT: &str = "2026-11-14T10:00:00Z";

/// The event object written by `serde_json` from a struct whose fields are in the order of FR-18.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedEvent<'a, D> {
    id: &'a str,
    r#type: &'a str,
    occurred_at: &'a str,
    data: D,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserData<'a> {
    user_id: &'a str,
}

/// A record value with the four members, in this order, written compactly.
fn envelope(id: &str, event_type: &str, occurred_at: &str, data: &str) -> String {
    format!(r#"{{"id":"{id}","type":"{event_type}","occurredAt":"{occurred_at}","data":{data}}}"#)
}

fn parse(value: &str) -> Result<StoredEvent, RejectReason> {
    parse_event(Offset::new(7), Some(value.as_bytes()), MAX_BYTES)
}

#[test]
fn a_valid_value_becomes_the_event_object_with_the_sequence_after_its_offset() {
    let value = r#"{"id":"8F14E45F-CEEA-467F-A8F4-3D1B5C4E1A2B","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"userId":"5f2c"},"extra":1}"#;

    let event = parse_event(Offset::new(1042), Some(value.as_bytes()), MAX_BYTES).unwrap();

    let expected = serde_json::to_string(&ExpectedEvent {
        id: LOWER_ID,
        r#type: TYPE,
        occurred_at: AT,
        data: UserData { user_id: "5f2c" },
    })
    .unwrap();
    assert_eq!(
        expected,
        r#"{"id":"8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b","type":"user.created","occurredAt":"2026-11-14T10:00:00Z","data":{"userId":"5f2c"}}"#
    );
    assert_eq!(
        event,
        StoredEvent {
            sequence: Sequence::new(1043).unwrap(),
            id: Uuid::try_parse(LOWER_ID).unwrap(),
            event_type: EventType::new(TYPE.to_owned()).unwrap(),
            json: expected,
        }
    );
}

#[test]
fn the_record_at_offset_zero_has_sequence_one() {
    let event = parse_event(
        Offset::new(0),
        Some(envelope(ID, TYPE, AT, "1").as_bytes()),
        MAX_BYTES,
    )
    .unwrap();

    assert_eq!(event.sequence, Sequence::new(1).unwrap());
}

#[test]
fn a_lower_case_id_is_accepted_and_kept() {
    let event = parse(&envelope(LOWER_ID, TYPE, AT, "1")).unwrap();

    assert_eq!(event.id, Uuid::try_parse(ID).unwrap());
    assert_eq!(event.json, envelope(LOWER_ID, TYPE, AT, "1"));
}

#[test]
fn white_space_around_the_members_is_removed_and_their_order_does_not_matter() {
    let value = format!(
        " {{ \"data\" : 1 ,\n \"occurredAt\" : \"{AT}\" , \"type\" : \"{TYPE}\" , \"id\" : \"{ID}\" }} "
    );

    assert_eq!(
        parse(&value).unwrap().json,
        envelope(LOWER_ID, TYPE, AT, "1")
    );
}

#[test]
fn a_member_name_written_with_an_escape_is_read_as_the_name_it_stands_for() {
    let value = format!(r#"{{"\u0069d":"{ID}","type":"{TYPE}","occurredAt":"{AT}","data":1}}"#);

    assert_eq!(
        parse(&value).unwrap().json,
        envelope(LOWER_ID, TYPE, AT, "1")
    );
}

#[rstest]
#[case::object(r#"{ "a" : 1 , "b" : [ true , null ] }"#, r#"{"a":1,"b":[true,null]}"#)]
#[case::empty_object("{ }", "{}")]
#[case::array(r#"[ 1 , "x y" , { "c" : "d" } ]"#, r#"[1,"x y",{"c":"d"}]"#)]
#[case::empty_array("[ ]", "[]")]
#[case::string(r#" "a \"quoted\" \\ text\n" "#, r#""a \"quoted\" \\ text\n""#)]
#[case::integer(" 42 ", "42")]
#[case::negative_decimal(" -12.5 ", "-12.5")]
#[case::true_value(" true ", "true")]
#[case::false_value(" false ", "false")]
#[case::null_value(" null ", "null")]
fn data_of_every_json_type_is_forwarded_equal_without_white_space_outside_strings(
    #[case] data: &str,
    #[case] forwarded: &str,
) {
    let event = parse(&envelope(ID, TYPE, AT, data)).unwrap();

    assert_eq!(event.json, envelope(LOWER_ID, TYPE, AT, forwarded));
}

#[test]
fn a_value_of_exactly_max_bytes_is_judged_by_the_other_rules() {
    let valid = envelope(ID, TYPE, AT, "1");
    let invalid = envelope(ID, "Bad", AT, "1");

    let accepted = parse_event(Offset::new(3), Some(valid.as_bytes()), valid.len());
    let rejected = parse_event(Offset::new(3), Some(invalid.as_bytes()), invalid.len());

    assert_eq!(accepted.unwrap().json, envelope(LOWER_ID, TYPE, AT, "1"));
    assert_eq!(rejected, Err(RejectReason::Type));
}

#[test]
fn a_value_one_byte_above_max_bytes_is_rejected_for_its_size_before_it_is_parsed() {
    let valid = envelope(ID, TYPE, AT, "1");
    let valid_with_a_trailing_space = format!("{valid} ");
    let not_utf_8 = [0xff_u8; 11];

    let too_large = parse_event(
        Offset::new(3),
        Some(valid_with_a_trailing_space.as_bytes()),
        valid.len(),
    );
    let unparsed = parse_event(Offset::new(3), Some(&not_utf_8), 10);

    assert_eq!(too_large, Err(RejectReason::TooLarge));
    assert_eq!(unparsed, Err(RejectReason::TooLarge));
}

#[test]
fn a_missing_value_is_rejected_before_its_size_is_checked() {
    assert_eq!(
        parse_event(Offset::new(3), None, MAX_BYTES),
        Err(RejectReason::Missing)
    );
    assert_eq!(
        parse_event(Offset::new(3), None, 0),
        Err(RejectReason::Missing)
    );
}

#[test]
fn a_value_that_is_not_utf_8_is_rejected_even_inside_a_member_that_is_ignored() {
    let mut value = envelope(ID, TYPE, AT, "1").into_bytes();
    value.pop();
    value.extend_from_slice(br#","extra":"x"#);
    value.push(0xff);
    value.extend_from_slice(br#""}"#);

    assert_eq!(
        parse_event(Offset::new(3), Some(&value), MAX_BYTES),
        Err(RejectReason::Shape)
    );
}

#[rstest]
#[case::empty("")]
#[case::not_json("not json")]
#[case::truncated(r#"{"id":"8F14E45F-CEEA-467F-A8F4-3D1B5C4E1A2B","type":"user.created""#)]
#[case::trailing_characters(&format!("{} x", envelope(ID, TYPE, AT, "1")))]
#[case::array(&format!("[{}]", envelope(ID, TYPE, AT, "1")))]
#[case::string(r#""text""#)]
#[case::number("42")]
#[case::null("null")]
#[case::duplicate_id(&format!(r#"{{"id":"{ID}","id":"{ID}","type":"{TYPE}","occurredAt":"{AT}","data":1}}"#))]
#[case::duplicate_escaped_name(&format!(r#"{{"id":"{ID}","\u0069d":"{ID}","type":"{TYPE}","occurredAt":"{AT}","data":1}}"#))]
#[case::duplicate_type(&format!(r#"{{"id":"{ID}","type":"{TYPE}","type":"{TYPE}","occurredAt":"{AT}","data":1}}"#))]
#[case::duplicate_occurred_at(&format!(r#"{{"id":"{ID}","type":"{TYPE}","occurredAt":"{AT}","occurredAt":"{AT}","data":1}}"#))]
#[case::duplicate_data(&format!(r#"{{"id":"{ID}","type":"{TYPE}","occurredAt":"{AT}","data":1,"data":1}}"#))]
#[case::duplicate_ignored_member(&format!(r#"{{"x":1,"id":"{ID}","type":"{TYPE}","occurredAt":"{AT}","data":1,"x":2}}"#))]
#[case::missing_id(&format!(r#"{{"type":"{TYPE}","occurredAt":"{AT}","data":1}}"#))]
#[case::number_id(&format!(r#"{{"id":42,"type":"{TYPE}","occurredAt":"{AT}","data":1}}"#))]
#[case::null_id(&format!(r#"{{"id":null,"type":"{TYPE}","occurredAt":"{AT}","data":1}}"#))]
#[case::missing_type(&format!(r#"{{"id":"{ID}","occurredAt":"{AT}","data":1}}"#))]
#[case::array_type(&format!(r#"{{"id":"{ID}","type":["{TYPE}"],"occurredAt":"{AT}","data":1}}"#))]
#[case::missing_occurred_at(&format!(r#"{{"id":"{ID}","type":"{TYPE}","data":1}}"#))]
#[case::number_occurred_at(&format!(r#"{{"id":"{ID}","type":"{TYPE}","occurredAt":0,"data":1}}"#))]
#[case::missing_data(&format!(r#"{{"id":"{ID}","type":"{TYPE}","occurredAt":"{AT}"}}"#))]
#[case::nothing_but_ignored_members(r#"{"extra":1}"#)]
fn a_value_that_is_not_an_object_with_the_four_members_once_is_rejected_as_its_shape(
    #[case] value: &str,
) {
    assert_eq!(parse(value), Err(RejectReason::Shape));
}

#[rstest]
#[case::simple("8f14e45fceea467fa8f43d1b5c4e1a2b")]
#[case::braced("{8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b}")]
#[case::urn("urn:uuid:8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2b")]
#[case::not_hexadecimal("8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2g")]
#[case::hyphen_misplaced("8f14e45fc-eea-467f-a8f4-3d1b5c4e1a2b")]
#[case::too_short("8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2")]
#[case::too_long("8f14e45f-ceea-467f-a8f4-3d1b5c4e1a2bb")]
#[case::empty("")]
fn an_id_that_is_not_a_hyphenated_uuid_is_rejected_as_the_id(#[case] id: &str) {
    assert_eq!(parse(&envelope(id, TYPE, AT, "1")), Err(RejectReason::Id));
}

#[rstest]
#[case::empty("")]
#[case::upper_case("User.Created")]
#[case::separator_at_the_end("user.")]
#[case::digit_first("1user")]
#[case::too_long(&"a".repeat(129))]
fn a_type_that_breaks_the_name_rules_is_rejected_as_the_type(#[case] event_type: &str) {
    assert_eq!(
        parse(&envelope(ID, event_type, AT, "1")),
        Err(RejectReason::Type)
    );
}

#[rstest]
#[case::other_offset("2026-11-14T10:00:00+01:00")]
#[case::negative_zero_offset("2026-11-14T10:00:00-00:00")]
#[case::no_suffix("2026-11-14T10:00:00")]
#[case::lower_case_suffix("2026-11-14T10:00:00z")]
#[case::lower_case_separator("2026-11-14t10:00:00Z")]
#[case::space_separator("2026-11-14 10:00:00Z")]
#[case::year_1969("1969-12-31T23:59:59Z")]
#[case::month_13("2026-13-14T10:00:00Z")]
#[case::day_32("2026-11-32T10:00:00Z")]
#[case::hour_24("2026-11-14T24:00:00Z")]
#[case::date_only("2026-11-14Z")]
#[case::empty("")]
#[case::suffix_only("Z")]
#[case::dot_without_digits("2026-11-14T10:00:00.Z")]
#[case::letter_in_the_fraction("2026-11-14T10:00:00.1x+00:00")]
#[case::text_between_seconds_and_suffix("2026-11-14T10:00:00ZabcZ")]
#[case::quote_between_seconds_and_suffix(r#"2026-11-14T10:00:00Z\"Z"#)]
#[case::offset_text_before_the_suffix("2026-11-14T10:00:00.1+abcdZ")]
#[case::both_suffixes("2026-11-14T10:00:00+00:00Z")]
fn an_occurred_at_that_is_not_a_utc_rfc_3339_time_from_1970_to_9999_is_rejected(
    #[case] occurred_at: &str,
) {
    assert_eq!(
        parse(&envelope(ID, TYPE, occurred_at, "1")),
        Err(RejectReason::OccurredAt)
    );
}

#[rstest]
#[case::first_second_of_1970("1970-01-01T00:00:00Z")]
#[case::last_second_of_9999("9999-12-31T23:59:59Z")]
#[case::plus_zero_offset("2026-11-14T10:00:00+00:00")]
#[case::fraction_with_z("2026-11-14T10:00:00.123Z")]
#[case::fraction_with_plus_zero_offset("2026-11-14T10:00:00.5+00:00")]
fn an_occurred_at_in_utc_from_1970_to_9999_is_forwarded_as_written(#[case] occurred_at: &str) {
    let event = parse(&envelope(ID, TYPE, occurred_at, "1")).unwrap();

    assert_eq!(event.json, envelope(LOWER_ID, TYPE, occurred_at, "1"));
}

#[rstest]
#[case::missing(RejectReason::Missing, "missing value")]
#[case::too_large(RejectReason::TooLarge, "value too large")]
#[case::shape(RejectReason::Shape, "not a JSON object with the envelope members")]
#[case::id(RejectReason::Id, "invalid id")]
#[case::event_type(RejectReason::Type, "invalid type")]
#[case::occurred_at(RejectReason::OccurredAt, "invalid occurredAt")]
fn every_reason_is_a_fixed_text_that_holds_no_member_value(
    #[case] reason: RejectReason,
    #[case] text: &str,
) {
    assert_eq!(reason.to_string(), text);
}

#[test]
fn the_reader_of_the_envelope_names_what_it_expects() {
    let expected: &dyn Expected = &EnvelopeVisitor;

    assert_eq!(expected.to_string(), "an envelope object");
}
