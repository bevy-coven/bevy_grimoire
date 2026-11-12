//! **The four `view.*` navigation bindings are gone, and nine `workspace.*` ones replaced them** (NV-02, #379).
//!
//! The issue adds nine bindings (`Cmd`/`Ctrl`+`1` to `9`) and removes four (`view.switch_door`,
//! `view.panel_1` to `view.panel_3`), which is the net +5 the key budget in `keys.rs` is raised by. This
//! file measures the removal from outside the crate:
//!
//! 1. the four ids name no command and no action, and the nine workspace commands are there;
//! 2. the bare keys they sat on (`=`, `1`, `2`, `3`) move nothing: the workspace and the panel stay put;
//! 3. **a person's saved keymap that names one of them still reads** (the file is not refused), and the
//!    retired entry binds nothing.

use std::path::PathBuf;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;

use bevy_cauldron::commands;
use bevy_cauldron::harness;
use bevy_cauldron::keys::{Action, binding};
use bevy_cauldron::settings::{SETTINGS_VERSION, SettingsLayer};
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::{Modifiers, Shortcut};

use crate::fixtures;

/// The four commands the workspaces replaced, as a saved keymap spells them.
const RETIRED: [&str; 4] = ["view.switch_door", "view.panel_1", "view.panel_2", "view.panel_3"];

/// Frames a workspace switch and its panels need.
const SETTLE: usize = 12;

fn editor(tag: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE {
        app.update();
    }
    (app, root)
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

/// **Press one bare key, the way the keyboard does**: down, a frame, up, a frame.
fn press(app: &mut App, key: KeyCode) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
    settle(app);
}

#[test]
fn the_four_view_ids_name_no_command_and_the_nine_workspace_ids_do() {
    for id in RETIRED {
        assert!(commands::by_id(id).is_none(), "`{id}` still names a command: the workspaces replaced it");
        assert!(
            Action::ALL.into_iter().all(|a| commands::identity(a).0 != id),
            "an action still carries the retired id `{id}`"
        );
    }
    for workspace in Workspace::ALL {
        let id = format!("workspace.{}", workspace.id());
        let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
        assert_eq!(command.action.workspace(), Some(workspace), "`{id}` goes somewhere else");
        let chord = binding(command.action);
        assert!(chord.needs_mod, "`{id}` is Cmd/Ctrl + a digit, never a bare digit");
    }
}

#[test]
fn the_bare_keys_the_four_sat_on_move_nothing() {
    let (mut app, _root) = editor("view_keys_bare");
    let kit = commands::by_id("workspace.kit").unwrap_or_else(|| panic!("workspace.kit"));
    let live = *app.world().resource::<bevy_cauldron::keys::Live>();
    app.world_mut().resource_mut::<bevy_cauldron::command_run::Chords>().queue(&kit, live);
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit);
    let panel = *app.world().resource::<Mode>();

    press(&mut app, KeyCode::Equal);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit, "`=` still crosses to the other workspace");
    for key in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3] {
        press(&mut app, key);
        assert_eq!(app.world().resource::<Mode>().label(), panel.label(), "{key:?} still opens a panel of the workspace");
        assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit, "{key:?} changed the workspace");
    }
}

#[test]
fn a_saved_keymap_that_names_a_retired_command_still_reads_and_binds_nothing() {
    let chord = ron::ser::to_string(&Shortcut::new(Modifiers::NONE, KeyCode::Backquote)).unwrap_or_else(|e| panic!("{e}"));
    for id in RETIRED {
        let text = format!("(version: {SETTINGS_VERSION}, keymap: {{\"{id}\": {chord}}})");
        let layer = SettingsLayer::read("old/settings.ron", &text)
            .unwrap_or_else(|r| panic!("a keymap that rebound `{id}` was refused: {}: {}", r.field.unwrap_or_default(), r.reason));
        assert!(!layer.keymap.contains_key(id), "`{id}` names no command, so the entry is dropped rather than kept");
    }
    // **A retired entry does not hide a real one**: the file's other rebinding survives the read.
    let text = format!("(version: {SETTINGS_VERSION}, keymap: {{\"view.switch_door\": {chord}, \"file.save\": {chord}}})");
    let layer = SettingsLayer::read("old/settings.ron", &text).unwrap_or_else(|r| panic!("{}", r.reason));
    assert!(layer.keymap.contains_key("file.save"));
    assert_eq!(layer.keymap.len(), 1);
}
