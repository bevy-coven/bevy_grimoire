//! **Ctrl+Shift+P opens a palette over every command; typing "undo" and Enter runs `edit.undo` once**
//! (UI-32, #226).
//!
//! The palette is `bevy_gooey`'s `command_palette`, filled from the one command registry
//! (`commands.rs`), and a chosen command runs by pressing its chord, so the dispatcher that answers is
//! the one Cmd+Z goes through. The assertions are on what a person sees change: the map, the undo and
//! redo stacks, the palette's rows and whether it is open.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_grimoire_mapper::commands;
use bevy_grimoire_mapper::editor::EditorState;
use bevy_grimoire_mapper::project::OpenMap;

use bevy_gooey::{CommandList, CommandPalette, PaletteHistory};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window")
}

fn send(app: &mut App, key_code: KeyCode, logical_key: Key, text: Option<&str>, state: ButtonState) {
    let window = window(app);
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key,
        state,
        text: text.map(Into::into),
        repeat: false,
        window,
    });
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

/// **A chord as a hand makes it**: every key down together, a frame, every key up, a frame.
fn chord(app: &mut App, keys: &[KeyCode]) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in keys {
            send(app, *key, Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified), None, state);
        }
        app.update();
    }
    settle(app);
}

fn ctrl_shift_p(app: &mut App) {
    chord(app, &[KeyCode::ControlLeft, KeyCode::ShiftLeft, KeyCode::KeyP]);
}

fn ctrl_k(app: &mut App) {
    chord(app, &[KeyCode::ControlLeft, KeyCode::KeyK]);
}

fn key_for(c: char) -> KeyCode {
    match c {
        'u' => KeyCode::KeyU,
        'n' => KeyCode::KeyN,
        'd' => KeyCode::KeyD,
        'o' => KeyCode::KeyO,
        other => panic!("no key for `{other}`"),
    }
}

fn type_str(app: &mut App, text: &str) {
    for c in text.chars() {
        let s = c.to_string();
        for state in [ButtonState::Pressed, ButtonState::Released] {
            let typed = (state == ButtonState::Pressed).then_some(s.as_str());
            send(app, key_for(c), Key::Character(s.as_str().into()), typed, state);
            app.update();
        }
    }
    settle(app);
}

fn enter(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, KeyCode::Enter, Key::Enter, None, state);
        app.update();
    }
    settle(app);
}

fn open(app: &mut App) -> bool {
    let mut q = app.world_mut().query_filtered::<Entity, With<CommandPalette>>();
    q.iter(app.world()).next().is_some()
}

fn placements(app: &App) -> usize {
    app.world().resource::<OpenMap>().map.placements.len()
}

fn lay_a_run(app: &mut App, from: (f32, f32), to: (f32, f32)) {
    bevy_grimoire_mapper::editor::run_for_test(app.world_mut(), "fence", from, to);
    settle(app);
}

fn undo_depth(app: &App) -> usize {
    app.world().resource::<EditorState>().undo_depth()
}

fn redo_depth(app: &App) -> usize {
    app.world().resource::<EditorState>().redo_depth()
}

/// **The acceptance.** Two runs are laid, so "once" means one of them goes and not both; the palette
/// opens on Ctrl+Shift+P, "undo" and Enter run `edit.undo`, and the palette is gone.
#[test]
fn typing_undo_and_enter_runs_edit_undo_once() {
    let mut app = editor("palette_undo");
    lay_a_run(&mut app, (0.0, 0.0), (8.0, 0.0));
    let one_run = placements(&app);
    lay_a_run(&mut app, (0.0, 4.0), (8.0, 4.0));
    assert!(placements(&app) > one_run, "the second run laid nothing");
    let (laid, undo_before) = (placements(&app), undo_depth(&app));
    assert_eq!(redo_depth(&app), 0);

    ctrl_shift_p(&mut app);
    assert!(open(&mut app), "Ctrl+Shift+P opens the palette");
    type_str(&mut app, "undo");
    enter(&mut app);

    assert!(!open(&mut app), "choosing a command closes the palette");
    assert_eq!(placements(&app), one_run, "the second run is taken back, and only it");
    assert!(placements(&app) < laid);
    assert_eq!(undo_depth(&app), undo_before - 1, "one undo step");
    assert_eq!(redo_depth(&app), 1, "`edit.undo` ran once: one step is waiting to be redone");
    assert_eq!(
        app.world().resource::<PaletteHistory>().0,
        ["edit.undo"],
        "what ran comes first the next time"
    );
}

/// **Ctrl+K opens it too, and the same chord closes it** (the person who opened it by mistake).
#[test]
fn ctrl_k_opens_the_palette_and_ctrl_k_again_closes_it() {
    let mut app = editor("palette_ctrl_k");
    assert!(!open(&mut app));
    ctrl_k(&mut app);
    assert!(open(&mut app), "Ctrl+K opens the palette");
    ctrl_k(&mut app);
    assert!(!open(&mut app), "Ctrl+K again closes it");
}

/// **The palette lists the registry, with the registry's chords and reasons**, judged in the context
/// the author was in when it opened, not the `Typing` the palette's own field puts the editor in.
#[test]
fn the_palette_lists_every_command_with_its_chord_and_the_reason_it_cannot_run() {
    let mut app = editor("palette_rows");
    ctrl_shift_p(&mut app);
    let list = app.world().resource::<CommandList>().0.clone();
    assert_eq!(list.len(), commands::all().len(), "one row per registered command");

    let undo = list.iter().find(|c| c.id == "edit.undo").expect("`edit.undo` is listed");
    assert_eq!(undo.label(), "Edit: Undo");
    assert_eq!(
        undo.shortcut.map(|s| s.label()),
        Some(commands::by_id("edit.undo").unwrap().shortcut_label()),
        "the row carries the command's own chord"
    );
    assert!(undo.enabled.is_ok(), "Undo can run on the Map tab: {:?}", undo.enabled);

    let tiles = list.iter().find(|c| c.id == "tiles.undo").expect("`tiles.undo` is listed");
    assert_eq!(
        tiles.enabled,
        Err("Not on this tab".to_owned()),
        "a Tiles command is listed on the Map tab with the registry's reason"
    );
}
