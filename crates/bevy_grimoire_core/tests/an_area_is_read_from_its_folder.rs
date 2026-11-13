//! **The core's reads of an area** (SG-06, #405): the names an area may have, the levels of a tree, the smallest area that holds a point.

use bevy_grimoire_core::design::{self, AreaOnMap};

fn area(name: &str, rect: [f32; 4]) -> AreaOnMap {
    AreaOnMap { name: name.to_owned(), path: format!("design/levels/m/areas/{name}"), rect, has_style: false }
}

#[test]
fn an_area_name_is_a_folder_name_and_a_label() {
    for good in ["docks", "north-quay_2", "A1"] {
        assert!(design::is_area_name(good), "{good}");
    }
    for bad in ["", "../x", "a/b", ".hidden", "two words", "a:b", &"x".repeat(design::AREA_NAME_MAX_CHARS + 1)] {
        assert!(!design::is_area_name(bad), "{bad:?}");
    }
}

#[test]
fn the_smallest_area_that_holds_a_point_is_the_one_it_is_in() {
    let areas = [area("whole", [0.0, 0.0, 10.0, 10.0]), area("inner", [2.0, 2.0, 4.0, 4.0])];
    assert_eq!(design::area_at(&areas, (3.0, 3.0)).map(|a| a.name.as_str()), Some("inner"));
    assert_eq!(design::area_at(&areas, (8.0, 8.0)).map(|a| a.name.as_str()), Some("whole"));
    assert!(design::area_at(&areas, (11.0, 3.0)).is_none());
}

#[test]
fn levels_are_found_by_their_folders_and_an_area_file_reads_back() {
    let root = std::env::temp_dir().join(format!("area_levels_{}", std::process::id()));
    std::fs::create_dir_all(root.join("design/levels/yard/areas/docks")).unwrap();
    std::fs::create_dir_all(root.join("design/regions/north/levels/harbour")).unwrap();
    std::fs::write(root.join("design/levels/yard/areas/docks/area.yaml"), design::area_yaml([1.0, 2.0, 3.0, 4.0]).unwrap()).unwrap();
    assert_eq!(design::levels_of(&root), ["design/levels/yard", "design/regions/north/levels/harbour"]);
    let read = design::areas_of_level(&root, "design/levels/yard");
    assert_eq!(read.len(), 1);
    assert_eq!((read[0].name.as_str(), read[0].rect, read[0].has_style), ("docks", [1.0, 2.0, 3.0, 4.0], false));
    std::fs::remove_dir_all(&root).unwrap();
}
