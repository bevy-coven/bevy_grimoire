//! **A place's kind is read from its folder's name alone** (SG-07, #406).
//!
//! `style_edit` decides which of a place's pages an agent may write by the place's kind: a level's and an area's yes, the game's and a
//! region's only by proposal. The kind must be known for a place whose folder is not made yet (a level's first `style.md`), so it is read
//! from the path and never from the disk.

use bevy_grimoire_core::design::{PlaceKind, kind_of_folder};

#[test]
fn each_kind_of_place_is_named_by_its_folder() {
    assert_eq!(kind_of_folder("design"), Some(PlaceKind::Game));
    assert_eq!(kind_of_folder("design/regions/harbour"), Some(PlaceKind::Region));
    assert_eq!(kind_of_folder("design/levels/yard"), Some(PlaceKind::Level));
    assert_eq!(kind_of_folder("design/regions/harbour/levels/docks"), Some(PlaceKind::Level));
    assert_eq!(kind_of_folder("design/levels/yard/areas/well"), Some(PlaceKind::Area));
    assert_eq!(kind_of_folder("design/regions/harbour/levels/docks/areas/crane-1"), Some(PlaceKind::Area));
}

#[test]
fn a_folder_that_is_no_place_has_no_kind() {
    for folder in [
        "",
        "assets/grimoire",
        "design/levels",
        "design/regions",
        "design/levels/yard/board",
        "design/levels/yard/areas",
        "design/levels/ya rd",
        "design/levels/../secrets",
        "design/areas/well",
        "design/regions/harbour/areas/well",
        "design/levels/yard/areas/well/areas/x",
    ] {
        assert_eq!(kind_of_folder(folder), None, "`{folder}` is no place");
    }
}
