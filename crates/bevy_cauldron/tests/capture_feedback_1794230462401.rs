//! **The map carries no "Add meshes…"; the right panel does** (`feedback-1794230462401`, again `feedback-1794147258746`; oracle
//! `spec`).
//!
//! The note, from the editor on a project with no pieces: *"We do not want the 'Add Meshes' here, it should be in the panel on
//! the right. It shouldn't block the map area."* The pick is a scene point under the sentence *This project has no pieces yet*
//! in the middle of the viewport, where an accent **Add meshes…** stood over the map. The PLACE list in the right dock said the
//! same and offered its own button in the same frame.
//!
//! The relation the note states, on a real editor through `app.update()`: of the **Add meshes…** buttons an empty project draws,
//! none is inside the viewport (the map's area), and exactly one stands, in the PLACE list. The prompt that does belong over
//! the map, *Place a piece from the library* for a library that has pieces, is not asked to go (its own tests keep it).

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_cauldron::add_meshes::{ACTION, AddMeshesButton, NO_PIECES};
use bevy_cauldron::chrome::Frame;
use bevy_cauldron::editor::PaletteList;
use bevy_cauldron::empty_state::{EmptyMapPrompt, SENTENCE};
use bevy_cauldron::new_project::{self, NewProject, Template};

/// A scratch directory for one project.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire_capture_1794230462401_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

/// The editor on a new Empty project's first map: a library with no pieces, a map with no placements.
fn on_an_empty_project(dir: &std::path::Path) -> App {
    let form = NewProject { name: "blank".to_owned(), location: dir.to_string_lossy().into_owned(), template: Template::Empty };
    let root = new_project::create(&form).unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_cauldron::harness::build_headless(&root, new_project::FIRST_MAP, None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..12 {
        app.update();
    }
    app
}

/// Whether `entity` is `ancestor` or stands anywhere below it.
fn is_inside(world: &World, mut entity: Entity, ancestor: Entity) -> bool {
    loop {
        if entity == ancestor {
            return true;
        }
        match world.get::<ChildOf>(entity) {
            Some(parent) => entity = parent.parent(),
            None => return false,
        }
    }
}

#[test]
fn an_empty_project_draws_no_add_meshes_over_the_map_and_one_in_the_place_list() {
    let dir = scratch("empty");
    let mut app = on_an_empty_project(&dir);
    let viewport = app.world().resource::<Frame>().viewport;

    let buttons: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<AddMeshesButton>>().iter(app.world()).collect();
    let over_the_map: Vec<Entity> = buttons.iter().copied().filter(|b| is_inside(app.world(), *b, viewport)).collect();
    assert!(over_the_map.is_empty(), "no {ACTION:?} button stands in the map's viewport, found {over_the_map:?}");

    let lists: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<PaletteList>>().iter(app.world()).collect();
    let in_the_list: Vec<Entity> = buttons.iter().copied().filter(|b| lists.iter().any(|l| is_inside(app.world(), *b, *l))).collect();
    assert_eq!(in_the_list.len(), 1, "exactly one {ACTION:?} stands in the PLACE list: {buttons:?} against {lists:?}");
    assert_eq!(buttons.len(), 1, "and it is the only one on the map's door: {buttons:?}");

    let said: Vec<String> = app.world_mut().query::<&Text>().iter(app.world()).map(|t| t.0.clone()).collect();
    assert_eq!(said.iter().filter(|t| t.as_str() == NO_PIECES).count(), 1, "the sentence {NO_PIECES:?} is said once, by the list: {said:?}");
    assert!(!said.iter().any(|t| t == SENTENCE), "the prompt for a library with pieces is not shown either: {said:?}");
    assert_eq!(app.world_mut().query::<&EmptyMapPrompt>().iter(app.world()).count(), 0, "no prompt stands over the map");
    let _ = std::fs::remove_dir_all(&dir);
}
