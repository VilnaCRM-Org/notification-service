use rstest::rstest;

use super::{InvalidPageRequest, PageRequest};

#[rstest]
#[case::first_page(1, 1, 0)]
#[case::defaults(PageRequest::DEFAULT_PAGE, PageRequest::DEFAULT_ITEMS_PER_PAGE, 0)]
#[case::largest_page_size(7, PageRequest::MAX_ITEMS_PER_PAGE, 600)]
#[case::largest_page(u32::MAX, 100, 429_496_729_400)]
fn accepts_a_page_from_1_and_items_per_page_from_1_to_100(
    #[case] page: u32,
    #[case] items_per_page: u32,
    #[case] offset: i64,
) {
    let request = PageRequest::new(page, items_per_page).unwrap();

    assert_eq!(request.limit(), i64::from(items_per_page));
    assert_eq!(request.offset(), offset);
}

#[rstest]
#[case::page_zero(0, 30)]
#[case::page_zero_and_bad_size(0, 0)]
fn rejects_page_zero_before_the_size(#[case] page: u32, #[case] items_per_page: u32) {
    assert_eq!(
        PageRequest::new(page, items_per_page),
        Err(InvalidPageRequest::Page)
    );
}

#[rstest]
#[case::zero(0)]
#[case::above_the_maximum(101)]
#[case::far_above_the_maximum(u32::MAX)]
fn rejects_items_per_page_outside_1_to_100(#[case] items_per_page: u32) {
    assert_eq!(
        PageRequest::new(1, items_per_page),
        Err(InvalidPageRequest::ItemsPerPage)
    );
}

#[test]
fn states_the_violated_bound_in_the_message() {
    assert_eq!(
        InvalidPageRequest::Page.to_string(),
        "page must be 1 or greater"
    );
    assert_eq!(
        InvalidPageRequest::ItemsPerPage.to_string(),
        "itemsPerPage must be between 1 and 100"
    );
}

#[test]
fn exposes_the_documented_defaults() {
    assert_eq!(PageRequest::DEFAULT_PAGE, 1);
    assert_eq!(PageRequest::DEFAULT_ITEMS_PER_PAGE, 30);
    assert_eq!(PageRequest::MAX_ITEMS_PER_PAGE, 100);
}

#[rstest]
#[case::first_page(1, 30, 0)]
#[case::second_page(2, 30, 30)]
#[case::third_page_of_ten(3, 10, 20)]
#[case::deep_page(1_000, 100, 99_900)]
#[case::largest_page(u32::MAX, 100, 429_496_729_400)]
fn computes_the_offset_in_i64(#[case] page: u32, #[case] items_per_page: u32, #[case] offset: i64) {
    let request = PageRequest::new(page, items_per_page).unwrap();

    assert_eq!(request.offset(), offset);
}

#[rstest]
#[case(1)]
#[case(30)]
#[case(100)]
fn limit_is_the_items_per_page(#[case] items_per_page: u32) {
    let request = PageRequest::new(5, items_per_page).unwrap();

    assert_eq!(request.limit(), i64::from(items_per_page));
}
