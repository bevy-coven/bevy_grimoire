//! **Cmd+B shows and hides the left panel, and Zen mode shows the world and nothing else** (feedback-1794146831022; oracle `spec`).
//!
//! The note, from the editor: *"We should have a toggle that toggles into "Zen" mode, that hides all the UI and just sees the game.
//! Also, Cmd+B should toggle the left panel, like VSCode."*
//!
//! The relations asserted are about what is laid out, which is what a person sees:
//!
//! - after `Cmd+B`, the left dock lays out to nothing and the viewport is wider by what the dock and its divider had; after the
//!   chord again, both are as they were;
//! - after `Shift+Cmd+M`, the menu bar, the chrome bar, the toolbar, the door's strip, both docks, the terminal and the status
//!   band lay out to nothing and **the viewport is the whole window**; after the chord again, every part is as it was;
//! - `Cmd+B` no longer arms the run tool, which is `Shift+Cmd+B` now (`Cmd+B` was Run's, and the two cannot share it).

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::chrome::{Frame, ViewportSlot};
use bevy_cauldron::editor::{EditorState, Tool};
use bevy_cauldron::menu_bar::EditorMenuBar;
use bevy_cauldron::shell_view::ShellView;
use bevy_cauldron::terminal_pane::TerminalPaneRoot;
use bevy_cauldron::toolbar::Toolbar;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.world_mut().resource_mut::<bevy_cauldron::terminal::Terminal>().set_collapsed(false); // #424: the pane starts collapsed; this test needs it raised
    bevy_cauldron::harness::resize_surface(&mut app, 1280, 720).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..12 {
        app.update();
    }
    app
}

/// The primary modifier's key on this build, as `keys::MOD_KEYS` names it.
fn primary() -> KeyCode {
    bevy_cauldron::keys::MOD_KEYS[0]
}

/// **A chord, pressed and let go**: every key down in one frame (so the modifiers are held as the last goes down), a frame, then up.
fn chord(app: &mut App, keys: &[KeyCode]) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in keys {
            let logical_key = match key {
                KeyCode::KeyB => Key::Character("b".into()),
                KeyCode::KeyM => Key::Character("m".into()),
                _ => Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            };
            app.world_mut().write_message(KeyboardInput { key_code: *key, logical_key, state, text: None, repeat: false, window: Entity::PLACEHOLDER });
        }
        for _ in 0..6 {
            app.update();
        }
    }
}

/// An entity's laid-out size in logical px, `Vec2::ZERO` when it is hidden.
fn size(app: &App, entity: Entity) -> Vec2 {
    app.world().get::<ComputedNode>(entity).map_or(Vec2::ZERO, |n| n.size() * n.inverse_scale_factor)
}

fn first<M: Component>(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<M>>().iter(world).next().unwrap_or_else(|| panic!("the editor has no {}", std::any::type_name::<M>()))
}

/// Every part Zen mode hides, with what it lays out to now.
fn parts(app: &mut App) -> Vec<(&'static str, Vec2)> {
    let frame = app.world().resource::<Frame>();
    let (chrome_bar, door_strip, right, status, left) = (frame.chrome_bar, frame.door_strip, frame.right, frame.status, frame.left);
    let (menu, toolbar, terminal) = (first::<EditorMenuBar>(app), first::<Toolbar>(app), first::<TerminalPaneRoot>(app));
    [
        ("menu bar", menu),
        ("chrome bar", chrome_bar),
        ("toolbar", toolbar),
        ("door strip", door_strip),
        ("left dock", left),
        ("right dock", right),
        ("terminal", terminal),
        ("status band", status),
    ]
    .into_iter()
    .map(|(name, entity)| (name, size(app, entity)))
    .collect()
}

fn viewport(app: &mut App) -> Vec2 {
    let entity = first::<ViewportSlot>(app);
    size(app, entity)
}

/// A frame's slack: layout is physical and rounds.
const SLACK_PX: f32 = 2.0;

#[test]
fn cmd_b_hides_the_left_panel_and_the_viewport_takes_the_room() {
    let mut app = editor("shell_view_left");
    let before = parts(&mut app);
    let left_before = before.iter().find(|(n, _)| *n == "left dock").map(|(_, s)| *s).expect("a left dock");
    let world_before = viewport(&mut app);
    assert!(left_before.x > 40.0, "the left dock is on screen to begin with: {left_before:?}");

    chord(&mut app, &[primary(), KeyCode::KeyB]);
    let left_hidden = parts(&mut app).into_iter().find(|(n, _)| *n == "left dock").map(|(_, s)| s).unwrap();
    let world_wider = viewport(&mut app);
    assert_eq!(left_hidden, Vec2::ZERO, "Cmd+B hid the left dock, and it still lays out as {left_hidden:?}");
    assert!(
        world_wider.x >= world_before.x + left_before.x - SLACK_PX,
        "the viewport was {world_before:?} beside a left dock of {left_before:?} and is {world_wider:?} without it: it did not take the room"
    );
    assert!(app.world().resource::<ShellView>().left_hidden);
    assert_ne!(app.world().resource::<EditorState>().tool, Tool::Run, "Cmd+B no longer arms the run tool");

    chord(&mut app, &[primary(), KeyCode::KeyB]);
    let after = parts(&mut app);
    let world_after = viewport(&mut app);
    for ((name, was), (_, is)) in before.iter().zip(after.iter()) {
        assert!((*was - *is).abs().max_element() <= SLACK_PX, "{name} was {was:?} and is {is:?} after Cmd+B twice: it did not come back");
    }
    assert!((world_before - world_after).abs().max_element() <= SLACK_PX, "the viewport was {world_before:?} and is {world_after:?}");
}

#[test]
fn shift_cmd_m_is_zen_mode_the_viewport_is_the_whole_window_and_the_chord_brings_it_back() {
    let mut app = editor("shell_view_zen");
    let before = parts(&mut app);
    let world_before = viewport(&mut app);
    for (name, size) in &before {
        assert!(size.x > 0.0 && size.y > 0.0, "{name} is on screen to begin with: {size:?}");
    }

    chord(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyM]);
    assert!(app.world().resource::<ShellView>().zen);
    for (name, size) in parts(&mut app) {
        assert_eq!(size, Vec2::ZERO, "Zen mode is on and the {name} still lays out as {size:?}");
    }
    let window = {
        let root = app.world().resource::<Frame>().root;
        size(&app, root)
    };
    let world_zen = viewport(&mut app);
    assert!(
        (world_zen - window).abs().max_element() <= SLACK_PX,
        "Zen mode: the viewport is {world_zen:?} in a window of {window:?}; it should be the whole window"
    );
    assert!(world_zen.x > world_before.x && world_zen.y > world_before.y, "the viewport grew in both directions: {world_before:?} to {world_zen:?}");

    chord(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyM]);
    assert!(!app.world().resource::<ShellView>().zen);
    let after = parts(&mut app);
    for ((name, was), (_, is)) in before.iter().zip(after.iter()) {
        assert!((*was - *is).abs().max_element() <= SLACK_PX, "{name} was {was:?} and is {is:?} after Zen mode ended: it did not come back");
    }
    assert!((viewport(&mut app) - world_before).abs().max_element() <= SLACK_PX, "and the viewport is what it was");
}

/// **The run tool moved to `Shift+Cmd+B`**, so the chord Cmd+B left it free.
#[test]
fn shift_cmd_b_arms_the_run_tool() {
    let mut app = editor("shell_view_run");
    assert_ne!(app.world().resource::<EditorState>().tool, Tool::Run);
    chord(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyB]);
    assert_eq!(app.world().resource::<EditorState>().tool, Tool::Run, "Shift+Cmd+B arms the run tool");
    assert!(!app.world().resource::<ShellView>().left_hidden, "and does not touch the left panel");
}

/// **A shell that has the keyboard keeps a Ctrl chord as its own, and Zen mode gives the keyboard back**: off a Mac the primary
/// chords are Ctrl chords, which a shell reads as control codes, so the chord must not toggle anything; on a Mac it is the
/// window's, Zen begins and the keys that move the camera stop reaching a shell nobody can see.
#[test]
fn zen_mode_gives_the_terminals_keyboard_back_and_a_shell_keeps_its_own_ctrl_chords() {
    use bevy_cauldron::terminal_pane::TerminalFocus;
    let mut app = editor("shell_view_focus");
    app.world_mut().resource_mut::<TerminalFocus>().take();
    let shells = app.world().resource::<TerminalFocus>().owns(&bevy_cauldron::shell_view::zen_chord());

    chord(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyM]);
    let zen = app.world().resource::<ShellView>().zen;
    assert_eq!(zen, !shells, "the shell owns the chord ({shells}) and Zen mode is {zen}");
    if zen {
        assert!(!app.world().resource::<TerminalFocus>().holds(), "Zen mode began and the hidden terminal still holds the keyboard");
    }
}
