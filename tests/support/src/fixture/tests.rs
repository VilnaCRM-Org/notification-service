use some_module_domain::{ExampleApiResourceDescription, ExampleApiResourceName};

use super::{example_api_resource, example_description, example_name};
use crate::seed::TestRng;

#[test]
fn the_same_seed_gives_equal_names_and_descriptions() {
    let first = (
        example_name(&mut TestRng::from_seed(7)),
        example_description(&mut TestRng::from_seed(7)),
    );
    let second = (
        example_name(&mut TestRng::from_seed(7)),
        example_description(&mut TestRng::from_seed(7)),
    );

    assert_eq!(first, second);
}

#[test]
fn the_same_seed_gives_equal_resources() {
    let first = example_api_resource(&mut TestRng::from_seed(11));
    let second = example_api_resource(&mut TestRng::from_seed(11));

    assert_eq!(first.id(), second.id());
    assert_eq!(first.name(), second.name());
    assert_eq!(first.description(), second.description());
}

#[test]
fn different_seeds_give_different_resources() {
    let first = example_api_resource(&mut TestRng::from_seed(1));
    let second = example_api_resource(&mut TestRng::from_seed(2));

    assert_ne!(first.id(), second.id());
}

#[test]
fn generated_values_satisfy_the_invariants() {
    let mut rng = TestRng::from_seed(20_261_110);

    for _round in 0..200 {
        let resource = example_api_resource(&mut rng);
        let name = resource.name().as_str();
        let description = resource.description().as_str();

        assert!(ExampleApiResourceName::new(name.to_owned()).is_ok());
        assert!(ExampleApiResourceDescription::new(description.to_owned()).is_ok());
        assert_eq!(resource.id().value().as_uuid().get_version_num(), 7);
    }
}
