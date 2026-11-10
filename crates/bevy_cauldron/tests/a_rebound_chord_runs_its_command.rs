//! **The key dispatcher follows the keymap** (UI-72b, #265).
//!
//! Rebinding Save to Ctrl+Shift+Y in `settings.ron` makes Ctrl+Shift+Y save and takes Ctrl+S's save
//! away, in the running editor; a command the file leaves out keeps its default chord; and deleting the
//! entry gives the default chord back. The chord travels with the app (`keys::Live`), not in a static,
//! so two apps in one process can disagree.

use std::path::{Path, PathBuf};

use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::{ButtonInput, ButtonState};
use bevy::prelude::*;
use bevy_cauldron::keys::{self, Action, Live};
use bevy_cauldron::settings::{SettingsLayer, UserSettingsFile};
use bevy_gooey::{Modifiers, Shortcut};

use crate::fixtures;

fn rig(tag: &str, rebound: &[(&str, Shortcut)]) -> (App, PathBuf, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let user = root.join(format!("user_config_{tag}_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    write_keymap(&user, rebound);
    app.insert_resource(UserSettingsFile(user.clone()));
    for _ in 0..6 {
        app.update();
    }
    (app, root, user)
}

fn write_keymap(user: &Path, rebound: &[(&str, Shortcut)]) {
    let mut layer = SettingsLayer::default();
    for (id, chord) in rebound {
        layer.keymap.insert((*id).to_owned(), *chord);
    }
    std::fs::write(user, bevy_grimoire_core::yaml::to_string(&layer).unwrap()).unwrap();
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// How many times the map has been saved: one `map_save` line per save in the project's op log.
fn saves(root: &Path) -> usize {
    std::fs::read_to_string(bevy_cauldron::ops::oplog_path_at(root))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.contains("\"map_save\""))
        .count()
}

/// Send the keys as the keyboard does: messages, which Bevy's input system turns into `ButtonInput`
/// (a press set on the resource itself is cleared at the start of the frame).
fn send(app: &mut App, keys: &[KeyCode], state: ButtonState) {
    for key in keys {
        app.world_mut().write_message(KeyboardInput {
            key_code: *key,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
}

/// Hold `modifiers`, press `key` for one frame, let everything go.
fn chord(app: &mut App, modifiers: &[KeyCode], key: KeyCode) {
    let down: Vec<KeyCode> = modifiers.iter().copied().chain([key]).collect();
    send(app, &down, ButtonState::Pressed);
    app.update();
    send(app, &down, ButtonState::Released);
    updates(app, 2);
}

fn ctrl_shift_y() -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyY)
}

#[test]
fn a_rebound_save_answers_to_its_new_chord_and_not_the_old_one() {
    let (mut app, root, _user) = rig("rebound_save", &[("file.save", ctrl_shift_y())]);
    let primary = keys::MOD_KEYS[0];

    let before = saves(&root);
    chord(&mut app, &[primary], KeyCode::KeyS);
    assert_eq!(saves(&root), before, "the old chord no longer saves once Save has moved");

    chord(&mut app, &[primary, KeyCode::ShiftLeft], KeyCode::KeyY);
    assert_eq!(saves(&root), before + 1, "the new chord saves");
}

#[test]
fn a_command_the_file_leaves_out_keeps_its_default_chord() {
    let (app, root, _user) = rig("rebound_other", &[("file.save", ctrl_shift_y())]);
    let live = *app.world().resource::<Live>();
    let input = {
        let mut input = ButtonInput::<KeyCode>::default();
        input.press(keys::MOD_KEYS[0]);
        input.press(KeyCode::KeyZ);
        input
    };
    assert!(keys::just_pressed(&input, live, Action::Undo), "Undo was not rebound, so Ctrl+Z still undoes");
    assert_eq!(saves(&root), 0);
}

#[test]
fn deleting_the_entry_gives_the_default_chord_back() {
    let (mut app, root, user) = rig("rebound_removed", &[("file.save", ctrl_shift_y())]);
    let primary = keys::MOD_KEYS[0];
    write_keymap(&user, &[]);
    updates(&mut app, 4);

    let before = saves(&root);
    chord(&mut app, &[primary], KeyCode::KeyS);
    assert_eq!(saves(&root), before + 1, "Ctrl+S saves again");
}

#[test]
fn the_rebinding_belongs_to_the_app_that_read_it() {
    let (rebound_app, _r1, _u1) = rig("rebound_mine", &[("file.save", ctrl_shift_y())]);
    let (plain_app, _r2, _u2) = rig("rebound_yours", &[]);
    let mut chord_down = ButtonInput::<KeyCode>::default();
    chord_down.press(keys::MOD_KEYS[0]);
    chord_down.press(KeyCode::KeyS);
    let mine = *rebound_app.world().resource::<Live>();
    let yours = *plain_app.world().resource::<Live>();
    assert!(!keys::just_pressed(&chord_down, mine, Action::Save), "Ctrl+S is not Save in the app that moved it");
    assert!(keys::just_pressed(&chord_down, yours, Action::Save), "and still is in the one that did not");
}

#[test]
fn the_palette_and_the_badges_name_the_rebound_chord() {
    let (app, _root, _user) = rig("rebound_named", &[("file.save", ctrl_shift_y())]);
    let live = *app.world().resource::<Live>();

    let rows = bevy_cauldron::command_palette::list(live);
    let save = rows.iter().find(|r| r.id == "file.save").expect("Save is in the palette");
    assert_eq!(save.shortcut, Some(ctrl_shift_y()), "the palette shows the chord Save answers to");
    let undo = rows.iter().find(|r| r.id == "edit.undo").expect("Undo is in the palette");
    assert_eq!(undo.shortcut, Some(bevy_cauldron::commands::command(Action::Undo).shortcut), "an unmoved command shows its default");

    let badge_for_save = |rebound: &keys::Rebound| {
        keys::badges(keys::Context::Global, keys::Stance::Idle)
            .into_iter()
            .filter(|b| b.actions.contains(&Action::Save))
            .map(|b| b.rebound(rebound).chord)
            .next()
            .expect("Save has a badge")
    };
    assert_eq!(badge_for_save(&live.3), ctrl_shift_y().label(), "the badge says the new chord");
    assert_ne!(badge_for_save(&live.3), badge_for_save(&keys::Rebound::NONE), "and not the default");
}

#[test]
fn run_command_presses_the_rebound_chord() {
    let (mut app, root, _user) = rig("rebound_run", &[("file.save", ctrl_shift_y())]);
    let live = *app.world().resource::<Live>();
    let save = bevy_cauldron::commands::by_id("file.save").expect("a command");
    app.world_mut().resource_mut::<bevy_cauldron::command_run::Chords>().queue(&save, live);
    updates(&mut app, 6);
    assert_eq!(saves(&root), 1, "an agent's run_command goes down as the chord the dispatcher now answers to");
}

#[test]
fn a_rebound_chord_is_not_also_a_census_chord_on_the_same_key() {
    // Save moves to Alt+W; `W` alone pans the view forward, and the census's bare rows do not look at Alt.
    let alt_w = Shortcut::new(Modifiers::ALT, KeyCode::KeyW);
    let (app, _root, _user) = rig("rebound_shadow", &[("file.save", alt_w)]);
    let live = *app.world().resource::<Live>();
    let mut input = ButtonInput::<KeyCode>::default();
    input.press(KeyCode::AltLeft);
    input.press(KeyCode::KeyW);
    assert!(keys::just_pressed(&input, live, Action::Save), "Alt+W saves");
    assert!(!keys::just_pressed(&input, live, Action::PanForward), "and does not also pan");
    assert!(!keys::pressed(&input, live, Action::PanForward), "or hold a pan");

    let mut bare = ButtonInput::<KeyCode>::default();
    bare.press(KeyCode::KeyW);
    assert!(keys::just_pressed(&bare, live, Action::PanForward), "W alone still pans");
    assert!(!keys::just_pressed(&bare, live, Action::Save), "and does not save");
}
