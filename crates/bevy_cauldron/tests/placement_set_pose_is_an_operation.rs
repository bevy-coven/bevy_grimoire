//! **An agent can set a placement's turn and tip in one change, and `V` is the same operation
//! (E-002, #129 slice 5).**
//!
//! Straightening a piece was `straighten_under_cursor` writing `Placed::yaw` and `Placed::tip` and a
//! pair of `Undo` entries, named by no operation, so an agent could not stand a piece up and face
//! it in one step and the file followed only with the next save. It is now the registered operation
//! `placement_set_pose`. The first two tests go through the registered BRP methods, the way an
//! agent's request goes; the third presses the real `R`, `Y` and `V` keys over a piece, the way a
//! person does. All three assert on the map the editor holds and the file on disk.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::keys::MOD_KEYS;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::view::MainCamera;

use crate::fixtures;

/// **A crate standing on a table, and a barrel on its own**, so one piece can tip and one holds
/// another up.
fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("table", "alpha")
        .descriptor("crate", "alpha")
        .descriptor("barrel", "alpha")
        .place_as("table@0", "table", (0.0, 0.0))
        .place_on("crate", "table@0", (0.0, 0.0))
        .place_as("barrel@0", "barrel", (3.0, 3.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
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

fn pose(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_set_pose", "parameters": parameters, "dry_run": false}),
    )
}

fn pose_of(app: &App, id: &str) -> (f32, (u8, u8)) {
    let p = app
        .world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("no placement `{id}`"))
        .clone();
    (p.yaw, p.tip)
}

fn file(app: &App) -> String {
    std::fs::read_to_string(&app.world().resource::<OpenMap>().map_path).expect("the map file")
}

fn file_pose(app: &App, id: &str) -> (f32, (u8, u8)) {
    let saved = bevy_grimoire_core::map::Map::parse(&file(app)).expect("the map file parses");
    let p = saved.placements.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("no row `{id}`"));
    (p.yaw, p.tip)
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **One call sets both fields, the file holds them, and one undo puts both back.**
#[test]
fn an_agent_poses_a_piece_the_file_follows_and_one_undo_puts_both_back() {
    let mut app = editor("pose-agent");
    let before = bevy_grimoire_core::map::Map::parse(&file(&app)).expect("the fixture's map parses");
    let lines_before = log(&app).len();

    let a = pose(&mut app, serde_json::json!({"id": "barrel@0", "yaw": 90.0, "tip": [1, 0]}));
    assert_eq!(a["outcome"], "applied", "the pose was not applied: {a}");
    assert_eq!(pose_of(&app, "barrel@0"), (90.0, (1, 0)), "the map the editor holds is not posed");
    assert_eq!(
        file_pose(&app, "barrel@0"),
        (90.0, (1, 0)),
        "the map file does not hold the pose, so a reopen loses it"
    );
    assert_eq!(log(&app).len(), lines_before + 1, "a pose is one log line, not one per field");

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    );
    assert_eq!(back["outcome"], "applied", "undo was refused: {back}");
    assert_eq!(pose_of(&app, "barrel@0"), (0.0, (0, 0)), "one undo must put the yaw AND the tip back");
    let undone = bevy_grimoire_core::map::Map::parse(&file(&app)).expect("the map file parses");
    assert_eq!(undone.placements, before.placements, "undo must put the file's rows back");
}

/// **A tip the piece cannot take refuses the whole pose**, so the yaw is not turned alone and
/// nothing is written; a turn that leaves the tip as it is stays allowed on a piece that holds
/// another up.
#[test]
fn a_host_an_unknown_piece_a_fifth_quarter_and_a_typo_are_refused_and_nothing_is_written() {
    let mut app = editor("pose-refused");
    let before = file(&app);

    let host = pose(&mut app, serde_json::json!({"id": "table@0", "yaw": 90.0, "tip": [0, 1]}));
    assert_eq!(host["outcome"], "refused", "a piece holding another up was tipped: {host}");
    assert!(
        host.to_string().contains("holds 1 piece"),
        "the refusal does not say what the table holds up: {host}"
    );
    let unknown = pose(&mut app, serde_json::json!({"id": "barel@0", "yaw": 90.0, "tip": [0, 0]}));
    assert_eq!(unknown["outcome"], "refused", "a typo in the id was applied: {unknown}");
    let wrapped = pose(&mut app, serde_json::json!({"id": "barrel@0", "yaw": 90.0, "tip": [4, 0]}));
    assert_eq!(wrapped["outcome"], "refused", "a fifth quarter turn was applied: {wrapped}");
    let field = pose(&mut app, serde_json::json!({"id": "barrel@0", "yaw": 90.0, "tip": [0, 0], "lift": 1.0}));
    assert_eq!(field["outcome"], "refused", "an unknown field was applied: {field}");
    let half = pose(&mut app, serde_json::json!({"id": "barrel@0", "tip": [1, 0]}));
    assert_eq!(half["outcome"], "refused", "a pose without its yaw was applied: {half}");

    assert_eq!(pose_of(&app, "table@0"), (0.0, (0, 0)), "the refused tip turned the table");
    assert_eq!(pose_of(&app, "barrel@0"), (0.0, (0, 0)));
    assert_eq!(file(&app), before, "a refusal wrote the map file");

    let turned = pose(&mut app, serde_json::json!({"id": "table@0", "yaw": 180.0, "tip": [0, 0]}));
    assert_eq!(turned["outcome"], "applied", "turning a table that holds a crate was refused: {turned}");
    assert_eq!(pose_of(&app, "table@0"), (180.0, (0, 0)));
}

/// The keys a person holds this frame and where the pointer is; released at the start of the next,
/// so a held key never repeats (`a_persons_turn_is_one_undo_step`'s driver).
#[derive(Resource, Default)]
struct Script {
    next: Option<(Vec<KeyCode>, Vec2)>,
    held: Vec<KeyCode>,
}

fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let frame = script.next.take();
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some((pressed, pointer)) = frame else {
        return;
    };
    for key in &pressed {
        keys.press(*key);
    }
    script.held = pressed;
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(Some(pointer));
    }
}

/// One frame with `keys` down, then one with nothing down, both with the pointer at `over`.
fn press(app: &mut App, keys: Vec<KeyCode>, over: Vec2) {
    app.world_mut().resource_mut::<Script>().next = Some((keys, over));
    app.update();
    app.world_mut().resource_mut::<Script>().next = Some((Vec::new(), over));
    app.update();
}

fn depth(app: &App) -> usize {
    app.world().resource::<EditorState>().undo_depth()
}

/// **A person's `V` over a turned and tipped piece is one `placement_set_pose` line and one undo
/// entry, and one Cmd+Z puts the turn and the tip back** in the map and in the file. Before the
/// operation, `V` recorded two `Undo` entries of its own in a group, wrote no log line, and left
/// the file on the old pose until the next save. A second `V` on a straight piece writes nothing.
#[test]
fn a_persons_v_is_the_operation_and_one_cmd_z_puts_both_back() {
    let root = fixtures::Fixture::new("pose-person")
        .descriptor("barrel", "alpha")
        .place_as("barrel@0", "barrel", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    // Headless, nothing moves the OS pointer: this window's cursor is the pointer the editor reads.
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    let origin = app.world().resource::<OpenMap>().map.origin;
    let mut cameras = app
        .world_mut()
        .query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(app.world()).expect("the editor has one main camera");
    let over = camera
        .world_to_viewport(transform, Vec3::new(origin.0, 0.0, origin.2))
        .unwrap_or_else(|e| panic!("the barrel is not on screen: {e:?}"));

    // **`Esc` puts the armed brush down first**, or the keys steer the ghost, not the piece.
    press(&mut app, vec![KeyCode::Escape], over);
    assert!(app.world().resource::<EditorState>().brush.is_none(), "Esc must put the brush down");

    // **Turn it and tip it with the real keys**, so there is a pose to take back.
    press(&mut app, vec![KeyCode::KeyR], over);
    press(&mut app, vec![KeyCode::KeyY], over);
    let posed = pose_of(&app, "barrel@0");
    assert!(posed.0 != 0.0, "R did not turn the barrel: {posed:?}");
    assert!(posed.1 != (0, 0), "Y did not tip the barrel: {posed:?}");
    let (before, lines_before) = (depth(&app), log(&app).len());

    press(&mut app, vec![KeyCode::KeyV], over);
    assert_eq!(pose_of(&app, "barrel@0"), (0.0, (0, 0)), "V over the barrel did not straighten it");
    assert_eq!(file_pose(&app, "barrel@0"), (0.0, (0, 0)), "the file does not hold the straight pose");
    let written = log(&app);
    let new_lines: Vec<&Entry> = written.iter().skip(lines_before).collect();
    assert_eq!(new_lines.len(), 1, "V wrote {} operation-log lines: {new_lines:?}", new_lines.len());
    assert_eq!(new_lines[0].op, "placement_set_pose", "{:?}", new_lines[0]);
    assert!(matches!(new_lines[0].source, ops::Source::Human), "a person's V is the person's: {:?}", new_lines[0]);
    assert_eq!(depth(&app), before + 1, "V wrote {} undo entries for one keypress", depth(&app) - before);

    // **Straight already**: a second `V` is a note, not a log line and not an undo entry.
    press(&mut app, vec![KeyCode::KeyV], over);
    assert_eq!(log(&app).len(), lines_before + 1, "V on a straight piece wrote a log line");
    assert_eq!(depth(&app), before + 1, "V on a straight piece recorded an undo entry");

    press(&mut app, vec![MOD_KEYS[0], KeyCode::KeyZ], over);
    assert_eq!(pose_of(&app, "barrel@0"), posed, "one Cmd+Z must put the turn and the tip back");
    assert_eq!(file_pose(&app, "barrel@0"), posed, "the file still holds the straight pose Cmd+Z took back");
    assert_eq!(depth(&app), before, "the stack did not return to where it was before the V");
}
