//! **A person's box removal goes through the registry, one line per piece, and one Cmd+Z takes the
//! whole box back in the order it was in.**
//!
//! The box path used to delete rows in place and record `Undo::RemovedMany`, which left no line in
//! the operation log for the pieces it took. It now dispatches `placement_remove` for each piece,
//! back to front (so no index is shifted under a later removal), and records the lines as one
//! `Undo::Group`. Undo applies the group's children in list order, so the list must run lowest index
//! first: a put-back of index 2 before index 0 would land the second piece one slot short.
//!
//! The pointer and keys are injected the way `a_person_removes_a_piece_and_undoes_it.rs` does it.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::{EditorState, Tool};
use bevy_cauldron::harness;
use bevy_cauldron::keys::MOD_KEYS;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

/// One frame's worth of person: the keys held down, the pointer, and a mouse press or release.
#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

/// The next frame to play. `held` is what the previous frame pressed, released at the start of the
/// next so no key is left down by accident.
#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

/// Runs in `PreUpdate` after `InputSystems`, so the presses survive into `Update` and the window's
/// cursor is already where the person put it when `sense_pointer` reads it.
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
        .unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

fn held(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

fn on_disk(app: &App) -> bevy_grimoire_core::map::Map {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

fn ids(map: &bevy_grimoire_core::map::Map) -> Vec<String> {
    map.placements.iter().map(|p| p.id.clone()).collect()
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

#[test]
fn a_person_removes_a_box_of_pieces_and_one_cmd_z_puts_them_all_back_in_order() {
    // Three pieces: the two on the row are inside the box, the one off it is not. The box takes the
    // first and the last, so the survivor sits between them and the restored order is checkable.
    let root = fixtures::Fixture::new("person_box_remove")
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
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1", "site/wall@2"],
        "the fixture starts with three"
    );

    let from = pixel_over(&mut app, (-1.0, -1.0));
    let to = pixel_over(&mut app, (7.0, 1.0));

    // **X arms removal mode.**
    step(
        &mut app,
        Frame {
            keys: vec![KeyCode::KeyX],
            ..default()
        },
    );
    assert!(
        matches!(app.world().resource::<EditorState>().tool, Tool::Remove),
        "X must arm removal mode"
    );

    // **A press, a drag across the two pieces, and a release.**
    step(
        &mut app,
        Frame {
            pointer: Some(from),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            mouse: Some(false),
            ..default()
        },
    );
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@1"],
        "the box took the two pieces it covered and left the one off it"
    );
    assert_eq!(
        ids(&on_disk(&app)),
        ["site/wall@1"],
        "the file agrees with the editor"
    );

    // **Each piece is its own logged removal, back to front.** Removing index 0 first would shift
    // index 2 down, so the registry sees index 2 first, then 0.
    let removals: Vec<String> = log(&app)
        .into_iter()
        .filter(|e| e.op == "placement_remove")
        .map(|e| {
            assert_eq!(e.outcome, "applied", "{e:?}");
            assert!(matches!(e.source, ops::Source::Human), "{e:?}");
            e.params["id"].as_str().unwrap_or_default().to_owned()
        })
        .collect();
    assert_eq!(
        removals,
        ["site/wall@2", "site/wall@0"],
        "one logged placement_remove per piece, taken back to front"
    );

    // **One Cmd+Z takes the whole box back, in the order it was in.** Two undo entries would need
    // two presses, so one press restoring all three proves the box is one undo.
    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::KeyZ],
            ..default()
        },
    );
    step(&mut app, Frame::default());
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1", "site/wall@2"],
        "one Cmd+Z puts all three back at the rows they held"
    );
    assert_eq!(
        ids(&on_disk(&app)),
        ["site/wall@0", "site/wall@1", "site/wall@2"],
        "the file agrees with the editor after the undo"
    );
}

#[test]
fn a_box_over_a_stack_leaves_the_piece_between_its_rows() {
    // **The rider sits at a lower row than its host.** Row 0 is the rider, row 1 a piece apart, and
    // row 2 the host it stands on. The box covers the pair and not the piece at row 1. Removing by
    // the rows the box found takes the host, which takes the rider with it, which shifts the piece
    // at row 1 down to row 0 just before the next index is read. That index then removes the piece
    // the box never covered. It must survive.
    let root = fixtures::Fixture::new("person_box_stack_rows")
        .descriptor("site/wall", "alpha")
        .place_on("site/wall", "site/wall@2", (6.0, 0.0))
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
    assert_eq!(
        held(&app).placements[0].on.as_deref(),
        Some("site/wall@2"),
        "the fixture starts with the piece at row 0 standing on the host at row 2"
    );

    let from = pixel_over(&mut app, (4.5, -1.0));
    let to = pixel_over(&mut app, (7.0, 1.0));
    step(
        &mut app,
        Frame {
            keys: vec![KeyCode::KeyX],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(from),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            mouse: Some(false),
            ..default()
        },
    );
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@1"],
        "the box took the stack, and the piece between its rows stayed"
    );
    assert_eq!(
        ids(&on_disk(&app)),
        ["site/wall@1"],
        "the file agrees with the editor"
    );
    // **The receipt counts the pieces, not the rows the box found.** The host and its rider went in
    // one removal, so the note must say two placements. It said one until the count was fixed.
    let said = app
        .world()
        .resource::<EditorState>()
        .status
        .note_text()
        .to_owned();
    assert!(
        said.contains("removed 2 placement(s)"),
        "the receipt counts the pieces the box took: {said}"
    );
}

/// Three pieces, and the box takes the outer two. The piece between them stays on the map.
fn box_takes_the_outer_pair(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 5.0))
        .place("site/wall", (6.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    let from = pixel_over(&mut app, (-1.0, -1.0));
    let to = pixel_over(&mut app, (7.0, 1.0));
    step(
        &mut app,
        Frame {
            keys: vec![KeyCode::KeyX],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(from),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            mouse: Some(false),
            ..default()
        },
    );
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@1"],
        "the box took the outer pair"
    );
    app
}

/// **Through the registered method**, as an agent's request goes.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// An agent takes back the change one log line made.
fn agent_undo(app: &mut App, operation_id: &str) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": operation_id, "dry_run": false}),
    )
}

/// The newest log line that removed `id`.
fn removal_line(app: &App, id: &str) -> String {
    log(app)
        .iter()
        .rev()
        .find(|e| e.op == "placement_remove" && e.params["id"] == id)
        .unwrap_or_else(|| panic!("the log holds no removal of {id}"))
        .operation_id
        .clone()
}

#[test]
fn an_agent_undo_inside_a_box_leaves_the_persons_undo_whole() {
    // **The agent takes back one piece's removal from inside the box.** The box's group still held
    // that line, so the person's Cmd+Z would put the piece back a second time: a refusal, a problem
    // raised for an undo the person never made, and the rest of the box left out.
    let mut app = box_takes_the_outer_pair("person_box_agent_undo");
    let line = removal_line(&app, "site/wall@0");
    let answer = agent_undo(&mut app, &line);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1"],
        "the agent put site/wall@0 back"
    );

    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::KeyZ],
            ..default()
        },
    );
    step(&mut app, Frame::default());
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1", "site/wall@2"],
        "the person's Cmd+Z puts back what the box still holds"
    );
    let status = &app.world().resource::<EditorState>().status;
    assert!(
        !status.has_problem(),
        "no refusal for an undo the person never made: {}",
        status.problem_text()
    );
}

#[test]
fn an_agent_that_undoes_every_line_of_a_box_leaves_no_stale_group_for_cmd_z() {
    // **Once the agent has taken back each line, the box has nothing left in effect.** Its group must
    // go with its last child, so a Cmd+Z with nothing to take back changes nothing and raises nothing.
    let mut app = box_takes_the_outer_pair("person_box_agent_undo_all");
    for id in ["site/wall@0", "site/wall@2"] {
        let line = removal_line(&app, id);
        let answer = agent_undo(&mut app, &line);
        assert_eq!(answer["outcome"], "applied", "{answer}");
    }
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1", "site/wall@2"],
        "the agent put back both pieces"
    );

    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::KeyZ],
            ..default()
        },
    );
    step(&mut app, Frame::default());
    assert_eq!(
        ids(&held(&app)),
        ["site/wall@0", "site/wall@1", "site/wall@2"],
        "a Cmd+Z with nothing left to take back changes nothing"
    );
    let status = &app.world().resource::<EditorState>().status;
    assert!(
        !status.has_problem(),
        "a Cmd+Z over a stale group raises a refusal: {}",
        status.problem_text()
    );
}

/// A map with one stamp, `room@1`, of `site/room` at the origin, and one free piece, `site/wall@0`, at
/// (3, 0). The fixture's map carries no stamps, so the stamp is written into its text the way
/// `stamp_remove.rs` does it.
fn stamp_and_piece_in_one_box(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .composition("site/room", &[("site/wall", "site/wall", (0.0, 0.0))])
        .place("site/wall", (3.0, 0.0))
        .build("m");
    let map = root.join("assets/grimoire/maps/m.map.ron");
    let text = std::fs::read_to_string(&map).expect("the map is there");
    let stamped = text.replace(
        "stamps: [],",
        r#"stamps: [
        (
            id: "room@1",
            of: "site/room",
            at: (0.0, 0.0),
            yaw: 0.0,
            overrides: [],
            of_fingerprint: None,
            owned: false,
            owned_because: None,
            note: None,
        ),
    ],"#,
    );
    assert_ne!(
        stamped, text,
        "the fixture's map has no `stamps: [],` to fill; this test would stamp nothing"
    );
    std::fs::write(&map, stamped).expect("the map is writable");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn stamps(map: &bevy_grimoire_core::map::Map) -> Vec<String> {
    map.stamps.iter().map(|s| s.id.clone()).collect()
}

#[test]
fn a_box_over_a_stamp_and_a_piece_removes_both_and_one_cmd_z_puts_both_back() {
    // **A box is one undo group whether it takes a stamp or a piece.** The stamp is removed through
    // `stamp_remove` and the piece through `placement_remove`, and one Cmd+Z must restore both.
    let mut app = stamp_and_piece_in_one_box("person_box_remove_stamp");
    assert_eq!(
        stamps(&held(&app)),
        ["room@1"],
        "the fixture starts with the stamp"
    );
    assert_eq!(ids(&held(&app)), ["site/wall@0"], "and one free piece");

    let from = pixel_over(&mut app, (-1.0, -1.0));
    let to = pixel_over(&mut app, (7.0, 1.0));
    step(
        &mut app,
        Frame {
            keys: vec![KeyCode::KeyX],
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(from),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            ..default()
        },
    );
    step(
        &mut app,
        Frame {
            pointer: Some(to),
            mouse: Some(false),
            ..default()
        },
    );
    assert!(
        stamps(&held(&app)).is_empty(),
        "the box took the stamp it covered"
    );
    assert!(ids(&held(&app)).is_empty(), "and the piece it covered");
    assert!(
        stamps(&on_disk(&app)).is_empty(),
        "the file agrees on the stamp"
    );
    assert!(ids(&on_disk(&app)).is_empty(), "and on the piece");

    let removals: Vec<(String, String)> = log(&app)
        .into_iter()
        .filter(|e| e.op == "stamp_remove" || e.op == "placement_remove")
        .map(|e| (e.op.clone(), e.outcome.clone()))
        .collect();
    assert_eq!(
        removals.len(),
        2,
        "one logged line per removed thing: {removals:?}"
    );
    assert!(
        removals.iter().all(|(_, outcome)| outcome == "applied"),
        "{removals:?}"
    );

    step(
        &mut app,
        Frame {
            keys: vec![MOD_KEYS[0], KeyCode::KeyZ],
            ..default()
        },
    );
    step(&mut app, Frame::default());
    assert_eq!(
        stamps(&held(&app)),
        ["room@1"],
        "one Cmd+Z puts the stamp back"
    );
    assert_eq!(ids(&held(&app)), ["site/wall@0"], "and the piece with it");
    assert_eq!(
        stamps(&on_disk(&app)),
        ["room@1"],
        "the file agrees on the stamp"
    );
    assert_eq!(ids(&on_disk(&app)), ["site/wall@0"], "and on the piece");
}
