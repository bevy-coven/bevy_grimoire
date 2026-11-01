//! **An agent can lift a placement, and `[` and `]` are the same operation (E-002, #129 slice 4).**
//!
//! Lifting a piece was `lift_under_cursor` writing `Placed::lift` and an `Undo::Lifted` entry, named
//! by no operation, so an agent could not raise a lamp off its table and the lift reached the file
//! only with the next save. It is now the registered operation `placement_set_lift`. The first two
//! tests go through the registered BRP methods, the way an agent's request goes; the third presses
//! the real `]` and `[` keys over a piece, the way a person does. All three assert on the map the
//! editor holds and the file on disk.

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

/// **A crate standing on a table, and a barrel on its own**: a piece that holds another up lifts
/// too, and the piece on it rides with it.
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

fn lift(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_set_lift", "parameters": parameters, "dry_run": false}),
    )
}

fn lift_of(app: &App, id: &str) -> f32 {
    app.world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("no placement `{id}`"))
        .lift
}

fn file(app: &App) -> String {
    std::fs::read_to_string(&app.world().resource::<OpenMap>().map_path).expect("the map file")
}

#[test]
fn an_agent_lifts_a_piece_the_file_follows_and_undo_puts_it_back() {
    let mut app = editor("lift-agent");
    let before = bevy_grimoire_core::map::Map::parse(&file(&app)).expect("the fixture's map parses");

    // **The table holds the crate up**, and it lifts: the crate rides with it.
    let a = lift(&mut app, serde_json::json!({"id": "table@0", "lift": 0.5}));
    assert_eq!(a["outcome"], "applied", "the lift was not applied: {a}");
    assert_eq!(lift_of(&app, "table@0"), 0.5, "the map the editor holds is not lifted");
    let saved = bevy_grimoire_core::map::Map::parse(&file(&app)).expect("the map file parses");
    let table = saved.placements.iter().find(|p| p.id == "table@0").expect("the table row");
    assert_eq!(table.lift, 0.5, "the map file does not hold the lift, so a reopen loses it");

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    );
    assert_eq!(back["outcome"], "applied", "undo was refused: {back}");
    assert_eq!(lift_of(&app, "table@0"), 0.0);
    let undone = bevy_grimoire_core::map::Map::parse(&file(&app)).expect("the map file parses");
    assert_eq!(undone.placements, before.placements, "undo must put the file's rows back");

    // **Below the datum is a lift too**, and a lift within float noise of zero is zero, so the file
    // drops the field instead of keeping `lift: 0.00001`.
    let down = lift(&mut app, serde_json::json!({"id": "barrel@0", "lift": -0.25}));
    assert_eq!(down["outcome"], "applied", "a negative lift was refused: {down}");
    assert_eq!(lift_of(&app, "barrel@0"), -0.25);
    let home = lift(&mut app, serde_json::json!({"id": "barrel@0", "lift": 0.00001}));
    assert_eq!(home["outcome"], "applied", "{home}");
    assert_eq!(lift_of(&app, "barrel@0"), 0.0, "float noise around zero must be exactly zero");
    assert!(
        !file(&app).contains("lift"),
        "a piece on its datum must not write a lift field: {}",
        file(&app)
    );
}

/// **A lift an f32 cannot spell in a few digits still undoes.** The log line holds the lift as JSON
/// text and undo compares it with the map's lift, so a number that does not survive the text (an
/// f32 widened to f64 has 17 digits, and serde_json's default parser can read one back a step off)
/// made `grimoire/operation_undo` refuse "the map has changed outside the log". `0.03` is one such
/// value, and the key's 1/3 m step reaches more of them.
#[test]
fn a_lift_that_an_f32_spells_in_many_digits_still_undoes() {
    let mut app = editor("lift-digits");
    for lift_m in [0.03_f32, 0.11, 1.0 / 3.0] {
        let a = lift(&mut app, serde_json::json!({"id": "barrel@0", "lift": lift_m}));
        assert_eq!(a["outcome"], "applied", "a lift of {lift_m} was not applied: {a}");
        assert_eq!(lift_of(&app, "barrel@0"), lift_m, "the map does not hold {lift_m}");

        let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
        let back = call(
            &mut app,
            "grimoire/operation_undo",
            serde_json::json!({"operation_id": id, "dry_run": false}),
        );
        assert_eq!(back["outcome"], "applied", "undo of a lift of {lift_m} was refused: {back}");
        assert_eq!(lift_of(&app, "barrel@0"), 0.0, "undo of {lift_m} left the barrel lifted");
    }
}

#[test]
fn an_unknown_piece_a_typo_and_a_non_number_are_refused_and_nothing_is_written() {
    let mut app = editor("lift-refused");
    let before = file(&app);

    let unknown = lift(&mut app, serde_json::json!({"id": "barel@0", "lift": 0.5}));
    assert_eq!(unknown["outcome"], "refused", "a typo in the id was applied: {unknown}");
    assert!(
        unknown.to_string().contains("no placement is called `barel@0`"),
        "the refusal does not name the id it could not find: {unknown}"
    );
    let field = lift(&mut app, serde_json::json!({"id": "barrel@0", "height": 0.5}));
    assert_eq!(field["outcome"], "refused", "an unknown field was applied: {field}");
    let word = lift(&mut app, serde_json::json!({"id": "barrel@0", "lift": "high"}));
    assert_eq!(word["outcome"], "refused", "a lift that is not a number was applied: {word}");

    assert_eq!(lift_of(&app, "table@0"), 0.0);
    assert_eq!(lift_of(&app, "barrel@0"), 0.0);
    assert_eq!(file(&app), before, "a refusal wrote the map file");
}

/// The keys a person holds this frame and where the pointer is; released at the start of the next,
/// so a held `]` never repeats into a second lift (`a_persons_y_is_the_operation_and_one_cmd_z_takes_it_back`'s
/// driver).
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

/// **A person's `]` over a piece is one `placement_set_lift` line and one undo entry, `[` brings it
/// exactly home, and one Cmd+Z takes a lift back** in the map and in the file. Before the operation,
/// `]` recorded an `Undo::Lifted` of its own and wrote no log line, so an agent reading the log never
/// saw a person's lift and the file kept the old height until the next save.
#[test]
fn a_persons_bracket_is_the_operation_and_one_cmd_z_takes_it_back() {
    let root = fixtures::Fixture::new("lift-person")
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
    let over = over_barrel(&mut app, 0.0);

    // **`Esc` puts the armed brush down first**, or the bracket steers the ghost, not the piece.
    press(&mut app, vec![KeyCode::Escape], over);
    assert!(app.world().resource::<EditorState>().brush.is_none(), "Esc must put the brush down");
    let (before, lines_before) = (depth(&app), log(&app).len());

    press(&mut app, vec![KeyCode::BracketRight], over);
    let up = lift_of(&app, "barrel@0");
    assert!(up > 0.0, "] over the barrel did not lift it: {up}");
    let saved = bevy_grimoire_core::map::Map::parse(&file(&app)).expect("the map file parses");
    assert_eq!(saved.placements[0].lift, up, "the file does not hold the person's lift");
    let written = log(&app);
    let new_lines: Vec<&Entry> = written.iter().skip(lines_before).collect();
    assert_eq!(new_lines.len(), 1, "] wrote {} operation-log lines: {new_lines:?}", new_lines.len());
    assert_eq!(new_lines[0].op, "placement_set_lift", "{:?}", new_lines[0]);
    assert!(matches!(new_lines[0].source, ops::Source::Human), "a person's lift is the person's: {:?}", new_lines[0]);
    assert_eq!(depth(&app), before + 1, "] wrote {} undo entries for one keypress", depth(&app) - before);

    press(&mut app, vec![MOD_KEYS[0], KeyCode::KeyZ], over);
    assert_eq!(lift_of(&app, "barrel@0"), 0.0, "one Cmd+Z must put the barrel back on its datum");
    let undone = bevy_grimoire_core::map::Map::parse(&file(&app)).expect("the map file parses");
    assert_eq!(undone.placements[0].lift, 0.0, "the file still holds the lift Cmd+Z took back");
    assert_eq!(depth(&app), before, "the stack did not return to where it was before the lift");

    // **`]` then `[` is exactly home**, not a hair off it: the file drops the field.
    press(&mut app, vec![KeyCode::BracketRight], over);
    let lifted = lift_of(&app, "barrel@0");
    assert!(lifted > 0.0, "] over the barrel did not lift it the second time: {lifted}");
    // **The pick is the ray through the piece's own volume**, so once the barrel is a step up the
    // pixel that met its base meets air: the pointer follows the piece, as a hand would.
    let over_lifted = over_barrel(&mut app, lifted + 0.05);
    press(&mut app, vec![KeyCode::BracketLeft], over_lifted);
    assert_eq!(lift_of(&app, "barrel@0"), 0.0, "] then [ must be exactly on the datum");
    assert!(!file(&app).contains("lift"), "a piece on its datum must not write a lift field: {}", file(&app));
}

/// **The pixel over the barrel at height `y` metres**, so the pointer is on the piece wherever its lift
/// has put it.
fn over_barrel(app: &mut App, y: f32) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let mut cameras = app
        .world_mut()
        .query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(app.world()).expect("the editor has one main camera");
    camera
        .world_to_viewport(transform, Vec3::new(origin.0, y, origin.2))
        .unwrap_or_else(|e| panic!("the barrel is not on screen: {e:?}"))
}
