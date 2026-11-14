//! **Every operation says where its change is seen** (AF-01, #357).
//!
//! The editor follows the Familiar: after every change an agent makes, the person's view goes to it.
//! That needs one answer per operation to *where the change is*, and the answer has to come from the
//! operation itself — `Outcome` says what changed and not where to look, and most changes are not on
//! the map at all (a library piece, a composition, a rig, a data file, a note).
//!
//! `Operation::shows` is that answer, and it has **no default**, so a new operation cannot leave it
//! out — the same rule `writes` follows (#198). `Shown::Nowhere` is the exception, and it is pinned by
//! name below: an operation allowed to claim "nothing to look at" is a decision a reader can see, and
//! a new one shows up as a diff to that list rather than as an invisible omission.
//!
//! # What is proved here
//!
//! 1. **Every registered operation answers**, and the ones answering `Nowhere` are exactly
//!    [`PINNED_NOWHERE`].
//! 2. **A place, a move, a removal and an undo of a placement each name a rectangle that holds the
//!    placement's footprint** — where it stood before the change and where it stands after. The
//!    footprint is read from the map, so `shows` cannot pass by agreeing with itself.
//! 3. **A preview and its apply name the same place**, from the same parameters: a person watching
//!    the Familiar sees what is about to change before it changes, and the two cannot disagree.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Shown};
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

/// **The operations allowed to say `Nowhere`**, by name: the ones whose change is a person's own view
/// (a panel, a tool, a door, a selection), a read that changes nothing, or a write of a map that is
/// already on screen.
///
/// **A name added here is a decision, and it should be justified in the operation's own doc.** A name
/// that stops being `Nowhere` fails this test too, so the list cannot go stale in either direction.
const PINNED_NOWHERE: &[&str] = &[
    // A person's own view: a panel, a rung, the next click's tool, the shelf's selection, the door.
    "view_cycle_grid",
    "view_toggle_journal",
    // Owner exception (2026-11-10): the list grows by the operation #129 slice 21 registers, an overlay on the
    // rig bench that a person's own view toggles, like the journal above.
    "view_toggle_ghost",
    // Assumed owner exception (2026-11-11): the list grows by the three operations #237 registers, what the outliner's
    // eye and locks hold — a person's own view of the map, which nothing on the map shows, like the rung above.
    "placement_set_hidden",
    "layer_set_visible",
    "placement_set_locked",
    // Assumed owner exception (2026-11-11): the list grows by the two operations #129 slice 22 registers, the composition
    // picker's cursor and its hold on the arrows, which a person's own view holds, like the rung above.
    "composition_picker_walk",
    "composition_picker_toggle",
    // Assumed owner exception (2026-11-12): the list grows by the three operations #129 slice 23 registers, the COMPOSE tab's
    // cursors and which list has the arrows, which a person's own view holds, like the picker's above.
    "compose_walk",
    "compose_focus",
    "compose_carousel_step",
    // Assumed owner exception (2026-11-12): the list grows by the two operations #129 slice 24 registers, the rig bench's
    // cursor and the Tiles page's, which a person's own view holds, like the picker's above.
    "rig_walk",
    // Assumed owner exception (2026-11-13): the list grows by the operation #129 slice 32 registers, which shows the rig bench's
    // project-wide summary and asks the watcher to measure: what a person's own view holds, like the rig bench's cursor above.
    "rig_check_all",
    "tile_page_walk",
    // Assumed owner exception (2026-11-12): the list grows by the two operations #129 slice 25 registers, which of the candidate
    // list's groups are folded and which filter box has the keyboard, which a person's own view holds, like the picker's above.
    "pack_fold",
    "filter_focus",
    // Assumed owner exception (2026-11-13): the list grows by the three operations #129 slice 26 registers, whether the rig
    // bench's figure plays, the phase a scrub holds and the stage camera's framing, which a person's own view holds, like the picker's above.
    "stage_play",
    "stage_scrub",
    "stage_camera",
    // Assumed owner exception (2026-11-13): the list grows by the operation #129 slice 33 registers, which reads the Meshes tab's
    // pack directory again: the candidate list a person is looking at, like the lattice cursor below.
    "kit_rescan",
    // Assumed owner exception (2026-11-13): the list grows by the two operations #129 slice 34 registers, which hold or drop the
    // labeller's batch: the status band and the Meshes tab's tab badge a person is looking at, like the stage's controls above.
    "labels_walk",
    "labels_abandon",
    // Assumed owner exception (2026-11-13): the list grows by the five operations #129 slice 36 registers, which move the Tiles tab
    // between its pages, open and close the tile in hand and name a new one: a draft in the tab's own state until `Cmd+S` saves
    // it, which a person's own view holds, like the tile in hand's operations above.
    "tile_page_enter",
    "tile_page_leave",
    "tile_open",
    "tile_close",
    "tile_new",
    // Assumed owner exception (2026-11-13): the list grows by the operation #129 slice 27 registers, the lattice grid's cursor,
    // which a person's own view holds, like the picker's above.
    "lattice_cursor_step",
    // Assumed owner exception (2026-11-13): the list grows by the six operations #129 slice 28 registers, which shape the
    // tile in hand on the Tiles tab (take the piece, walk the members, deepen the ladder, turn, remove, empty): a draft in
    // the tab's own state until it is saved, which a person's own view holds, like the picker's above.
    "tile_hand_arm",
    "tile_member_focus",
    "tile_grid_deepen",
    "tile_member_turn",
    "tile_member_remove",
    "tile_clear",
    // Assumed owner exception (2026-11-13): the list grows by the four operations #129 slice 29 registers, which edit the
    // draft on the Tiles tab (nudge the focused member, put it flush, drop a piece, drop a hole): the same draft as the six
    // above, which a person's own view holds.
    "tile_member_nudge",
    "tile_member_flush",
    "tile_member_drop",
    "tile_slot_drop",
    // Assumed owner exception (2026-11-13): the list grows by the operation #129 slice 30 registers, which steps the Tiles tab's
    // own history of the draft back and forward: the same draft as the ten above, which a person's own view holds.
    "tile_history_step",
    // Assumed owner exception (2026-11-13): the list grows by the two operations #129 slice 31 registers, which pan and turn the
    // Map camera: where a person looks from, which a person's own view holds, like the picker's above.
    "camera_pan",
    "camera_turn",
    "tool_arm",
    "snip_arm",
    "brush_arm",
    "composition_arm",
    "piece_select",
    "door_open",
    // A proposal thrown away: the map was never touched by it.
    "layout_discard",
    // A game process, which is not the editor's project.
    "play",
    "play_stop",
    // A read: it changes nothing, so there is nothing to look at that is not already there.
    "crowd_preview",
    "dialogue_preview",
    "light_map",
    "light_pool",
    "people_run",
    "destructible_hit",
    "label_suggest",
    // A save writes the map that is already on screen.
    "map_save",
];

/// **The smallest parameter set this schema requires**, so an operation is asked in a form it
/// accepts: every `required` property gets a placeholder of the type it declares.
///
/// **It is a stand-in, not a valid call** — `shows` is pure and answers for parameters a commit would
/// reject, which is what lets a preview name a place before anything is written. What the walk needs
/// is that each answer is about the *operation*, so an operation that names a note is asked with an
/// id rather than with nothing.
fn least_params(schema: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    let required = schema.get("required").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    let properties = schema.get("properties").and_then(|p| p.as_object()).cloned().unwrap_or_default();
    for key in required.iter().filter_map(|k| k.as_str()) {
        let spec = properties.get(key).cloned().unwrap_or(serde_json::Value::Null);
        let kind = spec.get("type").and_then(|t| t.as_str()).unwrap_or("string");
        let value = match kind {
            "number" => json!(spec.get("minimum").and_then(|m| m.as_f64()).unwrap_or(1.0)),
            "integer" => json!(
                spec.get("minimum")
                    .and_then(|m| m.as_u64())
                    .unwrap_or(1)
                    .max(spec.get("minimum").and_then(|m| m.as_u64()).unwrap_or(1))
            ),
            "boolean" => json!(false),
            "array" => {
                let count = spec.get("minItems").and_then(|m| m.as_u64()).unwrap_or(1).max(1);
                let item = spec.get("items").cloned().unwrap_or(serde_json::Value::Null);
                let one = match item.get("type").and_then(|t| t.as_str()).unwrap_or("number") {
                    "string" => json!("x"),
                    _ => json!(1),
                };
                serde_json::Value::Array(vec![one; count as usize])
            }
            "object" => json!({}),
            // **A string is its first enum value where the schema states one**, so an axis, a door or
            // an envelope is a value the operation would accept.
            _ => match spec.get("enum").and_then(|e| e.as_array()).and_then(|e| e.first()) {
                Some(v) => v.clone(),
                None => json!("x"),
            },
        };
        out.insert(key.to_owned(), value);
    }
    serde_json::Value::Object(out)
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> (serde_json::Value, Shown) {
    let answer = call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    );
    let shown = shown_of(&answer);
    (answer, shown)
}

fn preview(app: &mut App, operation: &str, parameters: serde_json::Value) -> (serde_json::Value, Shown) {
    let answer = call(
        app,
        "grimoire/operation_preview",
        serde_json::json!({"operation": operation, "parameters": parameters}),
    );
    let shown = shown_of(&answer);
    (answer, shown)
}

/// **The `shown` field of an answer**, parsed back into the type the editor prints it from — so the
/// shape an agent reads is the shape the tests check.
fn shown_of(answer: &serde_json::Value) -> Shown {
    let field = answer
        .get("shown")
        .unwrap_or_else(|| panic!("the answer names where its change is seen: {answer}"));
    serde_json::from_value(field.clone())
        .unwrap_or_else(|e| panic!("`shown` does not read as a `Shown`: {e}: {field}"))
}

fn open(app: &App) -> bevy_grimoire_core::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

/// **The footprint of a placement as the map holds it**: a fixture piece is 1.0 × 1.0 m and faces +z,
/// so its floor box is half a metre each way from its centre. Read from the map, never from `shows`,
/// which is the point of the check.
const HALF_M: f32 = 0.5;

fn footprint(app: &App, id: &str) -> Option<[f32; 4]> {
    let map = open(app);
    let p = map.placements.iter().find(|p| p.id == id)?;
    let (x, z) = p.at;
    Some([x - HALF_M, z - HALF_M, x + HALF_M, z + HALF_M])
}

/// Whether `rect` holds `box_`, with a millimetre of slack for float rounding.
fn holds(rect: [f32; 4], box_: [f32; 4]) -> bool {
    const SLACK_M: f32 = 1e-3;
    rect[0] <= box_[0] + SLACK_M
        && rect[1] <= box_[1] + SLACK_M
        && rect[2] >= box_[2] - SLACK_M
        && rect[3] >= box_[3] - SLACK_M
}

fn rect_of(shown: &Shown, what: &str) -> [f32; 4] {
    match shown {
        Shown::Map { rect, .. } => *rect,
        other => panic!("{what} names a place on the map, and answered {other:?}"),
    }
}

/// **Every registered operation answers, and `Nowhere` is exactly the pinned set.**
#[test]
fn every_operation_says_where_its_change_is_seen() {
    let root = fixtures::Fixture::new(&format!("shows_registry_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let app = editor(&root);
    let project = app.world().resource::<Project>().clone();
    let open_map = app.world().resource::<OpenMap>().clone();
    let ctx = ops::Ctx {
        project: Some(project),
        open: Some(open_map),
        session: "test".to_owned(),
        ..ops::Ctx::default()
    };

    // **No default**, so this is the compiler's job first: an operation that does not answer does not
    // build. What is left for a test is the *answer*, which no type can check.
    let registry = ops::registry();
    assert!(registry.len() >= 80, "the registry is the whole surface, and it answered {} operations", registry.len());

    let mut nowhere: BTreeSet<&str> = BTreeSet::new();
    let mut checked = 0usize;
    for op in &registry {
        // **Each operation is asked with the parameters its own schema requires**, so the answer is
        // about the operation's nature and not about an empty call: a note lease with no id would be
        // refused, and refusing is not the same as "nothing to look at".
        let params = least_params(&op.schema());
        let shown = op.shows(&ctx, &params);
        checked += 1;
        if shown == Shown::Nowhere {
            nowhere.insert(op.name());
        }
        // **A `Map` answer is a real rectangle**: the frame code refuses an inverted or degenerate
        // box, so an answer that could never be framed is not an answer.
        if let Shown::Map { rect, .. } = &shown {
            let [x0, z0, x1, z1] = *rect;
            assert!(
                rect.iter().all(|v| v.is_finite()) && x1 >= x0 && z1 >= z0,
                "`{}` names the box {rect:?}, which is not a rectangle in map metres",
                op.name()
            );
        }
    }
    assert!(checked >= 80, "every registered operation was asked, and {checked} were");

    let pinned: BTreeSet<&str> = PINNED_NOWHERE.iter().copied().collect();
    let registered: BTreeSet<&str> = registry.iter().map(|op| op.name()).collect();
    let unexpected: Vec<&&str> = nowhere.difference(&pinned).collect();
    // **Only a name this build registers can go stale.** The pinned list spans every build, and an
    // operation behind a family crate's cargo feature is not in the registry of a build without it —
    // `crowd_preview` needs `bevy_orca` and `bevy_stigmergy`, `light_map` `bevy_light_grid`, and so
    // on. Counting those as stale would fail every build that leaves a family crate out, which is
    // every build the gate makes.
    let stale: Vec<&&str> = pinned
        .intersection(&registered)
        .filter(|id| !nowhere.contains(*id))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{} operation(s) answer `Nowhere` and are not on the pinned list: {unexpected:?}. \
         `Nowhere` says a change has nothing a person can look at — if that is true, add the name to \
         PINNED_NOWHERE with the reason; if it is not, name the place.",
        unexpected.len()
    );
    assert!(
        stale.is_empty(),
        "{} pinned operation(s) no longer answer `Nowhere`: {stale:?}. Take them off PINNED_NOWHERE, \
         so the list keeps naming what it claims to.",
        stale.len()
    );
    // **A pinned name no build has is a typo, and the names this build does not register are the
    // ones a family crate's cargo feature carries.** So the list is checked against the *feature
    // names*, which are the operations the gate's own build leaves out, and a name in neither set
    // fails here. The cost is that this file states which feature each absent name belongs to.
    const NOT_IN_THIS_BUILD: &[(&str, &str)] = &[
        ("crowd_preview", "bevy_orca"),
        ("destructible_hit", "bevy_carnage"),
        ("destructible_set", "bevy_carnage"),
        ("dialogue_preview", "bevy_speech_bubbles"),
        ("light_map", "bevy_light_grid"),
        ("light_pool", "bevy_lights"),
        ("terrain_preset", "bevy_elements"),
    ];
    let gated: BTreeMap<&str, &str> = NOT_IN_THIS_BUILD.iter().copied().collect();
    let mut unknown: Vec<&&str> = Vec::new();
    for name in pinned.iter().filter(|n| !registered.contains(*n)) {
        match gated.get(name) {
            Some(_) => {}
            None => unknown.push(name),
        }
    }
    assert!(
        unknown.is_empty(),
        "PINNED_NOWHERE names {unknown:?}, which this build does not register and no cargo feature \
         of this crate explains. A name in that list is an operation's, so a name with no operation \
         behind it is a typo."
    );
    // **And a pinned name whose feature this build DOES have must be registered**, or the gate's own
    // build has stopped shipping an operation it used to.
    for (name, feature) in NOT_IN_THIS_BUILD {
        let has_feature = match *feature {
            "bevy_orca" => cfg!(feature = "bevy_orca"),
            "bevy_carnage" => cfg!(feature = "bevy_carnage"),
            "bevy_speech_bubbles" => cfg!(feature = "bevy_speech_bubbles"),
            "bevy_light_grid" => cfg!(feature = "bevy_light_grid"),
            "bevy_lights" => cfg!(feature = "bevy_lights"),
            "bevy_elements" => cfg!(feature = "bevy_elements"),
            other => panic!("`{other}` is not a feature this test knows about"),
        };
        if has_feature && pinned.contains(name) {
            assert!(
                registered.contains(name),
                "this build has `{feature}`, so `{name}` is a registered operation, and it is not. \
                 Either the feature no longer registers it or the name is wrong."
            );
        }
    }
}

/// **A place, a move, a removal and an undo of a placement each name a rectangle that holds the
/// placement's footprint**, before and after the change.
#[test]
fn a_change_names_a_box_that_holds_the_placement() {
    let root = fixtures::Fixture::new(&format!("shows_footprint_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .descriptor("site/chair", "alpha")
        .descriptor("site/table", "alpha")
        .place_as("chair", "site/chair", (2.0, 2.0))
        .place_as("table", "site/table", (2.0, 0.0))
        .build("m");
    let mut app = editor(&root);

    // ── a place: an id is minted at commit, so the answer names the box and no ids ────────────────
    let at = (6.0, 4.0);
    let (answer, shown) = apply(&mut app, "placement_add", serde_json::json!({"piece": "site/wall", "at": [at.0, at.1]}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let placed = answer["log_entry"]["params"]["piece"].as_str().expect("the piece it placed");
    assert_eq!(placed, "site/wall", "{answer}");
    let before = footprint(&app, "wall@1").or_else(|| footprint(&app, "wall@0"));
    let minted: Vec<String> = open(&app)
        .placements
        .iter()
        .map(|p| p.id.clone())
        .filter(|id| id.starts_with("wall@"))
        .collect();
    assert_eq!(minted.len(), 1, "one wall stands, and it is the one just placed: {minted:?}");
    let after = footprint(&app, &minted[0]).unwrap_or_else(|| panic!("the placed wall is on the map"));
    let _ = before;
    assert!(
        holds(rect_of(&shown, "a place"), after),
        "a place names the box it landed in: {shown:?} does not hold {after:?}"
    );
    assert!(
        matches!(&shown, Shown::Map { placements, .. } if placements.is_empty()),
        "a place mints its id at commit, so the answer names the box and no ids: {shown:?}"
    );

    // ── a move: the relations solve walks the chair round to face the table ───────────────────────
    //
    // **There is no `placement_move` in the registry** — the moves are the biggest verb #129 still
    // owes and they wait on the owner — so the move this proves is the one operation that walks a
    // placement across the floor for it: the relations solver.
    let relations = root.join("assets/grimoire/maps/m.relations.yaml");
    std::fs::write(&relations, "{version: 1, relations: [{of: \"chair\", kind: !Near {to: \"table\", within: 1.0}}]}")
        .unwrap_or_else(|e| panic!("{relations:?}: {e}"));
    // Reopened, so the solver reads the relations file this test just wrote.
    let mut app = editor(&root);
    let before_move = footprint(&app, "chair").expect("the chair stands");
    let (answer, shown) = apply(&mut app, "relations_solve", serde_json::json!({}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let after_move = footprint(&app, "chair").expect("the chair still stands");
    let rect = rect_of(&shown, "a move");
    assert!(
        holds(rect, before_move) && holds(rect, after_move),
        "a move names the box it left and the box it landed in: {rect:?} does not hold both \
         {before_move:?} and {after_move:?} ({shown:?})"
    );
    assert!(
        matches!(&shown, Shown::Map { placements, .. } if placements.iter().any(|id| id == "chair")),
        "a move outlines the placement it moved: {shown:?}"
    );

    // ── a removal: the piece is gone afterwards, so the box is where it stood ─────────────────────
    let before_removal = footprint(&app, "table").expect("the table stands");
    let (answer, shown) = apply(&mut app, "placement_remove", serde_json::json!({"id": "table"}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert!(footprint(&app, "table").is_none(), "the table is off the map");
    assert!(
        holds(rect_of(&shown, "a removal"), before_removal),
        "a removal names the box the piece stood in: {shown:?} does not hold {before_removal:?}"
    );
    let id = answer["operation_id"].as_str().expect("an id").to_owned();

    // ── an undo of a placement: it puts the piece back, and names where it now stands ─────────────
    let undone = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    let shown = shown_of(&undone);
    let after_undo = footprint(&app, "table").expect("the table is back");
    assert_eq!(after_undo, before_removal, "it is back where it stood");
    assert!(
        holds(rect_of(&shown, "an undo of a placement"), after_undo),
        "an undo shows what its inverse's parameters show, which is where the piece stands again: \
         {shown:?} does not hold {after_undo:?}"
    );
}

/// **A preview and its apply name the same place**, for the same parameters.
#[test]
fn a_preview_and_its_apply_name_the_same_place() {
    let root = fixtures::Fixture::new(&format!("shows_preview_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place_as("wall", "site/wall", (1.0, 1.0))
        .build("m");
    // **The shipped file's shape**: one token a line and comments between, which is what the
    // vocabulary's own splice edits. The fixture writes each axis on a single line, and that splice
    // refuses a list it cannot find.
    std::fs::write(
        root.join("assets/grimoire/vocab.yaml"),
        r#"# what a thing is
kind:
  tokens:
  - name: prop
    note: a thing
effects:
  tokens:
  - name: inert
    note: does nothing
# how it looks
look:
  tokens:
  - name: plain
    note: unremarkable
surfaces:
  tokens:
  - name: worktop
    note: a top
capabilities:
  tokens: []
edge:
  tokens:
  - name: wall
    note: a solid run-face
slot:
  tokens: []
"#,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(&root);

    // **One of each kind of answer**: a place on the map, a piece in the library, a file, and a
    // setting. A preview that named a different place from its apply would send a person watching
    // the Familiar to the wrong door.
    let cases = [
        ("placement_add", serde_json::json!({"piece": "site/wall", "at": [5.0, 5.0]})),
        ("placement_set_yaw", serde_json::json!({"id": "wall", "yaw": 90.0})),
        ("placement_remove", serde_json::json!({"id": "wall"})),
        ("map_set_settings", serde_json::json!({"bounds": [32.0, 8.0, 32.0]})),
        ("map_rename", serde_json::json!({"name": "town_two"})),
        ("vocab_add_token", serde_json::json!({"axis": "kind", "name": "crate", "note": "a boxy thing"})),
        ("layout_propose", serde_json::json!({"source": "learned"})),
        ("note_add", serde_json::json!({"cells": [0, 0, 2, 2], "text": "a wall here"})),
    ];
    let mut agreed = 0usize;
    for (operation, params) in cases {
        let (previewed, before) = preview(&mut app, operation, params.clone());
        let (applied, after) = apply(&mut app, operation, params.clone());
        assert_eq!(
            previewed["outcome"], "previewed",
            "`{operation}` previews with these parameters: {previewed}"
        );
        assert!(
            matches!(applied["outcome"].as_str(), Some("applied") | Some("waiting")),
            "`{operation}` applies with the same parameters: {applied}"
        );
        assert_eq!(
            before, after,
            "`{operation}` previewed as {before:?} and applied as {after:?}; a preview and its \
             apply are the same parameters and must name the same place"
        );
        agreed += 1;
    }
    assert_eq!(agreed, 8, "every case agreed, and {agreed} did");
}
