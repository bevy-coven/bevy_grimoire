//! **A place's board files are kinds wherever the place is** (SG-03, #402): a row's path is matched one segment at a time, so
//! `reload::kind_of` classifies the files of a `board/` folder by the shape of the place around it.

use bevy_grimoire_core::reload::{Base, kind_of};

fn id(path: &str) -> Option<&'static str> {
    kind_of(Base::Root, path).map(|k| k.id)
}

#[test]
fn a_board_file_and_a_picture_in_any_places_board_folder_have_their_kinds() {
    for place in ["design", "design/regions/north", "design/levels/yard", "design/regions/n/levels/y", "design/levels/y/areas/docks"] {
        assert_eq!(id(&format!("{place}/board/board.yaml")), Some("board"), "{place}");
        assert_eq!(id(&format!("{place}/board/fog.png")), Some("reference_images"), "{place}");
        assert_eq!(id(&format!("{place}/board/FOG.JPEG")), Some("reference_images"), "{place}: the extension is not case-sensitive");
        assert_eq!(id(&format!("{place}/board/notes.txt")), None, "{place}: a file that is neither is no kind");
    }
}

#[test]
fn what_is_not_a_places_board_folder_is_not_one() {
    for path in ["design/topics/x/board/board.yaml", "assets/board/board.yaml", "board/board.yaml", "design/levels/y/board/deeper/board.yaml", "design/levels/.y/board/fog.png"] {
        assert_eq!(id(path), None, "{path}");
    }
    assert_eq!(id("references/Sketch v2.JPG"), Some("reference_images"), "the folder earlier editors copied into is still the pattern's");
    assert_eq!(kind_of(Base::Grimoire, "design/levels/y/board/fog.png").map(|k| k.id), None, "only under the project root");
}
