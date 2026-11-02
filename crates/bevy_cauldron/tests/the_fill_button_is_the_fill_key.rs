//! **The Fill button and the `F` key are one command** (UI-37, #231).
//!
//! #231's acceptance is *"Clicking Fill arms the fill exactly as its key does"*. Slice 1 measured what
//! "fill" is: `Tool` has six variants and **no `Fill`** — `Action::Fill` is a one-shot flood
//! (`flood_from_cursor`, `editor.rs`), not a mode. So the toolbar's Fill button runs the same command
//! `F` runs, and this file holds down the two claims that makes:
//!
//! 1. **One body.** The button carries the command id `map.fill` and, when activated, hands it to
//!    `command_run::Chords` — the palette's queue — exactly as every other toolbar button does. There is
//!    no second flood path for a click, and `commands::by_id("map.fill")` is what both ends resolve to.
//! 2. **A fill that cannot start says why.** A click on the button leaves the pointer over the *button*,
//!    not over the ground, so `cursor_ground` finds nothing and the flood cannot begin. Before this
//!    slice that was a silent `return`: indistinguishable from a button that does nothing at all. It is
//!    now a note naming the gesture that works, which is the rule `build.rs` states as *"an arrow that
//!    answers nothing at all is the defect"*.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..5 {
        app.update();
    }
    (root, app)
}

/// **The Fill button is the `map.fill` command, and the key's binding for it exists.**
///
/// A button that named a different id, or an id the registry does not hold, could not dispatch what `F`
/// dispatches. This is the parity the acceptance asks for, checked at the one place both ends meet.
#[test]
fn the_fill_button_names_the_command_the_key_runs() {
    let (_root, mut app) = editor("fill-parity");
    let buttons: Vec<&'static str> = {
        let world = app.world_mut();
        let mut q = world.query::<&bevy_cauldron::toolbar::ToolbarCommand>();
        q.iter(world).map(|b| b.0).collect()
    };
    assert!(
        buttons.contains(&"map.fill"),
        "the toolbar must carry the fill command; it carries {buttons:?}"
    );
    let command = bevy_cauldron::commands::by_id("map.fill")
        .expect("`map.fill` is a registered command, so the key and the button resolve to one thing");
    let action = bevy_cauldron::keys::binding(command.action);
    assert!(
        !action.chord.is_empty(),
        "the command the Fill button dispatches must have a key of its own, or the acceptance's \
         \"exactly as its key does\" has no key to be exact about"
    );
}

/// **A fill that cannot start says why, rather than doing nothing.**
///
/// Driven through the editor's real systems with no camera: `flood_from_cursor` has no ground point to
/// flood from, which is the same state a toolbar click leaves the pointer in. The note must name the
/// map and the key, so a person who clicked the button learns the gesture that works.
#[test]
fn a_fill_with_no_ground_under_the_pointer_says_so() {
    let (_root, mut app) = editor("fill-no-ground");
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().status.note(String::new());

    let command = bevy_cauldron::commands::by_id("map.fill").expect("map.fill is a command");
    // **The binding is read before the queue is borrowed**: `Live` is a resource and `Chords` is too, and
    // holding one `ResMut` across a read of the other is the borrow error this line avoids.
    let live = *app.world().resource::<bevy_cauldron::keys::Live>();
    app.world_mut()
        .resource_mut::<bevy_cauldron::command_run::Chords>()
        .queue(&command, live);
    for _ in 0..4 {
        app.update();
    }

    let said = app
        .world()
        .resource::<bevy_cauldron::editor::EditorState>()
        .status
        .note_text()
        .to_owned();
    let problem = app
        .world()
        .resource::<bevy_cauldron::editor::EditorState>()
        .status
        .problem_text()
        .to_owned();
    assert!(
        said.contains("a fill starts from the map"),
        "a fill asked for with no ground under the pointer must say where a fill starts; it said \
         {said:?} (problem {problem:?}). Silence here is what a button that does nothing looks like"
    );
}
