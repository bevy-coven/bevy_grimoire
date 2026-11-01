//! **A person snips an area of the editor, types a note, and Enter saves it.**
//!
//! `Cmd+\` arms the snip. A press and a drag over the window mark a rectangle, and the release
//! asks for a one-line note. Enter commits; Escape cancels and writes nothing. The keys and the
//! pointer are injected the way `a_person_removes_a_piece_and_undoes_it` injects them.
//!
//! **What a headless app can prove, and what it cannot.** A headless app has no render world, so
//! no screenshot can be read back and no PNG can be written. The gesture, the pending record, the
//! typing guard and the refusal are proved here. The bytes on disk are proved by the crate's own
//! test (`bevy_grimoire_snip`), which writes a real PNG and reads the record back, and by a live run.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::{EditorState, Tool};
use bevy_cauldron::harness;
use bevy_cauldron::keys::MOD_KEYS;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::snip::SnipEdit;
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

/// One printable character, typed as a person types it: the key goes down (so the keyboard census
/// sees it) and the character arrives as a keyboard message (so the note field reads it).
fn type_char(app: &mut App, ch: char, code: KeyCode) {
    step(
        app,
        Frame {
            keys: vec![code],
            ..default()
        },
    );
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: Key::Character(ch.to_string().into()),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    step(
        app,
        Frame {
            keys: vec![code],
            ..default()
        },
    );
    for state in [
        bevy::input::ButtonState::Pressed,
        bevy::input::ButtonState::Released,
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app
        .world_mut()
        .query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras
        .single(app.world())
        .expect("the editor has one main camera");
    camera
        .world_to_viewport(transform, world)
        .unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// The snip folder under the project. Nothing is written there headless, and the test says so.
fn snips_dir(app: &App) -> std::path::PathBuf {
    app.world()
        .resource::<Project>()
        .root
        .join("target/loop/captures/snips")
}

fn files_in(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| entries.count())
        .unwrap_or(0)
}

/// Arm the snip and drag a rectangle over the two pieces. Returns the rectangle in window pixels.
fn arm_and_drag(app: &mut App) -> [f32; 4] {
    let first = pixel_over(app, (0.0, 0.0));
    let last = pixel_over(app, (3.0, 0.0));
    let start = first - Vec2::new(60.0, 60.0);
    let end = last + Vec2::new(60.0, 60.0);

    step(
        app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::Backslash],
            ..default()
        },
    );
    assert!(
        app.world().resource::<SnipEdit>().armed,
        "Cmd+\\ must arm the snip"
    );

    step(
        app,
        Frame {
            pointer: Some(start),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        app,
        Frame {
            pointer: Some(end),
            ..default()
        },
    );
    step(
        app,
        Frame {
            pointer: Some(end),
            mouse: Some(false),
            ..default()
        },
    );
    [start.x, start.y, end.x, end.y]
}

#[test]
fn a_person_snips_an_area_and_types_a_note_and_enter_asks_to_save_it() {
    let mut app = editor("snip_gesture");
    let rect = arm_and_drag(&mut app);

    let pending = app
        .world()
        .resource::<SnipEdit>()
        .pending
        .clone()
        .expect("the release must leave a snip waiting for its note");
    assert!(
        (pending.rect[0] - rect[0]).abs() < 1.0
            && (pending.rect[1] - rect[1]).abs() < 1.0
            && (pending.rect[2] - rect[2]).abs() < 1.0
            && (pending.rect[3] - rect[3]).abs() < 1.0,
        "the snip must hold the dragged rectangle: {pending:?} vs {rect:?}"
    );
    assert_eq!(pending.note, "", "the note starts empty");

    // **Typing in the note is typing, not tools.** `X` arms removal mode everywhere else.
    type_char(&mut app, 'x', KeyCode::KeyX);
    assert!(
        !matches!(app.world().resource::<EditorState>().tool, Tool::Remove),
        "a letter typed into the note must not arm removal mode"
    );
    assert_eq!(
        app.world()
            .resource::<SnipEdit>()
            .pending
            .as_ref()
            .map(|p| p.note.as_str()),
        Some("x"),
        "the letter goes into the note"
    );

    // **Enter commits.** Headless there is no render world, so nothing can be written: the
    // refusal goes in `Status::problem`, and no snip folder is created.
    tap(&mut app, Key::Enter, KeyCode::Enter);
    assert!(
        app.world().resource::<SnipEdit>().pending.is_none(),
        "Enter must end the snip"
    );
    let problem = app.world().resource::<EditorState>().status.problem_text();
    assert!(
        problem.contains("render"),
        "a headless snip must be refused with a reason in the problem slot, got: {problem:?}"
    );
    assert_eq!(
        files_in(&snips_dir(&app)),
        0,
        "a refused snip writes no file"
    );
}

#[test]
fn a_person_cancels_a_snip_with_escape_and_nothing_is_written() {
    let mut app = editor("snip_cancel");
    arm_and_drag(&mut app);
    type_char(&mut app, 'q', KeyCode::KeyQ);
    tap(&mut app, Key::Escape, KeyCode::Escape);

    assert!(
        app.world().resource::<SnipEdit>().pending.is_none(),
        "Escape must drop the snip"
    );
    assert!(
        !app.world().resource::<SnipEdit>().armed,
        "Escape must leave the snip disarmed"
    );
    assert_eq!(
        files_in(&snips_dir(&app)),
        0,
        "a cancelled snip writes no file"
    );
}

fn placed_count(app: &App) -> usize {
    app.world().resource::<OpenMap>().map.placements.len()
}

/// Arm the snip and drag from `start` to `end`, the way a person does. Returns with the release done.
fn drag_snip(app: &mut App, start: Vec2, end: Vec2) {
    step(
        app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::Backslash],
            ..default()
        },
    );
    step(
        app,
        Frame {
            pointer: Some(start),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        app,
        Frame {
            pointer: Some(end),
            ..default()
        },
    );
    step(
        app,
        Frame {
            pointer: Some(end),
            mouse: Some(false),
            ..default()
        },
    );
}

/// **A snip is a capture, so its drag belongs to the snip and never to the piece tool under it.**
/// Place mode commits a piece on a click; a drag that is really a snip must place nothing.
#[test]
fn a_snip_drag_in_place_mode_places_nothing() {
    let mut app = editor("snip_place_gate");
    let before = placed_count(&app);
    let start = pixel_over(&mut app, (6.0, 0.0)) - Vec2::new(40.0, 40.0);
    let end = pixel_over(&mut app, (9.0, 0.0)) + Vec2::new(40.0, 40.0);
    drag_snip(&mut app, start, end);

    assert!(
        app.world().resource::<SnipEdit>().pending.is_some(),
        "the drag must still have been taken as a snip"
    );
    assert_eq!(
        placed_count(&app),
        before,
        "a snip drag in Place mode placed a piece"
    );
}

/// **A snip drag that starts on a piece in Remove mode removes nothing.** The removal box would
/// otherwise delete everything the snip covers.
#[test]
fn a_snip_drag_in_remove_mode_removes_nothing() {
    let mut app = editor("snip_remove_gate");
    app.world_mut().resource_mut::<EditorState>().tool = Tool::Remove;
    let before = placed_count(&app);
    let start = pixel_over(&mut app, (0.0, 0.0)) - Vec2::new(60.0, 60.0);
    let end = pixel_over(&mut app, (3.0, 0.0)) + Vec2::new(60.0, 60.0);
    drag_snip(&mut app, start, end);

    assert!(
        app.world().resource::<SnipEdit>().pending.is_some(),
        "the drag must still have been taken as a snip"
    );
    assert_eq!(
        placed_count(&app),
        before,
        "a snip drag in Remove mode removed a piece"
    );
}

/// **The drag's rectangle is drawn where it was dragged, at the window's real scale.**
///
/// The window is at scale 2, so the pointer is in surface pixels (physical), and the layout multiplies
/// `Val::Px` by `UiScale`. The overlay must divide by `UiScale` or it is drawn larger than the drag.
/// Measured in the real layout: `ComputedNode` is physical, so its size is the drag's size in surface
/// pixels and its centre is the drag's centre.
#[test]
fn the_drag_rectangle_is_drawn_where_it_was_dragged_at_scale_two() {
    use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
    use bevy_cauldron::snip::SnipOverlay;

    let mut app = editor("snip_overlay_layout");
    let world = app.world_mut();
    world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(world)
        .expect("the editor has one primary window")
        .resolution
        .set_scale_factor_override(Some(2.0));
    for _ in 0..3 {
        app.update();
    }

    // Logical window pixels in, so the surface rectangle is twice these.
    let start = Vec2::new(200.0, 120.0);
    let end = Vec2::new(420.0, 260.0);
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::Backslash],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(start),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(end),
            ..default()
        },
    );
    app.update();

    let ui_scale = app.world().resource::<UiScale>().0;
    let surface_start = start * 2.0;
    let surface_end = end * 2.0;
    let want_size = surface_end - surface_start;
    let want_centre = (surface_start + surface_end) / 2.0;
    let mut q = app
        .world_mut()
        .query_filtered::<(&ComputedNode, &UiGlobalTransform), With<SnipOverlay>>();
    let (node, at) = q
        .single(app.world())
        .expect("the drag draws exactly one overlay");
    let (size, centre) = (node.size(), at.translation);
    assert!(
        (size - want_size).abs().max_element() < 1.0,
        "the overlay must be the drag's size in surface pixels: {size:?} vs {want_size:?} at UiScale {ui_scale}"
    );
    assert!(
        (centre - want_centre).abs().max_element() < 1.0,
        "the overlay must sit over the drag: centre {centre:?} vs {want_centre:?} at UiScale {ui_scale}"
    );
}

/// **The snip's label sits above the box, never over it.** The label is a child of the overlay at
/// `top: -22`, and it has no width of its own, so in a narrow box its text wraps onto more lines and
/// the lines run down across the box's top edge. Measured in the real layout at the window's scale:
/// the label's bottom edge must be at or above the box's top edge, for a wide drag and a narrow one.
#[test]
fn the_snip_label_sits_above_the_box_and_never_over_it() {
    use bevy::ui::{ComputedNode, UiGlobalTransform};
    use bevy_cauldron::snip::SnipOverlay;

    let drags = [
        ("wide", Vec2::new(200.0, 120.0), Vec2::new(420.0, 260.0)),
        ("narrow", Vec2::new(200.0, 120.0), Vec2::new(262.0, 170.0)),
    ];
    for (name, start, end) in drags {
        let mut app = editor(&format!("snip_label_{name}"));
        let world = app.world_mut();
        world
            .query_filtered::<&mut Window, With<PrimaryWindow>>()
            .single_mut(world)
            .expect("the editor has one primary window")
            .resolution
            .set_scale_factor_override(Some(2.0));
        for _ in 0..3 {
            app.update();
        }
        step(
            &mut app,
            Frame {
                keys: vec![MOD_KEYS[0], KeyCode::Backslash],
                ..default()
            },
        );
        step(
            &mut app,
            Frame {
                pointer: Some(start),
                mouse: Some(true),
                ..default()
            },
        );
        step(
            &mut app,
            Frame {
                pointer: Some(end),
                ..default()
            },
        );
        app.update();

        let mut q = app
            .world_mut()
            .query_filtered::<(Entity, &ComputedNode, &UiGlobalTransform), With<SnipOverlay>>();
        let (overlay, node, at) = q
            .single(app.world())
            .expect("the drag draws exactly one overlay");
        let box_top = at.translation.y - node.size().y / 2.0;
        let label = app
            .world()
            .get::<Children>(overlay)
            .expect("the overlay carries its label")
            .to_vec()[0];
        let label_node = app
            .world()
            .get::<ComputedNode>(label)
            .expect("the label is laid out");
        let label_at = app
            .world()
            .get::<UiGlobalTransform>(label)
            .expect("the label has a place");
        let label_bottom = label_at.translation.y + label_node.size().y / 2.0;
        assert!(
            label_bottom <= box_top + 0.5,
            "the {name} drag's label reaches down over the box: its bottom edge is at {label_bottom} px, but the box's top edge is at {box_top} px (label size {:?})",
            label_node.size()
        );
    }
}

/// **A drag released outside the window is over.** Its start is cleared, so the next press begins a
/// new rectangle, and the person is told why nothing was snipped.
#[test]
fn a_drag_released_outside_the_window_is_cleared() {
    let mut app = editor("snip_release_outside");
    let start = Vec2::new(300.0, 200.0);
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::Backslash],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(start),
            mouse: Some(true),
            ..default()
        },
    );
    assert!(
        app.world().resource::<SnipEdit>().from.is_some(),
        "the press must begin the drag"
    );
    // The pointer has left the window, and the button comes up there.
    step(
        &mut app,
        Frame {
            mouse: Some(false),
            ..default()
        },
    );
    let snip = app.world().resource::<SnipEdit>();
    assert!(
        snip.from.is_none(),
        "a release outside the window must clear the drag, got {:?}",
        snip.from
    );
    assert!(
        snip.armed,
        "the snip stays armed so the person can drag again"
    );
    let problem = app.world().resource::<EditorState>().status.problem_text();
    assert!(
        problem.contains("outside"),
        "the refusal must say why nothing was snipped, got: {problem:?}"
    );
}

/// **The cursor the record reads is where the drag was released.** The person may move the pointer while
/// the note is typed; the ground point the record keeps must not follow it to Enter.
#[test]
fn the_record_reads_the_cursor_where_the_drag_was_released() {
    let mut app = editor("snip_release_cursor");
    for _ in 0..3 {
        app.update();
    }
    let release = Vec2::new(420.0, 260.0);
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::Backslash],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(Vec2::new(200.0, 120.0)),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(release),
            mouse: Some(false),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(Vec2::new(600.0, 400.0)),
            ..default()
        },
    );
    app.update();

    let pending = app
        .world()
        .resource::<bevy_cauldron::snip::SnipEdit>()
        .pending
        .clone()
        .expect("the release leaves the rectangle waiting for its note");
    assert_eq!(
        pending.release, release,
        "the cursor is read where the drag was released, not where the pointer is now"
    );
}

/// **A press that began before the snip was armed must not activate on release** — B-032.
///
/// `button_on_pointer_click` (`bevy_ui_widgets-0.20.0/src/button.rs:60-72`) fires `Activate` when a
/// `PointerClick` arrives on a button that has `Pressed` — and the press that sets `Pressed` may have
/// happened *before* the snip armed. Turning picking off while armed stops new presses reaching a
/// button; it cannot stop a release from completing one that is already in flight.
///
/// The person's sequence is ordinary: press a button, decide mid-press to snip instead, and the
/// button fires on release anyway. The workaround is to release and press again, which is why this is
/// not must-fix — but it is a press the person did not make.
///
/// **What this asserts is the mechanism, not the pixel**: while the snip claims the pointer, no
/// button may still be carrying `Pressed`. The editor drops it on arming, so the release finds no
/// button in flight.
#[test]
fn arming_the_snip_drops_a_press_that_was_already_in_flight() {
    use bevy::ui::Pressed;

    let mut app = editor("snip_arming_mid_press");
    for _ in 0..3 {
        app.update();
    }

    // A button with a press already in progress — the state `button_on_pointer_down` leaves behind.
    let button = app
        .world_mut()
        .spawn((
            bevy::ui_widgets::Button,
            Pressed,
            Node::default(),
        ))
        .id();
    app.update();
    assert!(
        app.world().get::<Pressed>(button).is_some(),
        "the fixture models a press in flight"
    );

    // Mid-press, the person arms the snip.
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::Backslash],
            ..default()
        },
    );
    app.update();
    assert!(
        app.world().resource::<SnipEdit>().armed,
        "the snip is armed after Cmd+\\"
    );

    assert!(
        app.world().get::<Pressed>(button).is_none(),
        "arming the snip must drop a press already in flight, or the release completes it and \
         activates a button the person was no longer pressing (B-032)"
    );
}

/// **While the snip is armed, no button sees the pointer.** A button fires its `Activate` on a click, so
/// a drag that starts and ends on it would press it. The snip turns picking off while it is armed and
/// back on when it is done, and this proves the switch; the activation itself needs a real pointer.
#[test]
fn a_snip_armed_over_a_button_does_not_press_it() {
    use bevy::picking::PickingSettings;

    let mut app = editor("snip_gates_picking");
    for _ in 0..3 {
        app.update();
    }
    assert!(
        app.world().resource::<PickingSettings>().is_enabled,
        "picking is on before the snip is armed"
    );

    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::Backslash],
            ..default()
        },
    );
    app.update();
    assert!(
        !app.world().resource::<PickingSettings>().is_enabled,
        "armed: the snip owns the pointer, so picking is off and a press cannot reach a button"
    );

    step(&mut app, Frame::default());
    app.update();
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::Backslash],
            ..default()
        },
    );
    app.update();
    assert!(
        app.world().resource::<PickingSettings>().is_enabled,
        "disarmed: picking is back on"
    );
}

/// **The drag box is plainly visible.** A snip's rectangle was drawn with a wash at alpha 0.08 and a
/// 2 px border, which measured as about 1% of a 600 px-wide capture: the box was there, but nobody
/// could find it. The wash must be a translucent accent (alpha between 0.2 and 0.5, so the map under
/// it still reads), and the border at least 3 px. Measured in the real layout, not from the constants.
#[test]
fn the_drag_box_is_plainly_visible() {
    use bevy::ui::Node;
    use bevy_cauldron::snip::SnipOverlay;

    let mut app = editor("snip_box_visible");
    let world = app.world_mut();
    world
        .query_filtered::<&mut Window, With<PrimaryWindow>>()
        .single_mut(world)
        .expect("the editor has one primary window")
        .resolution
        .set_scale_factor_override(Some(2.0));
    for _ in 0..3 {
        app.update();
    }
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::Backslash],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(Vec2::new(200.0, 120.0)),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(Vec2::new(420.0, 260.0)),
            ..default()
        },
    );
    app.update();

    let mut q = app
        .world_mut()
        .query_filtered::<(&Node, &BackgroundColor), With<SnipOverlay>>();
    let (node, fill) = q
        .single(app.world())
        .expect("the drag draws exactly one overlay");
    let alpha = fill.0.alpha();
    assert!(
        (0.2..=0.5).contains(&alpha),
        "the drag box's wash must be a translucent accent at alpha 0.2–0.5, not {alpha}"
    );
    let Val::Px(border) = node.border.left else {
        panic!("the drag box's border must be a pixel width, not {:?}", node.border.left);
    };
    assert!(
        border >= 3.0,
        "the drag box's border must be at least 3 px so it reads at a glance, not {border} px"
    );
}
