use rstest::rstest;

use super::PageLimit;

const LIMIT: PageLimit = PageLimit {
    rows: 500,
    bytes: 65_536,
};

#[rstest]
#[case::both_below(499, 65_535, false)]
#[case::rows_reached(500, 0, true)]
#[case::rows_above(501, 0, true)]
#[case::bytes_reached(0, 65_536, true)]
#[case::bytes_above(0, 65_537, true)]
#[case::both_reached(500, 65_536, true)]
#[case::nothing(0, 0, false)]
fn is_reached_by_rows_or_by_bytes(
    #[case] rows: usize,
    #[case] bytes: usize,
    #[case] reached: bool,
) {
    assert_eq!(LIMIT.reached(rows, bytes), reached);
}

#[test]
fn a_zero_limit_is_always_reached() {
    let limit = PageLimit { rows: 0, bytes: 0 };

    assert!(limit.reached(0, 0));
}
