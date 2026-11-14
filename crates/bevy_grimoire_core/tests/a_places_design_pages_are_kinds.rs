//! **A place's Markdown pages are `design` wherever the place is** (#420): a row's path is matched one segment at a time, so
//! `reload::kind_of` classifies a page beside a place's own files by the shape of the place around it, as it does a board's files.

use bevy_grimoire_core::reload::{Base, holds_place_files, kind_of};

fn id(path: &str) -> Option<&'static str> {
    kind_of(Base::Root, path).map(|k| k.id)
}

#[test]
fn a_page_in_any_place_is_the_design_kind() {
    for place in ["design", "design/regions/north", "design/levels/yard", "design/regions/n/levels/y", "design/levels/y/areas/docks", "design/regions/n/levels/y/areas/docks"] {
        assert_eq!(id(&format!("{place}/style.md")), Some("design"), "{place}");
        assert_eq!(id(&format!("{place}/dialogue.MD")), Some("design"), "{place}: the extension is not case-sensitive");
    }
}

#[test]
fn what_is_not_a_places_page_is_not_design() {
    for path in ["design/topics/x/style.md", "design/levels/style.md", "design/levels/y/board/style.md", "design/levels/.y/style.md", "assets/design/levels/y/style.md", "designx/levels/y/style.md"] {
        assert_eq!(id(path), None, "{path}");
    }
    assert_eq!(id("design/levels/y/area.yaml"), None, "only a Markdown page is a topic");
    assert_eq!(kind_of(Base::Grimoire, "design/levels/y/style.md").map(|k| k.id), None, "only under the project root");
}

#[test]
fn a_walk_enters_the_folders_a_place_file_can_be_in_and_no_others() {
    for dir in ["design", "design/levels", "design/regions", "design/regions/n", "design/regions/n/levels", "design/levels/y", "design/levels/y/areas", "design/levels/y/areas/a", "design/levels/y/board"] {
        assert!(holds_place_files(dir), "{dir}");
    }
    for dir in ["design/topics", "design/levels/y/areas/a/deeper", "design/levels/y/board/deeper", "assets", "design/levels/y/areas/a/board/x", "board"] {
        assert!(!holds_place_files(dir), "{dir}");
    }
}

/// **The walk and `kind_of` are one rule**: every folder above a file `kind_of` classifies by the shape of a place is one the walk enters, so no
/// classified file is out of reach of the panel (#420).
#[test]
fn every_folder_above_a_classified_place_file_is_one_the_walk_enters() {
    let places = ["design", "design/regions/north", "design/levels/yard", "design/regions/n/levels/y", "design/levels/y/areas/docks", "design/regions/n/levels/y/areas/docks"];
    for place in places {
        for file in ["style.md", "dialogue.md", "board/board.yaml", "board/fog.png", "board/fog.JPEG"] {
            let path = format!("{place}/{file}");
            assert!(kind_of(Base::Root, &path).is_some(), "{path} is classified");
            for (at, _) in path.match_indices('/') {
                assert!(holds_place_files(&path[..at]), "{path} is a kind, and the walk does not enter {}", &path[..at]);
            }
        }
    }
}
