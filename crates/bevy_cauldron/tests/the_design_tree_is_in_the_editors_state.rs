//! **`grimoire_state.design` lists the style tree and its problems, and an edit on disk is in the next
//! answer with no restart** (SG-01, #400; the series #399).
//!
//! The tree is not held anywhere: `bevy_grimoire_core::design::Tree::read` walks `design/` when the state is
//! asked for, which is why its reload row is `Live` and why an edit reaches an agent at once. This test is
//! the editor's half of that: it asks `grimoire/state` through the registry an agent uses, twice, with an
//! edit between the two calls and no frame in between — so a cache read would be caught.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops_brp::OpsBridgePlugin;
use serde_json::{Value, json};

use crate::fixtures;

/// **A project with a design tree**: the game, one region, a level inside it and that level's area.
fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    std::fs::create_dir_all(root.join("design/regions/north/levels/m/areas/docks")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("design/style.md"), "Grey concrete, sodium light.\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("design/regions/north/style.md"), "Salt on everything.\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("design/regions/north/levels/m/style.md"), "The wall is the horizon.\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        root.join("design/regions/north/levels/m/areas/docks/area.yaml"),
        "version: 1\nrect: [-8.0, -4.0, 4.0, 6.0]\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("design/regions/north/levels/m/areas/docks/style.md"), "Cranes, oil, rope.\n").unwrap_or_else(|e| panic!("{e}"));
    root
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(OpsBridgePlugin);
    // **Six frames**: the bridge builds its resources and takes its first decisions in the same passes the
    // guide's own test waits through; fewer leaves a `drop` of the app waiting on a frame that never ran.
    for _ in 0..6 {
        app.update();
    }
    app
}

/// **The `design` field of `grimoire/state`**, through the same registered method an agent calls.
fn design_of(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    let state = app
        .world_mut()
        .run_system_with(id, Some(json!({})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    state["design"].clone()
}

/// The place with this folder, from the answer.
fn place<'a>(design: &'a Value, path: &str) -> &'a Value {
    design["places"]
        .as_array()
        .and_then(|places| places.iter().find(|p| p["path"] == path))
        .unwrap_or_else(|| panic!("`{path}` is not a place of {}", design["places"]))
}

#[test]
fn grimoire_state_lists_the_design_tree_with_its_topics_an_areas_rectangle_and_its_problems() {
    let root = project(&format!("state_design_{}", std::process::id()));
    let mut app = editor(&root);
    let design = design_of(&mut app);

    let kinds: Vec<&str> = design["places"].as_array().expect("places is a list").iter().map(|p| p["kind"].as_str().unwrap_or("")).collect();
    assert_eq!(kinds, vec!["game", "region", "level", "area"], "every place, the whole game first: {design}");
    assert_eq!(place(&design, "design")["topics"][0]["name"], "style", "a place's topics are its files, `style` first");
    assert_eq!(place(&design, "design")["topics"][0]["file"], "design/style.md");
    let docks = place(&design, "design/regions/north/levels/m/areas/docks");
    assert_eq!(docks["rect"], json!([-8.0, -4.0, 4.0, 6.0]), "an area's rectangle, in map metres");
    assert_eq!(docks["map"], "m", "an area names its level's map");
    assert_eq!(design["problems"], json!([]), "a sound tree has no problems");

    // **And a fault is reported here**, with the path, so a person and an agent read the same sentence.
    // **Below the game, not at it**: `design/sound.md` would be the game's own topic, which is what a topic
    // below the game needs to have.
    std::fs::create_dir_all(root.join("design/regions/north")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("design/regions/north/sound.md"), "Gulls.\n").unwrap_or_else(|e| panic!("{e}"));
    let design = design_of(&mut app);
    let problems = design["problems"].as_array().expect("problems is a list");
    assert_eq!(problems.len(), 1, "a topic below the game with no game-level file is one problem: {design}");
    assert_eq!(problems[0]["where"]["file"], "design/sound.md", "the problem names the missing file");
    assert!(
        problems[0]["what"].as_str().is_some_and(|what| what.contains("sound")),
        "and says what is wrong with it: {}",
        problems[0]
    );
}

#[test]
fn an_edit_to_a_style_file_is_in_the_next_answer_with_no_restart() {
    let root = project(&format!("state_design_live_{}", std::process::id()));
    let mut app = editor(&root);
    let first = design_of(&mut app);
    assert_eq!(place(&first, "design")["topics"][0]["bytes"], json!("Grey concrete, sodium light.\n".len()));

    // **No frame runs between the write and the next question.** The tree is read from disk on every call, so
    // a cached answer would show the old size here.
    std::fs::write(root.join("design/style.md"), "Grey concrete, sodium light, and rain.\n").unwrap_or_else(|e| panic!("{e}"));
    let second = design_of(&mut app);
    assert_eq!(
        place(&second, "design")["topics"][0]["bytes"],
        json!("Grey concrete, sodium light, and rain.\n".len()),
        "an edit is in the next answer with no restart: {second}"
    );

    // A place that is added is a place the next answer lists, and one that is removed is gone.
    std::fs::create_dir_all(root.join("design/levels/m2")).unwrap_or_else(|e| panic!("{e}"));
    let third = design_of(&mut app);
    assert!(
        third["places"].as_array().is_some_and(|places| places.iter().any(|p| p["path"] == "design/levels/m2")),
        "a level folder made on disk is a place: {third}"
    );
    // **A level whose map does not exist is a problem**, named with both paths, and it is reported here.
    assert!(
        third["problems"].as_array().is_some_and(|problems| problems.iter().any(|p| p["what"]
            .as_str()
            .is_some_and(|what| what.contains("m2")))),
        "and the level's missing map is a problem: {third}"
    );
}

#[test]
fn a_project_with_no_design_folder_answers_a_tree_of_the_game_alone() {
    // **A project with no style yet answers an empty tree, not nothing**: the game is a place whether or not
    // its folder exists, and "no style" is a fact an agent should be able to read.
    let root = fixtures::Fixture::new(&format!("state_design_empty_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = editor(&root);
    let design = design_of(&mut app);
    assert_eq!(design["places"].as_array().map(Vec::len), Some(1), "the game alone: {design}");
    assert_eq!(design["places"][0]["kind"], "game");
    assert_eq!(design["problems"], json!([]), "most projects have no style yet, which is not a fault");
}
