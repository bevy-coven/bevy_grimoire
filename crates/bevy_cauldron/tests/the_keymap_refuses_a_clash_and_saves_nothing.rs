//! **The keymap screen rebinds a command and refuses a chord another command has** (UI-72, #250).
//!
//! Binding a second command to Ctrl+S, which Save has, shows the clash and writes no file; binding Save
//! elsewhere first frees the chord, and the same rebinding then lands in the file as a `keymap` entry keyed
//! by the command's stable id.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_cauldron::commands;
use bevy_cauldron::keymap_screen::{OpenKeymap, overlaps, taken_by};
use bevy_cauldron::keys::Context;
use bevy_cauldron::settings::{SettingsLayer, UserSettingsFile};
use bevy_gooey::testing::find_by_label;
use bevy_gooey::{Binding, Modifiers, Shortcut};

use crate::fixtures;

fn rig(tag: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let user = root.join(format!("user_config_{tag}_{}/settings.ron", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    app.insert_resource(UserSettingsFile(user.clone()));
    for _ in 0..3 {
        app.update();
    }
    app.world_mut().write_message(OpenKeymap);
    for _ in 0..4 {
        app.update();
    }
    (app, user)
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// The person pressed `chord` on the row named `title`.
fn press_chord_on(app: &mut App, title: &str, chord: Shortcut) {
    let row = find_by_label(app, title).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: row, value: Binding::Chord(chord), is_final: true });
    updates(app, 3);
}

fn narrow_to(app: &mut App, words: &str) {
    let row = app.world_mut().query_filtered::<Entity, With<bevy_gooey::TextHint>>().iter(app.world()).next().expect("the search row");
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: row, value: words.to_owned(), is_final: false });
    updates(app, 3);
}

fn said(app: &mut App) -> Vec<String> {
    let mut texts = app.world_mut().query::<&Text>();
    texts.iter(app.world()).map(|t| t.0.clone()).collect()
}

fn ctrl_s() -> Shortcut {
    Shortcut::primary(KeyCode::KeyS)
}

#[test]
fn a_second_command_on_ctrl_s_shows_the_clash_and_saves_nothing_until_save_moves() {
    let (mut app, user) = rig("keymap_clash");
    narrow_to(&mut app, "undo");
    press_chord_on(&mut app, "Undo", ctrl_s());
    assert!(!user.exists(), "a refused rebinding writes no file");
    let lines = said(&mut app);
    assert!(lines.iter().any(|t| t.contains(&ctrl_s().label()) && t.contains("already Save") && t.contains("Nothing was saved")), "the screen names the clash: {lines:?}");

    // The way through: Save goes elsewhere first.
    narrow_to(&mut app, "save");
    let elsewhere = Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyY);
    press_chord_on(&mut app, "Save", elsewhere);
    let layer = SettingsLayer::read("test", &std::fs::read_to_string(&user).expect("Save's new chord was written")).unwrap();
    assert_eq!(layer.keymap.get("file.save"), Some(&elsewhere), "keyed by the command's stable id");

    narrow_to(&mut app, "undo");
    press_chord_on(&mut app, "Undo", ctrl_s());
    let layer = SettingsLayer::read("test", &std::fs::read_to_string(&user).unwrap()).unwrap();
    assert_eq!(layer.keymap.get("edit.undo"), Some(&ctrl_s()), "with Ctrl+S free, the same rebinding lands");
    assert_eq!(layer.keymap.get("file.save"), Some(&elsewhere), "and Save keeps its new chord");
}

#[test]
fn reset_and_rebinding_to_the_default_take_the_entry_out_of_the_file() {
    let (mut app, user) = rig("keymap_reset");
    narrow_to(&mut app, "redo");
    let chord = Shortcut::new(Modifiers::PRIMARY | Modifiers::ALT, KeyCode::KeyJ);
    press_chord_on(&mut app, "Redo", chord);
    assert!(SettingsLayer::read("t", &std::fs::read_to_string(&user).unwrap()).unwrap().keymap.contains_key("edit.redo"));
    assert!(find_by_label(&mut app, "Reset Redo").is_ok(), "a rebound command offers Reset");
    let reset = find_by_label(&mut app, "Reset Redo").unwrap();
    bevy_gooey::testing::click(&mut app, reset);
    updates(&mut app, 3);
    assert!(SettingsLayer::read("t", &std::fs::read_to_string(&user).unwrap()).unwrap().keymap.is_empty(), "Reset leaves no entry");

    press_chord_on(&mut app, "Redo", chord);
    let default = commands::by_id("edit.redo").unwrap().shortcut;
    press_chord_on(&mut app, "Redo", default);
    assert!(SettingsLayer::read("t", &std::fs::read_to_string(&user).unwrap()).unwrap().keymap.is_empty(), "binding it back to its default is no rebinding");
}

#[test]
fn a_keymap_entry_for_no_command_or_with_no_key_is_refused_by_name() {
    let good = "(version: 1, keymap: {\"file.save\": (modifiers: (ctrl: true, shift: false, alt: false, meta: false), key: KeyY)})";
    assert!(SettingsLayer::read("settings.ron", good).is_ok(), "a rebinding reads");
    let unknown = good.replace("file.save", "file.teleport");
    let r = SettingsLayer::read("settings.ron", &unknown).unwrap_err();
    assert_eq!(r.field.as_deref(), Some("keymap.file.teleport"), "{}", r.reason);
    // Resolution is per command, the project over the user.
    let user = SettingsLayer::read("u", good).unwrap();
    let project = SettingsLayer::read("p", &good.replace("KeyY", "KeyU")).unwrap();
    let both = bevy_cauldron::settings::EditorSettings::resolve(Some(&user), Some(&project));
    assert_eq!(both.keymap["file.save"].key, KeyCode::KeyU, "the project's chord wins for that command");
}

#[test]
fn commands_clash_only_where_both_can_fire() {
    assert!(overlaps(Context::Global, Context::Map) && overlaps(Context::Map, Context::Map));
    assert!(!overlaps(Context::Map, Context::Tiles), "two tabs may share a key");
    let none = Default::default();
    assert!(taken_by(&none, "edit.undo", ctrl_s()).iter().any(|c| c.id == "file.save"));
    assert!(taken_by(&none, "file.save", ctrl_s()).is_empty(), "a command never clashes with itself");
}
