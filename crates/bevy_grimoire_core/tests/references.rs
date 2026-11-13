//! **The pictures a person pointed at, as a place's dream board** (E-075, #124; SG-03, #402): a "like this" is stored in
//! the place's `board/board.yaml`, named by its path (the `r1` ids went with `references.yaml`), and checked against the
//! disk.
//!
//! Owner exception (2026-11-13): this file tested `bevy_grimoire_core::references`, which the board replaced; each check
//! it made is kept, on the type that took its place.

use bevy_grimoire_core::board::{self, Board, Entry};

#[test]
fn a_picture_is_named_by_its_path_and_the_agent_can_cite_it() {
    let mut b = Board::default();
    b.add(Entry::new("refs/courthouse.png", "columns, steps up to the door"), None).unwrap();
    b.add(Entry::new("refs/street.jpg", "narrow, shop fronts"), None).unwrap();
    assert_eq!(b.cite("refs/courthouse.png").unwrap(), "board picture refs/courthouse.png: columns, steps up to the door");
    // a picture taken off is not citable, and the rest keep their order
    b.remove("refs/street.jpg").unwrap();
    b.add(Entry::new("refs/market.png", "stalls"), None).unwrap();
    assert!(b.cite("refs/street.jpg").unwrap_err().contains("not on this board"));
    assert_eq!(b.pictures.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), ["refs/courthouse.png", "refs/market.png"]);
    // and a citation in a note's words is found by its path
    assert_eq!(board::citations(["matched board picture refs/market.png: stalls"]), ["refs/market.png"]);
}

#[test]
fn the_same_picture_is_one_entry_and_paths_stay_inside_the_project() {
    let mut b = Board::default();
    b.add(Entry::new("refs/a.png", "one"), None).unwrap();
    let e = b.add(Entry::new("refs/./a.png", "again"), None).unwrap_err();
    assert!(e.contains("refs/a.png") && e.contains("already"), "{e}");
    for bad in ["/etc/x.png", "../x.png", "refs/../../x.png", "refs/notes.txt", ""] {
        assert!(b.add(Entry::new(bad, "x"), None).is_err(), "{bad} was accepted");
    }
}

#[test]
fn saved_in_the_place_and_missing_files_are_reported_and_kept() {
    let dir = std::env::temp_dir().join(format!("references_board_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("refs")).unwrap();
    std::fs::write(dir.join("refs/here.png"), b"x").unwrap();
    let place = board::level_place(&dir, "town");
    assert_eq!(place, "design/levels/town", "a map with no folder yet is the level in no region");
    let file = dir.join(board::file_of(&place));
    assert_eq!(file, dir.join("design/levels/town/board/board.yaml"));

    let mut b = Board::default();
    b.add(Entry::new("refs/here.png", "present"), None).unwrap();
    b.add(Entry::new("refs/gone.png", "deleted"), None).unwrap();
    b.save(&file).unwrap();
    let back = Board::load(&file).unwrap();
    assert_eq!(back, b);
    assert_eq!(back.missing(&dir).iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), ["refs/gone.png"]);
    assert_eq!(back.pictures.len(), 2, "a missing picture stays on the board");
    assert_eq!(board::places_with_a_board(&dir), [place]);
    // no pictures and no file writes nothing
    let none = dir.join("design/levels/none/board/board.yaml");
    Board::default().save(&none).unwrap();
    assert!(!none.exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_caption_says_what_to_take_and_a_restricted_picture_never_ships() {
    let mut b = Board::default();
    assert!(b.add(Entry::new("refs/a.png", "  "), None).unwrap_err().contains("caption"), "an empty caption is refused");
    let mut shop = Entry::new("assets/shots/store.png", "the shelf spacing");
    shop.restricted = true;
    assert!(b.add(shop.clone(), None).unwrap_err().contains("restricted"), "a restricted picture under assets/ is refused");
    shop.path = "design/board/store.png".to_owned();
    b.add(shop, None).unwrap();
    assert!(b.recaption("design/board/store.png", "").is_err(), "re-captioning to nothing is refused too");
    assert!(b.shipping_restricted().is_empty());
}
