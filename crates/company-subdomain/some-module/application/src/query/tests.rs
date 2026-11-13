use shared_domain::Uuid;
use some_module_domain::{
    ExampleApiResource, ExampleApiResourceDescription, ExampleApiResourceId, ExampleApiResourceName,
};

use super::ExampleApiResourceView;

#[test]
fn a_view_copies_the_fields_of_the_aggregate() {
    let id = Uuid::parse("0192f3a0-7b1c-7d2e-8f3a-4b5c6d7e8f90").unwrap();
    let resource = ExampleApiResource::restore(
        ExampleApiResourceId::new(id),
        ExampleApiResourceName::new("Name".to_owned()).unwrap(),
        ExampleApiResourceDescription::new("Description".to_owned()).unwrap(),
    );

    assert_eq!(
        ExampleApiResourceView::from(&resource),
        ExampleApiResourceView {
            id,
            name: "Name".to_owned(),
            description: "Description".to_owned(),
        }
    );
}
