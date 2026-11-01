//! **A change says what it broke (E-074).**
//!
//! The brief requires it in as many words (`emerge-feature-loop.md`:201): *"Relevant validators run
//! after every mutation. Their failures appear in the tool result."* Until this, an agent's
//! `grimoire_apply_operation` answer was `"applied"` and nothing else — a `placement_set_yaw` that
//! opened an edge seam on the far side of the map read exactly like one that changed nothing, and the
//! author found out by looking.
//!
//! # What is being proved
//!
//! 1. **A placement that makes an edge fault says so**, naming the pieces and the direction — and
//!    the total on the whole map, so "one new" is distinguishable from "one was already here".
//! 2. **A fault that was already there is not reported as new.** The second placement's answer names
//!    zero new faults while the total still reads one. This is the field's whole usability: an agent
//!    that is told twenty standing faults every time it moves a crate learns to ignore them.
//! 3. **Opening a map that overlaps says so**, with the pair of placement ids.
//! 4. **A preview and a read carry no checks at all** — the field is absent from a preview's answer
//!    and `null` on a read. A preview changes nothing, so it has nothing to check.
//! 5. **A capture sequence reports the same checks**, because it is a second front end for the same
//!    change and the two must not drift.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::sequencer::{Operation, SequenceParams, Sequencer};

use crate::fixtures;

/// **The editor with the bridge open**, on whichever map the fixture wrote.
///
/// Eight updates, as `an_agent_can_place_a_piece.rs` does: the bridge's systems, the map watcher and
/// the edge-fault panel all settle inside that budget, and a test that read the answer after four saw
/// a half-installed world.
fn editor_with_bridge(root: &std::path::Path, map: &str) -> App {
    let mut app = harness::build_headless(root, map, None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": operation,
            "parameters": parameters,
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

/// **Two half-metre pieces that disagree down one seam**, which is core's
/// `a_single_declared_token_re_enables_the_whole_check` in a fixture: a `wall` presenting the `wall`
/// token and a `pane` presenting `glass`. One 0.5 m cell each, so they abut exactly.
fn disagreeing_pair(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .side(8.0)
        .authored_lattice("wall", "alpha", "wall", 1)
        .edge_tokens(&["glass"])
        .authored_lattice("pane", "alpha", "glass", 1)
        // **Half a metre apart, because that is flush**: the fixture writes `at` with `{:.1}`, so
        // `(0.25, 0.5)` lands at `0.2` and leaves a 0.05 m gap where the seam should be. Two 0.5 m
        // pieces centred at 0.5 and 1.0 meet exactly at x = 0.75.
        .place("wall", (0.5, 0.5))
        .build("m")
}

/// **A placement that makes an edge fault says so, and names both what is new and what is there.**
///
/// The first answer carries the fault; the second, far from anything, carries none of it as new while
/// the map still holds one.
#[test]
fn a_placement_that_makes_an_edge_fault_says_so() {
    let root = disagreeing_pair("checks-edge");
    let mut app = editor_with_bridge(&root, "m");

    let answer = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "pane", "at": [1.0, 0.5]}),
    );
    assert_eq!(
        answer["outcome"], "applied",
        "the placement is what this test needs to have happened: {answer}"
    );
    let edge = &answer["checks"]["edge_faults"];
    assert_eq!(
        edge["new_total"], 1,
        "the pane against the wall is a disagreeing seam and the answer must say so: {answer}"
    );
    let named = edge["new"][0]["a"] == "wall@0" || edge["new"][0]["b"] == "wall@0";
    assert!(named, "the new fault must name the wall: {answer}");
    assert_eq!(edge["total"], 1, "and the whole map holds exactly that one: {answer}");
    assert_eq!(
        answer["checks"]["overlaps"]["new_total"], 0,
        "two half-metre pieces flush are not an overlap: {answer}"
    );

    // **The second placement is nowhere near anything**, so it introduces nothing — and the fault
    // the first one made is still on the map, which is why `total` stays at one.
    let far = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "pane", "at": [3.25, 0.5]}),
    );
    assert_eq!(far["outcome"], "applied", "{far}");
    assert_eq!(
        far["checks"]["edge_faults"]["new_total"], 0,
        "a fault that was already there is not what this change broke: {far}"
    );
    assert_eq!(
        far["checks"]["edge_faults"]["total"], 1,
        "the standing fault is still on the map: {far}"
    );
}

/// **Opening a map whose two placements sit in one another says so**, by the pair of ids.
///
/// `map_open` declares no inverse — the map it loaded had no counterpart in the map it replaced — so
/// it is the one document-changing operation the registry's own test cannot name, and the one whose
/// findings a reader most needs. Its `before` is the map that was open, so opening the clean map says
/// zero and opening the crowded one says one.
#[test]
fn opening_a_map_that_overlaps_says_so() {
    let root = fixtures::Fixture::new("checks-overlap")
        .side(8.0)
        .descriptor("crate", "alpha")
        .place("crate", (1.0, 1.0))
        .place("crate", (1.0, 1.0))
        .build("crowded");
    let mut app = editor_with_bridge(&root, "crowded");

    let made = apply(
        &mut app,
        "map_create",
        serde_json::json!({"name": "empty", "bounds": [8, 4, 8]}),
    );
    assert_eq!(made["outcome"], "applied", "{made}");

    let empty = apply(&mut app, "map_open", serde_json::json!({"name": "empty"}));
    assert_eq!(empty["outcome"], "applied", "{empty}");
    assert_eq!(
        empty["checks"]["overlaps"]["total"], 0,
        "the crowded map's overlap must not follow the empty map onto the screen: {empty}"
    );

    let crowded = apply(&mut app, "map_open", serde_json::json!({"name": "crowded"}));
    assert_eq!(crowded["outcome"], "applied", "{crowded}");
    assert_eq!(
        crowded["checks"]["overlaps"]["new_total"], 1,
        "two crates in one cell is an overlap and opening that map is when it arrives: {crowded}"
    );
    assert_eq!(crowded["checks"]["overlaps"]["total"], 1, "{crowded}");
    let pair = &crowded["checks"]["overlaps"]["new"][0];
    assert_eq!(
        pair,
        &serde_json::json!({"a": "crate@0", "b": "crate@1"}),
        "the pair is the map's own ids, smaller first: {crowded}"
    );
}

/// **A preview and a read carry no checks**, which is not the same as carrying an empty one.
///
/// A preview changes nothing, so a finding it named would be a finding the change did not make. The
/// answer omits the field entirely, and a read's is `null`: `null` names *no document changed*, and an
/// object with two zeroes would claim a check ran.
#[test]
fn a_preview_and_a_read_carry_no_checks() {
    let root = disagreeing_pair("checks-preview");
    let mut app = editor_with_bridge(&root, "m");

    let previewed = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "placement_add",
            "parameters": {"piece": "pane", "at": [1.0, 0.5]},
            "dry_run": true,
        }),
    )
    .unwrap_or_else(|e| panic!("the preview answers: {e}"));
    assert_eq!(previewed["outcome"], "previewed", "{previewed}");
    assert!(
        previewed.get("checks").is_none(),
        "a preview changes nothing and must not carry checks: {previewed}"
    );

    // **A read that touches no document.** `brush_arm` changes the armed piece, not the map, and the
    // registry's own `Undo::None` is what says so.
    let armed = apply(
        &mut app,
        "brush_arm",
        serde_json::json!({"descriptor": "pane"}),
    );
    assert_eq!(
        armed["outcome"], "applied",
        "arming is an apply with no document change: {armed}"
    );
    assert!(
        armed.get("checks").is_some_and(serde_json::Value::is_null),
        "an operation that changes no document answers `checks: null`: {armed}"
    );
}

/// **A rename keeps its document, so it reports nothing as new.**
///
/// The bug this pins was real and was introduced by the first version of the cross-map guard: a
/// finding is matched by placement id, so comparing `map_open` against the map it replaces would
/// under-report — but guarding that with the open map's **file path** made `map_rename` over-report
/// instead, because a rename keeps the same document and points `OpenMap::map_path` at the new file.
/// Renaming Sanctuary's `town` would have answered `overlaps.new_total: 781`, "your rename broke 781
/// things".
///
/// The fixture carries the same two-crate overlap the `opening_a_map_that_overlaps_says_so` test
/// uses, so the totals here are the crowded map's own.
#[test]
fn a_rename_reports_nothing_as_new() {
    let root = fixtures::Fixture::new("checks-rename")
        .side(8.0)
        .descriptor("crate", "alpha")
        .place("crate", (1.0, 1.0))
        .place("crate", (1.0, 1.0))
        .build("before_name");
    let mut app = editor_with_bridge(&root, "before_name");

    let renamed = apply(
        &mut app,
        "map_rename",
        serde_json::json!({"name": "after_name"}),
    );
    assert_eq!(renamed["outcome"], "applied", "{renamed}");
    assert_eq!(
        renamed["checks"]["overlaps"]["new_total"], 0,
        "a rename changed the name, not the map: {renamed}"
    );
    assert_eq!(
        renamed["checks"]["overlaps"]["total"], 1,
        "and the map still holds its one overlap, unchanged: {renamed}"
    );
}

/// **A capture sequence reports the same checks**, because it is the same change through a second
/// front end and the two answering differently is the drift this crate's parity tests exist for.
#[test]
fn a_sequence_reports_the_same_checks() {
    let root = disagreeing_pair("checks-seq");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }

    let out = std::env::temp_dir().join(format!("grimoire-checks-seq-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("the out directory is made");

    let params = SequenceParams {
        label: "checks".to_owned(),
        out_dir: out.to_string_lossy().into_owned(),
        seconds: 0.5,
        fps: 4,
        operation: Some(Operation {
            name: "placement_add".to_owned(),
            parameters: serde_json::json!({"piece": "pane", "at": [1.0, 0.5]}),
        }),
    };
    let id = {
        let mut sequencer = app.world_mut().resource_mut::<Sequencer>();
        sequencer.start(&params, 0.0).expect("the sequence starts")
    };

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut status = serde_json::Value::Null;
    while std::time::Instant::now() < deadline {
        app.update();
        status = app.world().resource::<Sequencer>().status(id);
        if matches!(
            status.get("status").and_then(|s| s.as_str()),
            Some("done") | Some("failed")
        ) {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&out);

    // **Headless, the run ends "failed" with "no render world"**, exactly as
    // `a_sequence_turn_is_on_the_receipt.rs` records: no frame can be written and the sequencer says
    // so rather than report success. The operation ran before the frames did, so its result is read.
    assert_eq!(
        status["operation_result"]["checks"]["edge_faults"]["new_total"], 1,
        "the sequence applied the same placement and must report the same fault: {status}"
    );
}

