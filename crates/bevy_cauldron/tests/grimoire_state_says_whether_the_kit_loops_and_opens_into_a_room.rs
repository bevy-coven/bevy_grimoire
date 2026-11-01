//! **`grimoire_state` says whether the kit can loop a hallway back and open it into a room** (#171).
//!
//! `adjacency::ring` (#160) and `adjacency::room` (#162) are the kit stress tests after
//! `dead_ends`, but only `dead_ends` reached an agent. So `grimoire_state.stress` carries both, from
//! the project's library on every call: the loop's four pieces and the room's nine, each with its
//! turn in degrees, or null, and `declared`, how many pieces carry an edge token at all, so an
//! unlabelled kit's nulls are not read as a kit that fails.

use bevy::prelude::*;
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::fixtures;

/// The lattice cell holding `hall` on each named face of a piece three cells square and one high
/// (x grows east, z grows south): the middle cell of that face.
fn cell(face: char) -> (u32, u32, u32) {
    match face {
        'N' => (1, 0, 0),
        'E' => (2, 0, 1),
        'S' => (1, 0, 2),
        _ => (0, 0, 1),
    }
}

fn subgrid(faces: &str) -> String {
    let cells: Vec<String> = faces
        .chars()
        .map(|f| {
            let (x, y, z) = cell(f);
            format!(r#"( at: ({x}, {y}, {z}), edge: Some("hall") )"#)
        })
        .collect();
    format!("subgrid: Some(( cells: [{}] ))", cells.join(", "))
}

/// A project whose kit is `pieces`, each `(id, faces open to a neighbour)`; `""` declares nothing.
fn project(name: &str, pieces: &[(&str, &str)]) -> PathBuf {
    let side = 3.0 * bevy_grimoire_core::grid::SNAP;
    let mut fixture = fixtures::Fixture::new(name).edge_tokens(&["hall"]);
    for (id, _) in pieces {
        fixture = fixture.sized_descriptor(id, "alpha", side, side);
    }
    let root = fixture.build("m");
    for (id, faces) in pieces {
        if !faces.is_empty() {
            set_faces(&root, id, faces);
        }
    }
    root
}

fn library_path(root: &std::path::Path) -> PathBuf {
    root.join("assets/grimoire/furniture/library.ron")
}

/// Rewrite piece `id`'s lattice in the project's library file to `hall` on `faces`.
fn set_faces(root: &std::path::Path, id: &str, faces: &str) {
    let lib = library_path(root);
    let text = std::fs::read_to_string(&lib).unwrap_or_else(|e| panic!("{lib:?}: {e}"));
    let at = text
        .find(&format!("id: \"{id}\""))
        .unwrap_or_else(|| panic!("no {id} row: {text}"));
    let (head, tail) = text.split_at(at);
    let end = tail.find("note:").unwrap_or_else(|| panic!("{id} has no note: {tail}"));
    let (row, rest) = tail.split_at(end);
    let start = row.find("subgrid:").unwrap_or_else(|| panic!("{id} has no subgrid: {row}"));
    let row = format!("{}{},\n            ", &row[..start], subgrid(faces));
    std::fs::write(&lib, format!("{head}{row}{rest}")).unwrap_or_else(|e| panic!("{e}"));
}

fn editor(root: &std::path::Path) -> App {
    let mut app = bevy_cauldron::harness::build_headless(root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn stress(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get("grimoire/state").cloned()
    else {
        panic!("no method `grimoire/state` is registered");
    };
    let st = app
        .world_mut()
        .run_system_with(id, Some(json!({})))
        .unwrap_or_else(|e| panic!("grimoire/state: {e:?}"))
        .unwrap_or_else(|e| panic!("grimoire/state: {e:?}"));
    st["stress"].clone()
}

const CORRIDOR: [(&str, &str); 2] = [("straight", "EW"), ("corner", "ES")];

fn piece_names(cells: &Value) -> Vec<String> {
    cells
        .as_array()
        .unwrap_or_else(|| panic!("not a list: {cells}"))
        .iter()
        .map(|c| c["piece"].as_str().unwrap_or_else(|| panic!("no piece: {c}")).to_owned())
        .collect()
}

#[test]
fn a_corridor_kit_loops_back_and_has_no_room() {
    let root = project(&format!("stress_corridor_{}", std::process::id()), &CORRIDOR);
    let mut app = editor(&root);
    let s = stress(&mut app);
    assert_eq!(s["declared"], 2, "{s}");
    assert_eq!(s["room"], Value::Null, "a corridor has no piece open on three sides: {s}");
    // A loop of four turns is four corners, each facing a different way, in the order north-west,
    // north-east, south-west, south-east: the unturned corner (open east and south) stands north-west.
    assert_eq!(
        s["ring"],
        json!([
            {"piece": "corner", "turn": 0},
            {"piece": "corner", "turn": 270},
            {"piece": "corner", "turn": 90},
            {"piece": "corner", "turn": 180},
        ]),
        "{s}"
    );
}

#[test]
fn a_wall_and_a_middle_open_the_kit_into_a_room() {
    let mut kit = CORRIDOR.to_vec();
    kit.push(("wall", "ESW"));
    kit.push(("middle", "NESW"));
    let root = project(&format!("stress_room_{}", std::process::id()), &kit);
    let mut app = editor(&root);
    let s = stress(&mut app);
    assert_eq!(s["declared"], 4, "{s}");
    let rows: Vec<Vec<String>> = s["room"]
        .as_array()
        .unwrap_or_else(|| panic!("a four-sided middle opens the kit into a room: {s}"))
        .iter()
        .map(piece_names)
        .collect();
    // The middle meets a neighbour on four sides, so only the four-sided piece stands there. Outer
    // faces are not asked, and pieces are tried in id order, so each edge takes `middle` too (it sorts
    // before `wall`), and each corner the first piece that turns there: `corner`.
    assert_eq!(
        rows,
        vec![
            vec!["corner", "middle", "corner"],
            vec!["middle", "middle", "middle"],
            vec!["corner", "middle", "corner"],
        ],
        "{s}"
    );
    // The room's corners in the ring's order and turns, so rows run north to south and west to east.
    let corners: Vec<&Value> = [(0, 0), (0, 2), (2, 0), (2, 2)]
        .iter()
        .map(|&(r, c)| &s["room"][r][c])
        .collect();
    let ring: Vec<&Value> = s["ring"]
        .as_array()
        .unwrap_or_else(|| panic!("a kit with corners closes a loop: {s}"))
        .iter()
        .collect();
    assert_eq!(corners, ring, "{s}");

    // Without the four-sided piece the walls alone make no room.
    let root = project(
        &format!("stress_walls_{}", std::process::id()),
        &[CORRIDOR[0], CORRIDOR[1], ("wall", "ESW")],
    );
    let s = stress(&mut editor(&root));
    assert_eq!(s["room"], Value::Null, "{s}");
    assert_eq!(s["declared"], 3, "{s}");
}

#[test]
fn an_unlabelled_kit_declares_nothing_and_builds_nothing() {
    let root = project(
        &format!("stress_blank_{}", std::process::id()),
        &[("blank", ""), ("plain", "")],
    );
    let mut app = editor(&root);
    assert_eq!(
        stress(&mut app),
        json!({"declared": 0, "ring": null, "room": null}),
        "no tokens: nothing to test, and nulls that say so"
    );
}

#[test]
fn an_edit_to_the_library_reaches_the_next_answer() {
    let root = project(&format!("stress_edit_{}", std::process::id()), &CORRIDOR);
    let mut app = editor(&root);
    // Past the watcher's first poll, so the library's stamp has been taken.
    for _ in 0..8 {
        app.update();
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert_ne!(stress(&mut app)["ring"], Value::Null);

    // A corner that only faces east turns nothing: the kit can no longer close a loop.
    set_faces(&root, "corner", "E");
    let mut last = Value::Null;
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.update();
        last = stress(&mut app);
        if last["ring"].is_null() {
            break;
        }
    }
    assert_eq!(last["ring"], Value::Null, "the edited library never reached grimoire_state: {last}");
    assert_eq!(last["declared"], 2, "{last}");
}
