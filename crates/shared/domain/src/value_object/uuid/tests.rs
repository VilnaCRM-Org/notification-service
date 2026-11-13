use proptest::prelude::*;
use rstest::rstest;

use super::{InvalidUuid, Uuid};

const NIL: &str = "00000000-0000-0000-0000-000000000000";
const V4: &str = "67e55044-10b1-426f-9247-bb680e5fe0c8";
const V7: &str = "0192f3c4-7a1b-7c3d-8e5f-0123456789ab";

#[rstest]
#[case::nil(NIL)]
#[case::version_4(V4)]
#[case::version_7(V7)]
#[case::upper_case("67E55044-10B1-426F-9247-BB680E5FE0C8")]
#[case::simple_form("67e5504410b1426f9247bb680e5fe0c8")]
#[case::braced("{67e55044-10b1-426f-9247-bb680e5fe0c8}")]
#[case::urn("urn:uuid:67e55044-10b1-426f-9247-bb680e5fe0c8")]
#[case::unknown_version("ffffffff-ffff-ffff-ffff-ffffffffffff")]
fn accepts_every_syntactically_valid_uuid(#[case] text: &str) {
    let parsed = Uuid::parse(text).unwrap();

    assert_eq!(parsed.as_uuid(), uuid::Uuid::try_parse(text).unwrap());
}

#[rstest]
#[case::empty("")]
#[case::text("not-a-uuid")]
#[case::too_short("67e55044-10b1-426f-9247-bb680e5fe0c")]
#[case::too_long("67e55044-10b1-426f-9247-bb680e5fe0c80")]
#[case::bad_digit("g7e55044-10b1-426f-9247-bb680e5fe0c8")]
#[case::surrounding_space(" 67e55044-10b1-426f-9247-bb680e5fe0c8")]
fn rejects_other_input_and_carries_it(#[case] text: &str) {
    let error = Uuid::parse(text).unwrap_err();

    assert_eq!(
        error,
        InvalidUuid {
            value: text.to_owned()
        }
    );
}

#[test]
fn names_the_rejected_input_in_the_message() {
    let error = Uuid::parse("nope").unwrap_err();

    assert_eq!(error.to_string(), "`nope` is not a valid UUID");
}

#[test]
fn displays_the_lower_case_hyphenated_form() {
    let parsed = Uuid::parse("67E55044-10B1-426F-9247-BB680E5FE0C8").unwrap();

    assert_eq!(parsed.to_string(), V4);
}

#[test]
fn wraps_and_unwraps_a_uuid() {
    let inner = uuid::Uuid::from_u128(42);

    assert_eq!(Uuid::from_uuid(inner).as_uuid(), inner);
}

#[test]
fn orders_and_compares_by_the_wrapped_value() {
    let low = Uuid::from_uuid(uuid::Uuid::from_u128(1));
    let high = Uuid::from_uuid(uuid::Uuid::from_u128(2));

    assert!(low < high);
    assert_ne!(low, high);
    assert_eq!(low, low);
}

proptest! {
    #[test]
    fn round_trips_through_its_text_form(bits in any::<u128>()) {
        let generated = Uuid::from_uuid(uuid::Uuid::from_u128(bits));

        prop_assert_eq!(Uuid::parse(&generated.to_string()), Ok(generated));
    }
}
