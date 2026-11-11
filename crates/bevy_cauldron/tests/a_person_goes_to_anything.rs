//! **Go to anything: one finder over everything a developer can name** (NV-05, #382; series getting
//! around, #377).
//!
//! The editor already has a command palette (UI-32, #226) and a reveal (AF-03, #359). This slice adds
//! no third mechanism: Cmd/Ctrl+P opens the **same** palette widget over a second source — the things
//! a project holds — and a chosen row is a [`Shown`], the one answer to *where a thing is seen*.
//! Typing `>` first lists commands instead, which is the existing palette's own source.
//!
//! Each test here is one acceptance item, measured on the thing a person sees:
//!
//! 1. A piece's name goes to the Kit workspace with that piece selected; a placement's id goes to the
//!    Map with the placement framed, and the framing is checked by projecting the placement's own
//!    footprint through the editor's camera (AF-02's measurement, reused rather than re-invented).
//! 2. `>` followed by a command's name runs that command.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::tiles::{ImportState, Mode};
use bevy_cauldron::view::{MainCamera, Rig};
use bevy_cauldron::workspace::Workspace;

use bevy_gooey::CommandPalette;

use crate::fixtures;

/// One frame of the test clock, seconds — the reveal is carried a frame at a time.
const FRAME: f32 = 1.0 / 60.0;

/// **Frames a reveal may take**: the workspace switch, the panel's first build and the selection.
const SETTLE: usize = 40;

/// **How much of the viewport a framed placement must fill**, as a fraction of its shorter side. A
/// piece framed as a change arrives with room around it ([`crate::fixtures`]' own margin, `1.25`), so
/// it covers well over this; a camera still on the whole 64 m map covers far less. The same figure
/// `the_editor_follows_an_agents_change` uses for the same question.
const FRAMED_SHARE: f32 = 0.02;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("goto_{tag}_{}", std::process::id()))
        .sized_descriptor("lamp_tall", "alpha", 0.4, 0.4)
        .sized_descriptor("bed", "alpha", 2.0, 1.0)
        .place("lamp_tall", (2.0, 2.0))
        // **Far from the origin, because the entry frame is on it**: a 64 m map opens framed on the
        // whole map, and a 2 m piece at (9, 9) still covers 29% of that viewport — the first version's
        // own guard fired on it. A far corner and a small piece put it below the threshold honestly,
        // rather than by weakening the guard.
        .place_as("bed@3", "bed", (28.0, -28.0))
        .side(64.0)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME)));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn palette_is_open(app: &mut App) -> bool {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CommandPalette>>().iter(world).next().is_some()
}

fn key_for(c: char) -> KeyCode {
    match c {
        'a' => KeyCode::KeyA,
        'b' => KeyCode::KeyB,
        'd' => KeyCode::KeyD,
        'e' => KeyCode::KeyE,
        'l' => KeyCode::KeyL,
        'm' => KeyCode::KeyM,
        'n' => KeyCode::KeyN,
        'o' => KeyCode::KeyO,
        'p' => KeyCode::KeyP,
        'r' => KeyCode::KeyR,
        's' => KeyCode::KeyS,
        't' => KeyCode::KeyT,
        'u' => KeyCode::KeyU,
        '>' => KeyCode::Period,
        other => panic!("no key for `{other}`"),
    }
}

fn send(app: &mut App, key_code: KeyCode, logical: Key, text: Option<&str>, state: ButtonState) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window");
    app.world_mut().write_message(KeyboardInput {
        key_code,
        logical_key: logical,
        state,
        text: text.map(Into::into),
        repeat: false,
        window,
    });
}

/// **The platform's primary modifier**, which is what Cmd/Ctrl+P is spelled with.
fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

/// **Cmd/Ctrl+P**, as a hand makes it: both keys down, a frame, both up.
fn ctrl_p(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in [primary(), KeyCode::KeyP] {
            send(app, key, Key::Unidentified(NativeKey::Unidentified), None, state);
        }
        app.update();
    }
    settle(app);
}

/// **Type `text` as a hand types it.** `>` is Shift+Period on a US keyboard, and the character is what
/// the palette's field reads — a bare `KeyCode::Period` produces no `>` at all, which is why the first
/// version's `>` never reached the source swap. The modifier goes down with the key and up after it,
/// the way [`crate::fixtures`]' own chord helper does it.
fn type_str(app: &mut App, text: &str) {
    for c in text.chars() {
        let s = c.to_string();
        let shift = c == '>';
        for state in [ButtonState::Pressed, ButtonState::Released] {
            if shift {
                send(app, KeyCode::ShiftLeft, Key::Unidentified(NativeKey::Unidentified), None, state);
            }
            let typed = (state == ButtonState::Pressed).then_some(s.as_str());
            send(app, key_for(c), Key::Character(s.as_str().into()), typed, state);
            if shift {
                send(app, KeyCode::ShiftLeft, Key::Unidentified(NativeKey::Unidentified), None, state);
            }
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

/// **Every corner of a box, projected through the editor's own camera, inside its viewport**, and the
/// fraction of the viewport the box covers. Both `Err`s are the sentence a failing assertion prints.
fn framed(app: &App, rect: [f32; 4]) -> Result<f32, String> {
    let world = app.world();
    let mut cameras = world.try_query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras
        .as_mut()
        .and_then(|q| q.single(&*world).ok())
        .ok_or_else(|| "the editor has no one main camera yet".to_owned())?;
    let view = camera
        .logical_viewport_rect()
        .ok_or_else(|| "the Map camera has no viewport, so nothing can be asserted".to_owned())?;
    if view.width() <= 0.0 || view.height() <= 0.0 {
        return Err(format!("the camera reports an empty viewport {view:?}"));
    }
    // **A map row's `at` is (x, z)**, and the map sits at its own origin's Y: the piece stands on the
    // floor, which is what the follow frames.
    let floor = world.resource::<OpenMap>().map.origin.1;
    let mut lo = (f32::INFINITY, f32::INFINITY);
    let mut hi = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (x, z) in [
        (rect[0], rect[1]),
        (rect[0], rect[3]),
        (rect[2], rect[1]),
        (rect[2], rect[3]),
    ] {
        let p = camera
            .world_to_viewport(transform, Vec3::new(x, floor, z))
            .map_err(|e| format!("corner ({x}, {z}) is not in view: {e}"))?;
        lo = (lo.0.min(p.x), lo.1.min(p.y));
        hi = (hi.0.max(p.x), hi.1.max(p.y));
    }
    let covered = (hi.0 - lo.0).max(0.0) * (hi.1 - lo.1).max(0.0);
    Ok(covered / (view.width() * view.height()))
}

/// **A piece's name in Cmd+P opens the Kit workspace with that piece selected.**
#[test]
fn typing_part_of_a_pieces_name_opens_the_kit_workspace_with_it_selected() {
    let mut app = editor("piece");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Map, "the test starts on the Map");

    ctrl_p(&mut app);
    assert!(palette_is_open(&mut app), "Cmd/Ctrl+P opens the one finder");
    type_str(&mut app, "lamp");
    enter(&mut app);

    assert!(!palette_is_open(&mut app), "choosing a result closes the finder");
    settle(&mut app);
    assert_eq!(
        *app.world().resource::<Workspace>(),
        Workspace::Kit,
        "a library piece is shown on the Kit workspace"
    );
    assert_eq!(
        app.world().resource::<Mode>().label(),
        Mode::Meshes.label(),
        "on its Meshes panel"
    );
    assert_eq!(
        app.world().resource::<ImportState>().selected_library_id.as_deref(),
        Some("lamp_tall"),
        "the piece the person named is the row selected, not the fixture's first"
    );
}

/// **A placement's id opens the Map with the placement framed**, measured by projecting its own
/// footprint through the editor's camera (AF-02) rather than by reading the camera's numbers back.
#[test]
fn a_placement_id_opens_the_map_with_the_placement_framed() {
    let mut app = editor("placement");
    // **Somewhere else entirely, so a finder that did nothing cannot pass.** The entry frame frames the
    // whole 64 m map, which already holds a 2 m piece at nearly a third of the viewport — the first
    // version's own guard proved it (`Ok(0.29532656)`). So the camera is put on the *opposite* corner at
    // 8 m across: the piece at (28, -28) is outside that box, and only a real go-to brings it in.
    {
        let mut rig = app.world_mut().resource_mut::<Rig>();
        rig.focus = Vec3::new(-28.0, 0.0, 28.0);
        rig.height = 8.0;
    }
    settle(&mut app);

    let rect = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .find(|p| p.id == "bed@3")
        .map(|p| {
            let d = app.world().resource::<bevy_cauldron::project::Project>().library.get(&p.descriptor);
            d.and_then(|d| bevy_cauldron::ops::footprint_rect(d, p.at, p.yaw, p.tip))
                .unwrap_or_else(|| bevy_cauldron::ops::point_box(p.at))
        })
        .expect("the fixture places `bed@3`");
    let before = framed(&app, rect);
    assert!(
        before.as_ref().is_err() || before.as_ref().is_ok_and(|share| *share < FRAMED_SHARE),
        "the camera already holds the placement, so the test proves nothing: {before:?}"
    );

    ctrl_p(&mut app);
    assert!(palette_is_open(&mut app));
    type_str(&mut app, "bed@3");
    enter(&mut app);
    settle(&mut app);

    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Map, "a placement is on the Map");
    let share = framed(&app, rect)
        .unwrap_or_else(|why| panic!("the placement is not framed after go-to: {why}"));
    assert!(
        share > FRAMED_SHARE,
        "the placement covers {:.1}% of the viewport, so the camera framed the map and not it",
        share * 100.0
    );
}

/// **`>` followed by a command's name runs that command.** The undo depth is the measurement: a
/// command that ran once leaves exactly one step waiting to be redone, as the palette's own test
/// pins it.
#[test]
fn a_greater_than_sign_then_a_command_name_runs_that_command() {
    let mut app = editor("command");
    bevy_cauldron::editor::run_for_test(app.world_mut(), "bed", (3.0, 3.0), (6.0, 3.0));
    settle(&mut app);
    assert!(app.world().resource::<EditorState>().undo_depth() > 0, "a run is one undo step");

    ctrl_p(&mut app);
    assert!(palette_is_open(&mut app));
    type_str(&mut app, ">undo");
    enter(&mut app);
    settle(&mut app);

    assert!(!palette_is_open(&mut app), "choosing a command closes the finder");
    assert_eq!(
        app.world().resource::<EditorState>().redo_depth(),
        1,
        "`>` then the command's name ran `edit.undo` once: the `>` itself was not part of the query"
    );
}
