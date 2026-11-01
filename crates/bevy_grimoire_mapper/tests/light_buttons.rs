//! **A person reaches the light map and the pool budget by clicking, and the click is the operation** (#103).
//!
//! `light_map` and `light_pool` were registered with an MCP route and no UI entry, so the parity test
//! was red under `--features bevy_lights,bevy_light_grid`. The LIGHT MAP button now dispatches
//! `light_map` and says its receipt; the POOL HERE button dispatches `light_pool` at the point the
//! camera looks at. Each leaves one line in the operation log, from a human.
#![cfg(feature = "bevy_lights")]

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_grimoire_mapper::editor::EditorState;
use bevy_grimoire_mapper::ops::{self, Source};
use bevy_grimoire_mapper::project::Project;
use bevy_grimoire_mapper::view::Rig;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
        .descriptor("site/torch", "alpha")
        .place("site/torch", (0.0, 0.0))
        .place("site/torch", (30.0, 0.0))
        .build("m");
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_lights\"])").unwrap();
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let mut open = app.world_mut().resource_mut::<bevy_grimoire_mapper::project::OpenMap>();
    for p in open.map.placements.iter_mut() {
        p.family.insert("bevy_lights".to_owned(), "(base: 1000.0)".to_owned());
    }
    app
}

/// Click the one button carrying component `B`, as the button's own `Activate` does.
fn click<B: Component>(app: &mut App) {
    let entity = app.world_mut().query_filtered::<Entity, With<B>>().iter(app.world()).next().expect("the button is on screen");
    app.world_mut().trigger(Activate { entity });
    app.world_mut().flush();
    app.update();
}

fn logged(app: &App, op: &str) -> Vec<ops::Entry> {
    let project = app.world().resource::<Project>();
    ops::read_entries(&ops::oplog_path(project)).into_iter().filter(|e| e.op == op).collect()
}

#[test]
fn the_pool_button_asks_the_pool_where_the_camera_looks_and_logs_it_as_a_person() {
    let mut app = editor("light_buttons_pool");
    app.world_mut().resource_mut::<Rig>().focus = Vec3::new(1.0, 0.0, 0.0);
    click::<bevy_grimoire_mapper::ops_pool::PoolHere>(&mut app);
    let note = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(note.starts_with("light pool: "), "the status says the operation's receipt, not a note of its own: {note:?}");
    let entries = logged(&app, "light_pool");
    assert_eq!(entries.len(), 1, "one click, one logged operation");
    assert_eq!(entries[0].source, Source::Human);
    assert_eq!(entries[0].params["focus"], serde_json::json!([1.0, 0.0]), "the focus is the camera's ground point in map space");
}

#[cfg(feature = "bevy_light_grid")]
#[test]
fn the_light_map_button_dispatches_the_light_map_and_says_its_receipt() {
    use bevy_grimoire_mapper::light_overlay::{LightMapToggle, LightOverlay};
    let mut app = editor("light_buttons_map");
    click::<LightMapToggle>(&mut app);
    assert!(app.world().resource::<LightOverlay>().on, "the overlay is shown");
    let note = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(note.contains("light map: 2 fixtures"), "the status carries the operation's receipt: {note:?}");
    let entries = logged(&app, "light_map");
    assert_eq!(entries.len(), 1, "showing it is one logged operation");
    assert_eq!(entries[0].source, Source::Human);

    click::<LightMapToggle>(&mut app);
    assert!(!app.world().resource::<LightOverlay>().on, "the second click hides it");
    assert_eq!(logged(&app, "light_map").len(), 1, "hiding reads nothing, so it logs nothing");
}
