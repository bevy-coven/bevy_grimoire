//! **A terrain file whose heights do not fill its shape is refused, not indexed** (#187).
//!
//! A hand edit, a truncated write or a bad merge can leave `terrain/<map>.ron` with fewer heights
//! than `width × depth`. The editor panicked drawing the ground ("index out of bounds") and inside
//! `terrain_stroke`'s dispatch. Now the map opens with no terrain drawn and the file named on the
//! status line, and a stroke on it is refused.

use bevy::prelude::*;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::terrain_view::TerrainView;
use serde_json::json;

use crate::fixtures;

const SHORT: &str = "(origin: (0.0, 0.0), cell_m: 1.0, width: 4, depth: 4, heights: [0.0, 0.0, 0.0])";

fn problems(app: &App) -> Vec<String> {
    app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect()
}

/// A project whose map `m` is open headless when the short terrain file lands, as a hand edit or a
/// bad merge would, with the terrain view given time to read it (it looks every half second).
fn open(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    let terrain = app.world().resource::<bevy_cauldron::project::Project>().grimoire_dir.join("terrain");
    std::fs::create_dir_all(&terrain).unwrap_or_else(|e| panic!("{terrain:?}: {e}"));
    let file = terrain.join("m.ron");
    std::fs::write(&file, SHORT).unwrap_or_else(|e| panic!("{file:?}: {e}"));
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < 3.0 && !problems(&app).iter().any(|p| p.contains("terrain/m.ron")) {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    (app, file)
}

#[test]
fn it_draws_no_ground_and_the_status_line_names_the_file() {
    let (mut app, _) = open("terrain_short_open");
    let named = problems(&app);
    assert!(
        named.iter().any(|p| p.contains("terrain/m.ron") && p.contains("3 heights for 4x4")),
        "the status line names the file and its shape: {named:?}"
    );
    let world = app.world_mut();
    let drawn = world.query::<&TerrainView>().iter(world).count();
    assert_eq!(drawn, 0, "a terrain that does not read draws nothing");
}

#[test]
fn a_stroke_on_it_is_refused_and_writes_nothing() {
    let (app, file) = open("terrain_short_stroke");
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    let mut ctx = Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() };
    let p = json!({"map": "m", "stroke": {"brush": "Raise", "radius_m": 2.0, "strength": 1.0, "points": [[1.0, 1.0]]}});
    let refused = match ops::dispatch(&mut ctx, "terrain_stroke", &p) {
        Err(e) | Ok(Applied::Refused(e)) => e.0,
        Ok(_) => panic!("a stroke on a terrain that does not fill its shape was applied"),
    };
    assert!(refused.contains("3 heights for 4x4"), "{refused}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), SHORT, "the refused stroke wrote nothing");
}
