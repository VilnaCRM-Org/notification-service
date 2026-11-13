use super::DuplicateHandler;

#[test]
fn a_duplicate_handler_names_the_message() {
    let error = DuplicateHandler { message: "Rename" };

    assert_eq!(
        error.to_string(),
        "a handler is already registered for `Rename`"
    );
}
