//! **The repository ships a map, so a reader and the demo storyboards have something to open** (#262).
//!
//! `assets/grimoire/maps/bedroom.map.ron` is the map every standing demo in `docs/media/` aims at and the
//! one the README's quick start opens. It names descriptors by id, and an id the kits do not define is
//! refused only when the editor resolves the map (`placement ... names descriptor ..., which this library
//! does not define`), which a headless build would never notice: so this resolves it here, against the
//! merged library the project binds.

use std::path::Path;

use bevy_grimoire_core::project_data;
use bevy_grimoire_core::stack;

fn grimoire_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/grimoire")
}

#[test]
fn the_shipped_bedroom_map_loads_and_every_piece_on_it_resolves_to_a_height() {
    let data = project_data::load(&grimoire_dir(), &["bedroom"]).expect("the shipped project loads with its bedroom");
    let map = data.maps.get("bedroom").expect("bedroom was asked for and is in the project's maps folder");

    assert!(!map.placements.is_empty(), "a demo map with nothing on it shows nothing");
    let heights = stack::resolve_y(map, &data.bound.library)
        .unwrap_or_else(|e| panic!("the shipped bedroom does not resolve against the shipped kits: {e}"));
    assert_eq!(heights.len(), map.placements.len(), "every placement gets a height");

    // The storyboards name these placements by id; a rename here must be a rename there.
    for id in ["bed", "sofa", "armchair", "bookcase", "bedside"] {
        assert!(map.placements.iter().any(|p| p.id == id), "the storyboards aim at `{id}`, which the bedroom lacks");
    }
}

#[test]
fn every_piece_in_the_bedroom_stands_inside_the_room() {
    let data = project_data::load(&grimoire_dir(), &["bedroom"]).expect("the shipped project loads with its bedroom");
    let map = &data.maps["bedroom"];
    let (w, _, d) = map.bounds;
    for p in &map.placements {
        let (x, z) = p.at;
        assert!(
            x.abs() <= w / 2.0 && z.abs() <= d / 2.0,
            "`{}` stands at ({x}, {z}), outside the {w} x {d} m room a demo frames",
            p.id
        );
    }
}
