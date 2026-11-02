//! **The toolbar arms a tool, and it is the key's own body that does it** (UI-37, #231).
//!
//! The acceptance is *"Clicking Fill arms the fill exactly as its key does"*; the issue's comment
//! measured that there is no Fill *tool*, so this slice takes the half that can be built and holds it
//! to the same standard: **a toolbar button and its key dispatch one thing**. The button carries a
//! command id and hands it to `command_run::Chords` — the palette's queue — so the editor's key
//! handler is what changes the tool, and a button cannot arm Move one way while `B` arms it another.
//!
//! Three claims, and each can fail:
//!
//! 1. **Every command on the bar is a registered command with a glyph.** A button that names nothing
//!    is a picture, and one with no glyph is a blank square.
//! 2. **A click asks for that command's chord** and nothing more: the queue holds it, and the tool has
//!    not changed yet — the same two-frame path the palette takes.
//! 3. **The armed tool's button is the only lit one**, read from `EditorState`, so the light follows
//!    the key rather than a click.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..6 {
        app.update();
    }
    (root, app)
}

/// **The bar is there, once, and every button on it names a registered command with an icon.**
#[test]
fn the_toolbar_shows_registered_commands_that_have_icons() {
    let (_root, mut app) = editor("toolbar-shows");
    let bars = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_cauldron::toolbar::Toolbar>>()
        .iter(app.world())
        .count();
    assert_eq!(bars, 1, "the window has exactly one toolbar; {bars} means it was spawned twice or not at all");

    let buttons: Vec<&'static str> = {
        let world = app.world_mut();
        let mut q = world.query::<&bevy_cauldron::toolbar::ToolbarCommand>();
        q.iter(world).map(|b| b.0).collect()
    };
    assert!(
        buttons.len() >= 8,
        "a toolbar of {} buttons is not the editor's command set: {buttons:?}",
        buttons.len()
    );
    for id in &buttons {
        assert!(
            bevy_cauldron::commands::by_id(id).is_some(),
            "`{id}` is on the bar but is not a registered command, so a click on it dispatches nothing"
        );
        assert!(
            bevy_cauldron::icons::index(id).is_some(),
            "`{id}` has no glyph, so its button is a blank square"
        );
    }
    // **The tool modes are all there**, which is what the acceptance is about.
    for (id, _) in bevy_cauldron::toolbar::tools() {
        assert!(buttons.contains(&id), "the toolbar is missing `{id}`: a tool with no button is a mode a person can only reach by key");
    }
}

/// **A click on the Move button asks for Move's chord and leaves the tool alone**, because the chord
/// has not gone down yet — the palette's two-frame path, which is what keeps one arming body.
#[test]
fn a_click_on_the_move_button_asks_for_the_chord_and_the_key_arms_it() {
    let (_root, mut app) = editor("toolbar-click");
    assert_eq!(
        app.world().resource::<bevy_cauldron::editor::EditorState>().tool,
        bevy_cauldron::editor::Tool::Place,
        "the editor starts on Place"
    );

    let button = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &bevy_cauldron::toolbar::ToolbarCommand)>();
        q.iter(world)
            .find(|(_, b)| b.0 == "map.move_mode")
            .map(|(e, _)| e)
            .expect("the Move button is on the bar")
    };
    // **`commands().trigger`, not `world_mut().trigger`**: the deferred form is what gooey's own
    // tests use to reach an observer, and it flushes with the next `update`.
    // **`commands().trigger`, not `world_mut().trigger`**: the deferred form is what gooey's own
    // tests use to reach an observer. Flush it without running a schedule, so what the click queued
    // is read *before* `press_waiting` (a `PreUpdate` system) consumes it in the next `update`.
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: button });
    app.world_mut().flush();

    // **The queue holds Move's chord, and the tool has not changed.** That is the claim that a click
    // arms nothing by itself: it asks, exactly as the palette does, and the key handler decides.
    let waiting = app.world().resource::<bevy_cauldron::command_run::Chords>();
    assert!(
        waiting.waiting() > 0,
        "the click asked for nothing: the button is a picture, not a command"
    );
    assert_eq!(
        app.world().resource::<bevy_cauldron::editor::EditorState>().tool,
        bevy_cauldron::editor::Tool::Place,
        "a click must not arm a tool by itself — only the chord it asks for may, or the button and \
         its key are two arming paths"
    );

    // Frames later the chord has been pressed and the editor's own handler has armed Move.
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<bevy_cauldron::editor::EditorState>().tool,
        bevy_cauldron::editor::Tool::Move,
        "the chord the button asked for must arm Move through the key handler, or the button and `B` \
         are two arming paths"
    );
}

/// **The armed tool's button is lit and no other is**, read from the state rather than from a click.
#[test]
fn the_armed_tools_button_is_the_only_lit_one() {
    let (_root, mut app) = editor("toolbar-lit");
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().tool =
        bevy_cauldron::editor::Tool::Clone;
    for _ in 0..2 {
        app.update();
    }

    let mut lit: Vec<&'static str> = Vec::new();
    {
        let world = app.world_mut();
        let mut q = world.query::<(&bevy_cauldron::toolbar::ToolbarCommand, &Children)>();
        let pairs: Vec<(&'static str, Vec<Entity>)> = q
            .iter(world)
            .map(|(b, children)| (b.0, children.iter().collect()))
            .collect();
        for (id, children) in pairs {
            for child in children {
                if let Some(tone) = world.get::<bevy_gooey::IconTone>(child)
                    && *tone == bevy_gooey::IconTone::Accent
                {
                    lit.push(id);
                }
            }
        }
    }
    assert_eq!(
        lit,
        vec!["map.clone_mode"],
        "exactly the Clone button must be lit while Clone is armed; a one-shot command that lit up \
         would be lying about what its key did"
    );
}
