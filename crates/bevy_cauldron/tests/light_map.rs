//! **The light map reads `bevy_light_grid`'s own field** (#103): placements carrying a `bevy_lights`
//! `Flicker` are its fixtures, a wall blocks them, and the operation's numbers are the crate's.
#![cfg(feature = "bevy_light_grid")]

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn editor(name: &str) -> Ctx {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/torch", "alpha")
        .descriptor("site/wall", "alpha")
        .place("site/torch", (0.0, 0.0))
        .place("site/wall", (2.0, 0.0))
        .place("site/torch", (6.0, 6.0))
        .build("m");
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_lights\"])").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let mut project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    for d in project.library.descriptors.iter_mut().filter(|d| d.id == "site/wall") {
        d.kind = vec!["wall".to_owned()];
    }
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    Ctx { project: Some(project), open: Some(open), session: name.to_owned(), ..Ctx::default() }
}

fn torch(ctx: &mut Ctx, index: usize) {
    let id = ctx.open.as_ref().unwrap().map.placements.iter().filter(|p| p.descriptor == "site/torch").nth(index).unwrap().id.clone();
    let done = ops::dispatch(ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_lights", "value": "(base: 1000.0)"})).unwrap();
    assert!(matches!(done, Applied::Done { .. }), "{done:?}");
}

fn lux(ctx: &mut Ctx, at: (f32, f32)) -> f64 {
    match ops::dispatch(ctx, "light_map", &json!({"at": [[at.0, at.1]]})).unwrap() {
        Applied::Done { outcome, .. } => outcome.after.unwrap()["samples"][0]["lux"].as_f64().unwrap(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_cell_reads_what_the_grid_says_and_a_second_torch_changes_it() {
    let mut ctx = editor("light_map_two");
    assert_eq!(lux(&mut ctx, (0.0, 0.0)), 0.0, "no fixture, no light");
    torch(&mut ctx, 0);
    // The fixture's own cell is full intensity; the grid's falloff is 1 - dist/range, range 6 cells.
    assert!((lux(&mut ctx, (0.0, 0.0)) - 1.0).abs() < 1e-6);
    assert!((lux(&mut ctx, (0.0, 3.0)) - 0.5).abs() < 1e-6);
    let before = lux(&mut ctx, (4.0, 5.0));
    torch(&mut ctx, 1);
    assert!(lux(&mut ctx, (4.0, 5.0)) > before, "the second torch lights the cell more");
}

#[test]
fn a_wall_casts_a_shadow_the_light_map_shows() {
    let mut ctx = editor("light_map_wall");
    torch(&mut ctx, 0);
    assert_eq!(lux(&mut ctx, (4.0, 0.0)), 0.0, "the wall at x=2 shadows x=4");
    assert!(lux(&mut ctx, (0.0, 4.0)) > 0.0, "off the wall's line the light reaches");
}
