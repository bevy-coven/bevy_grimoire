//! An after-edit check's `before` picture is the scene as the editor last saw it, and the first run
//! has none (#111).

use bevy_cauldron::after_edit::PictureMemory;

#[test]
fn the_first_run_has_no_before_and_the_next_has_the_last_after() {
    let mut m = PictureMemory::default();
    assert_eq!(m.swap(Some("a".into())), None);
    assert_eq!(m.swap(Some("b".into())), Some("a".into()));
    assert_eq!(m.swap(Some("c".into())), Some("b".into()));
}

#[test]
fn a_run_without_a_picture_keeps_the_memory() {
    let mut m = PictureMemory::default();
    m.swap(Some("a".into()));
    assert_eq!(m.swap(None), Some("a".into()));
    assert_eq!(m.swap(Some("b".into())), Some("a".into()));
}
