//! **A piece on the floor stands on the map's sculpted ground** (#99): the editor's heights read the saved terrain.

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::json;

use crate::fixtures;

#[test]
fn sculpting_lifts_the_piece_and_no_terrain_leaves_it_flat() {
    let root = fixtures::Fixture::new("stands_on_ground").descriptor("site/wall", "alpha").place("site/wall", (5.0, 5.0)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<OpenMap>().clone();
    let flat = bevy_cauldron::ops_terrain::heights_on_ground(&project, &open).unwrap();
    let mut ctx = Ctx { project: Some(project.clone()), open: Some(open.clone()), session: "test".to_owned(), ..Ctx::default() };
    let p = json!({
        "map": "m",
        "stroke": {"brush": "Raise", "radius_m": 3.0, "strength": 2.0, "points": [[5.0, 5.0]]},
        "create": {"origin": [0.0, 0.0], "cell_m": 1.0, "width": 11, "depth": 11, "height_m": 0.0}
    });
    let Applied::Done { operation_id, .. } = ops::dispatch(&mut ctx, "terrain_stroke", &p).unwrap() else { panic!("refused") };
    let hill = bevy_cauldron::ops_terrain::heights_on_ground(&project, &open).unwrap();
    assert!(hill[0] > flat[0] + 1.0, "the piece rides the hill: flat {} hill {}", flat[0], hill[0]);
    ops::undo(&mut ctx, Some(&operation_id)).unwrap();
    let back = bevy_cauldron::ops_terrain::heights_on_ground(&project, &open).unwrap();
    assert_eq!(back, flat, "undo puts the piece back on the flat");
}
