//! **Shift-click selects pieces; it does not pick them up** (#236 slice 2, #89's "Shift-click pieces").
//!
//! With a clear cursor a click on a piece carries it (`drive_move`). Shift on the *drop* is the finer
//! snap, so only the grab is read: a Shift-click on a piece becomes the primary of the selection when
//! nothing is locked, joins it when something is, and leaves it again on a second Shift-click — and
//! never puts the piece in the hand. A plain click still carries, so the old gesture is unchanged.
//!
//! The person route, headless, the way `a_person_shift_clicks_two_pieces_into_one_note.rs` injects it.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::{Held, MoveDrag};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::selection::Selection;
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let frame = script.next.take();
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some(frame) = frame else {
        return;
    };
    for key in &frame.keys {
        keys.press(*key);
    }
    script.held = frame.keys;
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(frame.pointer);
    }
    match frame.mouse {
        Some(true) => mouse.press(MouseButton::Left),
        Some(false) => mouse.release(MouseButton::Left),
        None => {}
    }
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app.world_mut().query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(app.world()).expect("the editor has one main camera");
    camera
        .world_to_viewport(transform, world)
        .unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

fn click(app: &mut App, at: Vec2, keys: &[KeyCode]) {
    for mouse in [true, false] {
        step(app, Frame { keys: keys.to_vec(), pointer: Some(at), mouse: Some(mouse) });
    }
}

fn selected(app: &App) -> Vec<String> {
    app.world().resource::<Selection>().ids().to_vec()
}

fn held(app: &App) -> Option<Held> {
    app.world().resource::<MoveDrag>().held.clone()
}

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 5.0))
        .place("site/wall", (6.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    // **A clear cursor**, which is the state a click carries a piece in: the fixture arms its one
    // descriptor as the brush, and a person clears it with Esc.
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().brush = None;
    app.update();
    app
}

/// **The first Shift-click makes the primary, the second adds beside it, the third takes it out** — and
/// not one of them carries a piece.
#[test]
fn shift_click_builds_a_selection_and_carries_nothing() {
    let mut app = editor("shift_click_selection");
    let (a, b) = (pixel_over(&mut app, (0.0, 0.0)), pixel_over(&mut app, (6.0, 0.0)));
    let shift = [KeyCode::ShiftLeft];

    click(&mut app, a, &shift);
    assert_eq!(selected(&app), ["site/wall@0"], "nothing was locked, so the piece is the primary");
    assert_eq!(held(&app), None, "a Shift-click selects, it does not pick the piece up");

    click(&mut app, b, &shift);
    assert_eq!(selected(&app), ["site/wall@0", "site/wall@2"], "the second joins beside the primary");
    assert_eq!(held(&app), None);

    click(&mut app, b, &shift);
    assert_eq!(selected(&app), ["site/wall@0"], "a second Shift-click on it takes it out again");

    click(&mut app, a, &shift);
    assert_eq!(selected(&app), ["site/wall@0"], "the primary is not toggled away");
}

/// **A plain click still carries**, so the gesture this editor already had is unchanged.
#[test]
fn a_plain_click_still_picks_the_piece_up() {
    let mut app = editor("shift_click_plain");
    let a = pixel_over(&mut app, (0.0, 0.0));
    click(&mut app, a, &[]);
    assert_eq!(held(&app), Some(Held::Piece("site/wall@0".into())), "an unmodified click carries");
    assert!(selected(&app).is_empty(), "and selects nothing: the set is Shift-click's");
}
