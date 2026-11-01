//! **A person's own `R` writes ONE undo entry, and one Cmd+Z takes the whole turn back.**
//!
//! E-027. Since 6aa6185 the turn keys dispatch the registered `placement_set_yaw`, which appends an
//! operation-log line and records an [`Undo::Logged`] on the person's stack; before that, the key
//! recorded an `Undo::Turned` of its own. Both are reasonable on their own. **Both at once would be
//! one keypress and two undos** — the first Cmd+Z taking back the `Logged` entry and leaving the
//! `Turned` entry on top of a stack that no longer matched the map, so the second Cmd+Z would undo
//! a turn that had already happened and the third would reach an older, unrelated edit.
//!
//! 4364dd4 ("one history") is what made the log line *be* the undo entry, and the backlog has
//! carried the open question ever since: **no test showed a human turn wrote one entry, not two.**
//! This is that test. It drives the real `R` key over a real piece in a real headless editor and
//! counts the stack, so a second record site added to `turn_under_cursor` fails here instead of
//! being discovered by a person whose second Cmd+Z took back the wrong change.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::keys::MOD_KEYS;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

/// One frame's worth of person: the keys held down and where the pointer is.
#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
}

/// The next frame to play. `held` is what the previous frame pressed, released at the start of the
/// next so no key is left down by accident — the pattern `a_person_removes_a_piece_and_undoes_it`
/// uses, and the reason is the same: a held `R` repeats, and a repeat is a second turn.
#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

/// Runs in `PreUpdate` after `InputSystems`, so the press survives into `Update` and the window's
/// cursor is already where the person put it when `sense_pointer` reads it.
fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
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
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

/// Where on the screen a person would point to hit the ground at a map-space spot.
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
        .unwrap_or_else(|e| panic!("the piece is not on screen: {e:?}"))
}

fn held(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

fn on_disk(app: &App) -> bevy_grimoire_core::map::Map {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

fn depth(app: &App) -> usize {
    app.world().resource::<EditorState>().undo_depth()
}

/// The editor, on a map holding one wall at the origin, with a window for the pointer to live in.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    // Headless, nothing moves the OS pointer: this entity is only read, so
    // `window.set_cursor_position` is the pointer `sense_pointer` sees.
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **`R` over a piece is one entry on one stack, and one Cmd+Z takes the whole turn back.**
///
/// The count is the assertion, not the final yaw: two records and one undo would also leave the
/// map looking right after the first Cmd+Z — what gives it away is the *second* Cmd+Z, which
/// reaches an entry describing a turn that has already been undone.
#[test]
fn a_persons_turn_writes_one_undo_entry_and_one_cmd_z_takes_it_back() {
    let mut app = editor("person_turn_one_entry");
    let start = held(&app);
    assert_eq!(
        start.placements.len(),
        1,
        "the fixture starts with one wall"
    );
    let id = start.placements[0].id.clone();
    let start_yaw = start.placements[0].yaw;
    let before = depth(&app);
    let lines_before = log(&app).len();

    let over = pixel_over(&mut app, (0.0, 0.0));
    // **`Esc` puts the brush down first, and that is the person's own step.** The editor opens with
    // a piece armed (an editor with nothing selected reads as broken), and `R` with a brush in hand
    // steers the *ghost* rather than the piece under the cursor — the subject rule `drive_ghost`
    // follows. A person turning a wall on the map presses `Esc` first; the test does the same
    // rather than reaching past the key to the state it would have set.
    step(
        &mut app,
        Frame {
            keys: vec![KeyCode::Escape],
            pointer: Some(over),
        },
    );
    step(
        &mut app,
        Frame {
            keys: vec![],
            pointer: Some(over),
        },
    );
    assert!(
        app.world().resource::<EditorState>().brush.is_none(),
        "Esc must put the brush down, or R steers a ghost instead of the piece"
    );

    step(
        &mut app,
        Frame {
            keys: vec![KeyCode::KeyR],
            pointer: Some(over),
        },
    );
    step(
        &mut app,
        Frame {
            keys: vec![],
            pointer: Some(over),
        },
    );

    let turned = held(&app);
    assert!(
        turned.placements[0].yaw != start_yaw,
        "R over the wall did not turn it: {} is still {start_yaw}",
        turned.placements[0].yaw
    );
    let after_yaw = turned.placements[0].yaw;

    // **One line in the log, and one entry on the stack.** The log is what an agent reads; the
    // stack is what Cmd+Z walks. A keypress that wrote one and not the other is a change that is
    // half-recorded, and the two counts disagreeing is the whole defect E-027 describes.
    let written = log(&app);
    let new_lines: Vec<&Entry> = written.iter().skip(lines_before).collect();
    assert_eq!(
        new_lines.len(),
        1,
        "R wrote {} operation-log lines, so one keypress is {} changes: {new_lines:?}",
        new_lines.len(),
        new_lines.len()
    );
    assert_eq!(new_lines[0].op, "placement_set_yaw", "{:?}", new_lines[0]);
    assert!(
        matches!(new_lines[0].source, ops::Source::Human),
        "a person's own turn is the person's, not an agent's: {:?}",
        new_lines[0]
    );
    assert_eq!(
        depth(&app),
        before + 1,
        "R wrote {} undo entries for one keypress. A second entry means the first Cmd+Z takes back \
         one half of the turn and leaves the other on the stack, so the next Cmd+Z undoes a change \
         that has already happened.",
        depth(&app) - before
    );

    // **One Cmd+Z takes the whole turn back**, on the screen, in the map and in the file.
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::KeyZ],
            pointer: Some(over),
        },
    );
    step(
        &mut app,
        Frame {
            keys: vec![],
            pointer: Some(over),
        },
    );
    assert_eq!(
        held(&app).placements[0].yaw,
        start_yaw,
        "one Cmd+Z must put `{id}` back to {start_yaw}; it left it at {}",
        held(&app).placements[0].yaw
    );
    assert_eq!(
        on_disk(&app).placements[0].yaw,
        start_yaw,
        "the file still holds the turn Cmd+Z took back"
    );
    assert_eq!(
        depth(&app),
        before,
        "the stack did not return to where it was before the turn"
    );
    assert!(
        log(&app).iter().any(|e| e.undoes.is_some() && e.outcome == "applied"),
        "the Cmd+Z is not a logged line naming the turn it took back: {:?}",
        log(&app)
    );

    // **And a second Cmd+Z changes nothing**, because the turn was the newest change and nothing
    // older is on the stack. **This is the assertion a duplicate entry fails**: with a second record
    // underneath, the turn-back sits above an entry describing the same turn, so the next Cmd+Z
    // re-applies it and the wall comes back round to `after_yaw`.
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::KeyZ],
            pointer: Some(over),
        },
    );
    step(
        &mut app,
        Frame {
            keys: vec![],
            pointer: Some(over),
        },
    );
    assert_eq!(
        held(&app).placements[0].yaw,
        start_yaw,
        "a second Cmd+Z turned the wall again, to {} from {start_yaw}: the turn is recorded twice, \
         so the stack still holds a turn that has already been taken back",
        held(&app).placements[0].yaw
    );
    assert_ne!(
        held(&app).placements[0].yaw,
        after_yaw,
        "a second Cmd+Z replayed the turn ({after_yaw} deg) the first one took back, so one \
         keypress wrote two entries"
    );
    assert_eq!(
        depth(&app),
        before,
        "a second Cmd+Z found an entry to take back, so the turn wrote more than one"
    );
}
