//! **An agent can arm a brush piece, and the palette click is the same operation.**
//!
//! E-037: arming a piece was a click handler in `editor.rs` that no operation named, so an agent
//! could place pieces but could not choose one. `brush_arm` is the registered operation; the
//! palette click dispatches it. These tests go through the registered BRP method, the way an
//! agent's request goes, and assert on the editor's own state and on the log.

use bevy::prelude::*;
use bevy_cauldron::editor::{EditorState, Tool, arm_piece};
use bevy_cauldron::harness;
use bevy_cauldron::ops;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .build("m");
    open(root)
}

/// The same library with one wall already on the map, so an undo has a removal to take back.
fn editor_with_a_wall(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .place("site/wall", (1.0, 1.0))
        .build("m");
    open(root)
}

fn open(root: std::path::PathBuf) -> App {
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn placement_ids(app: &App) -> Vec<String> {
    app.world().resource::<OpenMap>().map.placements.iter().map(|p| p.id.clone()).collect()
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

fn arm(app: &mut App, descriptor: &str) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "brush_arm",
            "parameters": {"descriptor": descriptor},
            "dry_run": false,
        }),
    )
}

/// **The piece the editor holds armed**, by id: the brush is an index into the library.
fn armed(app: &App) -> Option<String> {
    let project = app.world().resource::<Project>();
    let state = app.world().resource::<EditorState>();
    let ix = state.brush?;
    project.library.descriptors.get(ix).map(|d| d.id.clone())
}

fn log_lines(app: &App) -> usize {
    let project = app.world().resource::<Project>();
    std::fs::read_to_string(ops::oplog_path(project))
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

#[test]
fn an_agent_arms_a_piece_and_the_editor_holds_it() {
    let mut app = editor("arm_known");
    assert_ne!(
        armed(&app).as_deref(),
        Some("site/gate"),
        "the start must hold a different piece, or arming it proves nothing"
    );

    let answer = arm(&mut app, "site/gate");
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(
        armed(&app).as_deref(),
        Some("site/gate"),
        "the editor's brush is the piece the agent named"
    );
}

#[test]
fn an_unknown_piece_is_refused_and_nothing_changes() {
    let mut app = editor("arm_unknown");
    arm(&mut app, "site/gate");
    let before_brush = armed(&app);
    let before_log = log_lines(&app);

    let answer = arm(&mut app, "site/no_such_piece");
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("site/no_such_piece") && why.contains("library"),
        "the refusal names the id and the library it was not found in: {why}"
    );
    assert_eq!(armed(&app), before_brush, "a refused arm leaves the brush as it was");
    assert_eq!(log_lines(&app), before_log, "a refused arm writes no log line");
}

/// **A piece from a kit the map's bash hides is refused by name** (gap 5). The palette cannot show
/// it, so an agent could arm a piece a person could not find, and the armed brush would sit on a
/// piece the map's own palette had put out of reach.
#[test]
fn an_agent_cannot_arm_a_piece_from_a_kit_the_maps_bash_hides() {
    let root = fixtures::Fixture::new("arm_hidden_kit")
        .kit("site", "ozea", &["site/wall"])
        .kit("furn", "pack", &["furn/chair"])
        .bash("only_furniture", &["furn"])
        .build("m");
    let mut app = open(root);
    app.world_mut().resource_mut::<OpenMap>().map.bash = Some("only_furniture".to_owned());
    app.update();
    let before_log = log_lines(&app);
    let before_brush = armed(&app);

    let refused = arm(&mut app, "site/wall");
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("only_furniture") && why.contains("site"),
        "the refusal names the bash and the kit it leaves out: {why}"
    );
    assert_eq!(armed(&app), before_brush, "a refused arm leaves the brush alone");
    assert_eq!(log_lines(&app), before_log, "a refused arm is not logged");

    let ok = arm(&mut app, "furn/chair");
    assert_eq!(ok["outcome"], "applied", "{ok}");
    assert_eq!(armed(&app).as_deref(), Some("furn/chair"));
}

/// **A refusal names the pieces that exist (gap 2).** "The palette lists the library's pieces" sent
/// an agent to a palette it cannot read: `grimoire_state` gave the count of descriptors and no ids. The
/// refusal now lists the closest ids and the total, so the agent can retry with a real one.
#[test]
fn a_refusal_names_the_pieces_an_agent_can_arm() {
    let mut app = editor("arm_names");
    let answer = arm(&mut app, "site/wal");
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("site/wall") && why.contains("site/gate"),
        "the refusal lists the library's ids so the agent can retry: {why}"
    );
    assert!(why.contains("2"), "the refusal states how many pieces the library holds: {why}");
}

#[test]
fn the_palette_click_and_the_operation_leave_the_same_state() {
    // The palette path calls `arm_piece`, which dispatches `brush_arm` and applies its outcome.
    // The operation path is an agent's `grimoire/operation_apply`. Both must leave the same brush
    // and the same tool.
    let mut by_click = editor("arm_click");
    {
        let project = by_click.world().resource::<Project>().clone();
        let open = by_click.world().resource::<OpenMap>().clone();
        let mut state = by_click.world_mut().resource_mut::<EditorState>();
        arm_piece(&mut state, &project, Some(&open), "site/wall");
    }

    let mut by_op = editor("arm_op");
    let answer = arm(&mut by_op, "site/wall");
    assert_eq!(answer["outcome"], "applied", "{answer}");

    assert_eq!(armed(&by_click), armed(&by_op));
    assert_eq!(armed(&by_click).as_deref(), Some("site/wall"));
    assert_eq!(
        by_click.world().resource::<EditorState>().tool,
        by_op.world().resource::<EditorState>().tool,
        "both paths leave the same tool"
    );
}

/// **An arm that could not be logged and changed nothing says so.** The editor took the arm back
/// before the refusal, and the refusal must not say the change is still in place when there was no
/// change to keep.
#[test]
fn an_arm_that_cannot_be_logged_says_nothing_was_changed() {
    let mut app = editor("arm_unlogged");
    let before = armed(&app);
    let log = {
        let project = app.world().resource::<Project>();
        ops::oplog_path(project)
    };
    // A folder where the log file goes: every append is refused.
    let _ = std::fs::remove_file(&log);
    std::fs::create_dir_all(&log).expect("the log path is made a folder");

    let answer = arm(&mut app, "site/gate");
    let text = answer.to_string();
    assert!(
        text.contains("nothing was changed"),
        "an arm that changed nothing must say so: {answer}"
    );
    assert!(
        !text.contains("still in place"),
        "there is no change to keep in place: {answer}"
    );
    assert_eq!(armed(&app), before, "the held piece is unchanged");
    let _ = std::fs::remove_dir_all(&log);
}

/// **The id of the newest line in the log**, the arm's own line when the test has just armed.
fn newest_operation_id(app: &App) -> String {
    let project = app.world().resource::<Project>();
    let text = std::fs::read_to_string(ops::oplog_path(project)).expect("the log is read");
    let last = text
        .lines()
        .rfind(|l| !l.trim().is_empty())
        .expect("the log has a line");
    let line: serde_json::Value = serde_json::from_str(last).expect("the line is JSON");
    line["operation_id"].as_str().expect("the line names its id").to_owned()
}

/// **Undoing an arm by its id says there is nothing to take back.** An arm changes no map and
/// records no inverse, so the log lists it but the undo path must not answer as if the id were
/// missing from the log.
#[test]
fn undo_names_an_arm_as_having_nothing_to_take_back() {
    let mut app = editor("arm_undo_by_id");
    let answer = arm(&mut app, "site/gate");
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let id = newest_operation_id(&app);

    let answer = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    );
    let text = answer.to_string();
    assert!(
        text.contains("nothing to undo"),
        "an arm must be named as having nothing to undo: {answer}"
    );
    assert!(
        !text.contains("is in the log"),
        "the arm is in the log, so the refusal must not say it is not: {answer}"
    );
}

/// **`descriptor: null` puts the brush down**, as Esc does for an armed piece. Before this, the
/// only way for an agent to leave a piece armed was to arm a different one.
#[test]
fn a_null_descriptor_puts_the_brush_down() {
    let mut app = editor("arm_disarm");
    let answer = arm(&mut app, "site/gate");
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(armed(&app).as_deref(), Some("site/gate"));

    let answer = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "brush_arm",
            "parameters": {"descriptor": null},
            "dry_run": false,
        }),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(armed(&app), None, "a null descriptor must leave nothing armed");
}

/// **A missing `descriptor` is still a refusal.** Null is a request to disarm; an absent key is a
/// typo, and a typo must not put the brush down.
#[test]
fn a_missing_descriptor_is_refused_and_the_brush_stays() {
    let mut app = editor("arm_missing");
    let answer = arm(&mut app, "site/gate");
    assert_eq!(answer["outcome"], "applied", "{answer}");

    let answer = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "brush_arm",
            "parameters": {},
            "dry_run": false,
        }),
    );
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(armed(&app).as_deref(), Some("site/gate"));
}

/// **An undo after an arm takes back the change before it, and skips the arm.** The arm is in the log
/// and changes no map, so the undo the person asks for must reach the removal.
#[test]
fn an_undo_after_an_arm_takes_back_the_removal_and_not_the_arm() {
    let mut app = editor_with_a_wall("arm_then_undo");
    let before = placement_ids(&app);
    assert!(before.iter().any(|id| id == "site/wall@0"), "the fixture holds the wall: {before:?}");

    let removed = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_remove", "parameters": {"id": "site/wall@0"}, "dry_run": false}),
    );
    assert_eq!(removed["outcome"], "applied", "{removed}");
    assert!(!placement_ids(&app).iter().any(|id| id == "site/wall@0"));

    let answer = arm(&mut app, "site/gate");
    assert_eq!(answer["outcome"], "applied", "{answer}");

    let undone = call(&mut app, "grimoire/operation_undo", serde_json::json!({"dry_run": false}));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    assert_eq!(placement_ids(&app), before, "the undo puts the removed wall back");
    assert_eq!(armed(&app).as_deref(), Some("site/gate"), "the undo leaves the brush as the arm set it");
}

/// **Arming from the remove tool switches to placing and says what it turned off.** The note is the
/// person's only sign of that, so it must name the tool.
#[test]
fn an_arm_from_the_remove_tool_switches_to_placing_and_says_so() {
    let mut app = editor("arm_from_remove");
    app.world_mut().resource_mut::<EditorState>().tool = Tool::Remove;
    {
        let project = app.world().resource::<Project>().clone();
        let open = app.world().resource::<OpenMap>().clone();
        let mut state = app.world_mut().resource_mut::<EditorState>();
        arm_piece(&mut state, &project, Some(&open), "site/wall");
    }
    let state = app.world().resource::<EditorState>();
    assert_eq!(state.tool, Tool::Place, "arming returns to placing");
    assert_eq!(armed(&app).as_deref(), Some("site/wall"));
    assert_eq!(
        state.status.note_text(),
        "site/wall armed — removal mode off"
    );
}

/// **A refused arm from the remove tool keeps the remove tool, and says why.**
#[test]
fn a_refused_arm_from_the_remove_tool_keeps_the_remove_tool() {
    let mut app = editor("refused_from_remove");
    arm(&mut app, "site/gate");
    let before = armed(&app);
    app.world_mut().resource_mut::<EditorState>().tool = Tool::Remove;
    {
        let project = app.world().resource::<Project>().clone();
        let open = app.world().resource::<OpenMap>().clone();
        let mut state = app.world_mut().resource_mut::<EditorState>();
        arm_piece(&mut state, &project, Some(&open), "site/no_such_piece");
    }
    let state = app.world().resource::<EditorState>();
    assert_eq!(state.tool, Tool::Remove, "a refused arm puts the tool back");
    assert_eq!(armed(&app), before, "a refused arm leaves the brush as it was");
    assert!(
        state.status.problem_text().contains("site/no_such_piece"),
        "the refusal names the piece: {}",
        state.status.problem_text()
    );
}

/// **Each applied arm writes exactly one log line.** The log is what grimoire_read_oplog and undo read.
#[test]
fn each_applied_arm_writes_exactly_one_log_line() {
    let mut app = editor("one_line_each");
    let before = log_lines(&app);
    assert_eq!(arm(&mut app, "site/wall")["outcome"], "applied");
    assert_eq!(arm(&mut app, "site/gate")["outcome"], "applied");
    assert_eq!(log_lines(&app), before + 2, "one line per applied arm, no more");
}

/// **The palette click and the operation leave the same tool, from the remove tool too.** The
/// earlier parity test started both on the place tool, where a tool mismatch cannot show.
#[test]
fn the_palette_click_and_the_operation_agree_on_the_tool_from_remove() {
    let mut by_click = editor("tool_click");
    by_click.world_mut().resource_mut::<EditorState>().tool = Tool::Remove;
    {
        let project = by_click.world().resource::<Project>().clone();
        let open = by_click.world().resource::<OpenMap>().clone();
        let mut state = by_click.world_mut().resource_mut::<EditorState>();
        arm_piece(&mut state, &project, Some(&open), "site/wall");
    }

    let mut by_op = editor("tool_op");
    by_op.world_mut().resource_mut::<EditorState>().tool = Tool::Remove;
    let answer = arm(&mut by_op, "site/wall");
    assert_eq!(answer["outcome"], "applied", "{answer}");

    let click_tool = by_click.world().resource::<EditorState>().tool;
    let op_tool = by_op.world().resource::<EditorState>().tool;
    assert_eq!(click_tool, op_tool, "both paths leave the same tool");
    assert_eq!(op_tool, Tool::Place);
}
