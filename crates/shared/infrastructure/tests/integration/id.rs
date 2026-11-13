use shared_domain::UuidGenerator;
use shared_infrastructure::id::UuidV7Generator;

#[test]
fn generated_identifiers_are_distinct_version_7_and_ordered() {
    let generator: &dyn UuidGenerator = &UuidV7Generator;

    let ids: Vec<_> = (0..1000).map(|_| generator.generate()).collect();

    assert!(ids.iter().all(|id| id.as_uuid().get_version_num() == 7));
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
}
