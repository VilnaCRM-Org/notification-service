use event_stream_domain::EventType;

use super::EventTypeView;

#[test]
fn a_view_carries_the_name() {
    let view = EventTypeView::from(EventType::new("user.created".to_owned()).unwrap());

    assert_eq!(
        view,
        EventTypeView {
            name: "user.created".to_owned()
        }
    );
}
