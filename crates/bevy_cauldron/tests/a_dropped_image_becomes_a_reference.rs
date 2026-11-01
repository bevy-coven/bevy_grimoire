//! **An image dropped on the editor window becomes a reference of the open map** (#124, E-075).
//!
//! The drop goes through `reference_set`, like the `ref` line and an agent's `apply`. A picture from
//! outside the project is copied into the project's `references/` first, so the reference travels
//! with the project; one already inside is referenced where it stands. A non-image, or a different
//! picture under a name `references/` already holds, changes nothing and says why.

use bevy::prelude::*;
use bevy::window::FileDragAndDrop;
use bevy_grimoire_core::references::{self, References};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

fn drop_file(app: &mut App, path: &std::path::Path) {
    app.world_mut().write_message(FileDragAndDrop::DroppedFile { window: Entity::PLACEHOLDER, path_buf: path.to_path_buf() });
    app.update();
}

fn paths(app: &App) -> Vec<String> {
    let open = app.world().resource::<OpenMap>();
    References::load(&references::path_for_map(&open.map_path)).unwrap().refs.iter().map(|r| r.path.clone()).collect()
}

#[test]
fn a_dropped_image_is_referenced_and_copied_into_the_project() {
    let root = fixtures::Fixture::new("dropped_reference").descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project_root = app.world().resource::<Project>().root.clone();
    let outside = std::env::temp_dir().join(format!("dropped_reference_{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();

    // From outside the project: copied into references/, and referenced there.
    let court = outside.join("court.png");
    std::fs::write(&court, b"court picture").unwrap();
    drop_file(&mut app, &court);
    assert_eq!(paths(&app), ["references/court.png"], "status: {}", app.world().resource::<EditorState>().status.problem_text());
    assert_eq!(std::fs::read(project_root.join("references/court.png")).unwrap(), b"court picture");

    // Already inside the project: referenced where it stands, nothing copied.
    std::fs::create_dir_all(project_root.join("art")).unwrap();
    let inside = project_root.join("art/hall.jpg");
    std::fs::write(&inside, b"hall picture").unwrap();
    drop_file(&mut app, &inside);
    assert_eq!(paths(&app), ["references/court.png", "art/hall.jpg"]);
    assert!(!project_root.join("references/hall.jpg").exists(), "a picture inside the project is not copied");

    // A non-image, and a different picture under a name references/ already holds, change nothing.
    let notes = outside.join("notes.txt");
    std::fs::write(&notes, b"words").unwrap();
    drop_file(&mut app, &notes);
    assert!(app.world().resource::<EditorState>().status.problem_text().contains("notes.txt"), "a refused drop says which file");
    let other = outside.join("other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("court.png"), b"a different court").unwrap();
    drop_file(&mut app, &other.join("court.png"));
    assert!(app.world().resource::<EditorState>().status.problem_text().contains("references/court.png"), "the clash is named");
    assert_eq!(paths(&app), ["references/court.png", "art/hall.jpg"]);
    assert_eq!(std::fs::read(project_root.join("references/court.png")).unwrap(), b"court picture", "the first picture is not overwritten");

    std::fs::remove_dir_all(&outside).unwrap();
}
