use super::Watermarks;
use crate::value_object::cursor::Cursor;

#[test]
fn holds_both_ends() {
    let watermarks = Watermarks {
        earliest: Cursor::new(3),
        end: Cursor::new(9),
    };

    assert_eq!(watermarks.earliest, Cursor::new(3));
    assert_eq!(watermarks.end, Cursor::new(9));
}

#[test]
fn compares_both_ends() {
    let watermarks = Watermarks {
        earliest: Cursor::new(3),
        end: Cursor::new(9),
    };
    let copy = watermarks;

    assert_eq!(copy, watermarks);
    assert_ne!(
        watermarks,
        Watermarks {
            earliest: Cursor::new(3),
            end: Cursor::new(8),
        }
    );
    assert_ne!(
        watermarks,
        Watermarks {
            earliest: Cursor::new(2),
            end: Cursor::new(9),
        }
    );
}
