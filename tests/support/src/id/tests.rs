use shared_domain::UuidGenerator;

use super::SequenceUuidGenerator;

#[test]
fn generates_distinct_version_7_uuids_in_increasing_order() {
    let generator: &dyn UuidGenerator = &SequenceUuidGenerator::starting_at(1);

    let ids: Vec<_> = (0..5).map(|_index| generator.generate()).collect();

    for id in &ids {
        assert_eq!(id.as_uuid().get_version_num(), 7);
        assert_eq!(id.as_uuid().get_variant(), uuid::Variant::RFC4122);
    }
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn the_counter_is_the_low_bits_of_the_first_uuids() {
    let generator = SequenceUuidGenerator::starting_at(41);

    assert_eq!(
        generator.generate().to_string(),
        "00000000-0000-7000-8000-000000000029"
    );
    assert_eq!(
        generator.generate().to_string(),
        "00000000-0000-7000-8000-00000000002a"
    );
}

#[test]
fn a_huge_counter_keeps_the_version_and_the_variant() {
    let generator = SequenceUuidGenerator::starting_at(u64::MAX);

    let id = generator.generate().as_uuid();

    assert_eq!(id.get_version_num(), 7);
    assert_eq!(id.get_variant(), uuid::Variant::RFC4122);
}
