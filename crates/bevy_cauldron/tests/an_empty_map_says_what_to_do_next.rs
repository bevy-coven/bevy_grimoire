//! **An empty map says what to do next** (UI-81, #255).
//!
//! A new map is a black rectangle with a grid in it. The overlay over its viewport says one sentence,
//! "Place a piece from the library", and offers one action, "Open library", which hands the library's
//! filter box the keyboard through the same chord `Cmd+F` is. It is there while the map has no
//! placements and gone the frame it has one.

use bevy::prelude::*;
use bevy_cauldron::empty_state::{ACTION, EmptyMapPrompt, SENTENCE};
use bevy_gooey::testing::{click, find_by_label};

use crate::fixtures;

fn editor(name: &str, with_a_piece: bool) -> App {
    let mut f = fixtures::Fixture::new(name).descriptor("site/wall", "alpha");
    if with_a_piece {
        f = f.place("site/wall", (0.0, 0.0));
    }
    let root = f.build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn shown(app: &mut App) -> Display {
    let world = app.world_mut();
    let mut q = world.query_filtered::<&Node, With<EmptyMapPrompt>>();
    q.single(world).expect("the editor has one empty-map prompt").display
}

/// **An empty map shows its sentence and its action.**
#[test]
fn an_empty_map_shows_the_prompt_and_its_action() {
    let mut app = editor("empty_map_prompt", false);
    assert_ne!(shown(&mut app), Display::None, "a map with no placements shows the prompt");
    assert!(
        find_by_label(&mut app, ACTION).is_ok(),
        "the prompt offers one button that says {ACTION:?}"
    );
    assert_eq!(SENTENCE, "Place a piece from the library");
}

/// **A map with a piece in it does not.**
#[test]
fn a_map_with_a_piece_hides_the_prompt() {
    let mut app = editor("empty_map_prompt_full", true);
    assert_eq!(shown(&mut app), Display::None, "a map that has a placement is not empty");
}

/// **The prompt follows the map**: emptied it comes, and the first placement takes it away again.
#[test]
fn the_prompt_follows_whether_the_map_has_a_piece() {
    let mut app = editor("empty_map_prompt_follows", true);
    assert_eq!(shown(&mut app), Display::None);
    let piece = {
        let mut open = app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>();
        std::mem::take(&mut open.map.placements)
    };
    assert!(!piece.is_empty(), "the fixture map had a piece to take away");
    app.update();
    assert_ne!(shown(&mut app), Display::None, "a map with nothing in it shows the prompt");
    app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>().map.placements = piece;
    app.update();
    assert_eq!(shown(&mut app), Display::None, "one placement and the map is no longer empty");
}

/// Every key the keyboard was told about, in order: what a queued chord becomes a frame or two later.
#[derive(Resource, Default)]
struct KeysSeen(Vec<KeyCode>);

fn watch_keys(mut seen: ResMut<KeysSeen>, mut keys: MessageReader<bevy::input::keyboard::KeyboardInput>) {
    for key in keys.read() {
        if key.state == bevy::input::ButtonState::Pressed {
            seen.0.push(key.key_code);
        }
    }
}

/// **The action is the filter's chord**: a click on it presses `Cmd+F` (the F, with the platform's
/// modifier), which is what `view.focus_filter` is bound to.
#[test]
fn the_action_asks_for_the_library_filter() {
    let mut app = editor("empty_map_prompt_action", false);
    app.init_resource::<KeysSeen>().add_systems(PreUpdate, watch_keys);
    let button = find_by_label(&mut app, ACTION).expect("the button");
    assert!(app.world().resource::<KeysSeen>().0.is_empty(), "no key is pressed before the click");
    click(&mut app, button);
    for _ in 0..6 {
        app.update();
    }
    let seen = &app.world().resource::<KeysSeen>().0;
    assert!(
        seen.contains(&KeyCode::KeyF),
        "a click on {ACTION:?} must press the library filter's chord (Cmd+F); the keys pressed were {seen:?}"
    );
}
