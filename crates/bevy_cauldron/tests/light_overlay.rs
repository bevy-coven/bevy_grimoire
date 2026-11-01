//! **The light map drawn in the viewport is the light map an agent reads** (#103): shown by its
//! button, the overlay's value at a cell is `light_map`'s value for that cell, and it follows the map
//! when a torch is lit.
#![cfg(feature = "bevy_light_grid")]

use bevy::prelude::App;
use bevy_cauldron::light_overlay::LightOverlay;
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::json;

use crate::fixtures;

const POINTS: [(f32, f32); 5] = [(0.0, 0.0), (0.0, 3.0), (4.0, 0.0), (4.0, 5.0), (6.0, 6.0)];

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
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
    for d in app.world_mut().resource_mut::<Project>().library.descriptors.iter_mut().filter(|d| d.id == "site/wall") {
        d.kind = vec!["wall".to_owned()];
    }
    app
}

/// Light the `index`th torch the way `placement_set_family` leaves the map.
fn light(app: &mut App, index: usize) {
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    let torch = open.map.placements.iter_mut().filter(|p| p.descriptor == "site/torch").nth(index).unwrap();
    torch.family.insert("bevy_lights".to_owned(), "(base: 1000.0)".to_owned());
}

/// What an agent reads for the cell at `at`, through the registry.
fn agent_reads(app: &App, at: (f32, f32)) -> f32 {
    let mut ctx = Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        session: "light_overlay".to_owned(),
        ..Ctx::default()
    };
    match ops::dispatch(&mut ctx, "light_map", &json!({"at": [[at.0, at.1]]})).unwrap() {
        Applied::Done { outcome, .. } => outcome.after.unwrap()["samples"][0]["lux"].as_f64().unwrap() as f32,
        other => panic!("{other:?}"),
    }
}

fn shown(app: &App, at: (f32, f32)) -> f32 {
    let overlay = app.world().resource::<LightOverlay>();
    overlay.drawn.as_ref().expect("the light map is shown").lux_at(at).expect("the point is on the map")
}

#[test]
fn the_overlay_shows_the_light_maps_own_cells_and_follows_a_lit_torch() {
    let mut app = editor("light_overlay_follows");
    light(&mut app, 0);
    app.update();
    assert_eq!(app.world().resource::<LightOverlay>().drawn, None, "hidden until its button shows it");

    app.world_mut().resource_mut::<LightOverlay>().on = true;
    app.update();
    for at in POINTS {
        assert_eq!(shown(&app, at), agent_reads(&app, at), "the overlay at {at:?} is the light map's cell");
    }
    assert_eq!(shown(&app, (4.0, 0.0)), 0.0, "the wall at x=2 shadows x=4 on screen too");
    let before = shown(&app, (4.0, 5.0));

    light(&mut app, 1);
    app.update();
    assert!(shown(&app, (4.0, 5.0)) > before, "lighting the second torch brightens the overlay");
    for at in POINTS {
        assert_eq!(shown(&app, at), agent_reads(&app, at), "after the second torch, the overlay at {at:?} is the light map's cell");
    }

    app.world_mut().resource_mut::<LightOverlay>().on = false;
    app.update();
    assert_eq!(app.world().resource::<LightOverlay>().drawn, None, "hiding it drops the field");
}
