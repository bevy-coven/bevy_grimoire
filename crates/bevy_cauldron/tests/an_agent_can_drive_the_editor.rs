//! **An agent can drive the editor, and the editor says what it did.**
//!
//! The integration test for the 2026-10-21 run's enhancement.
//!
//! # What this proves
//!
//! **That the agent's door works end to end**, over the transport an agent actually uses:
//!
//! 1. The editor registers its operations as BRP commands, and the commands are discoverable.
//! 2. **A preview changes nothing** — the map, the file and the log are all as they were.
//! 3. **An apply changes the map and writes the file**, and the log says so.
//! 4. **A refusal changes nothing**, and says why.
//! 5. **The two front ends agree**: the same operation, dispatched the same way, refuses the same
//!    thing with the same words.
//!
//! # And the latency, because the brief's target is a number
//!
//! **The edit-to-visible latency is measured, not asserted.** The clock starts when the file is
//! written and stops when the editor's state reflects it — and the two paths are measured
//! separately, because they are different paths with different costs:
//!
//! | path | the clock starts at | the dominant term |
//! |---|---|---|
//! | **the file** | the file is written | the watcher's poll (250 ms) |
//! | **the agent** | the BRP call is made | the dispatch and the write |
//!
//! **The target is about a second**, and the test prints both numbers so a regression is visible
//! rather than inferred.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::keys;

use crate::fixtures;

/// **How long to wait for the editor to notice a change, in frames.**
///
/// **The watch polls at 250 ms and the editor's frames are longer than that in a headless run**,
/// so the test waits in frames and the latency it reports is in milliseconds of wall clock.
const SETTLE_FRAMES: usize = 40;

/// **A project with one map, one piece, and a map that has been saved.**
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

/// **The editor, with the agent's door open.
///
/// **The debugger's plugin is added by hand**, because the harness builds the editor's plugin
/// graph *without* the transport — a test process builds several `App`s and cannot have two
/// things bound to one port. **The command table is what this test reads**, and it is registered
/// by the plugin that owns the commands.
fn editor_with_bridge(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

/// **Call one of the editor's BRP commands, the way the MCP server does.**
///
/// **Through the registered method**, not by calling the handler — so the test exercises the
/// same path an agent's request takes, including the parameter parsing and the refusal.
fn call(
    app: &mut App,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
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

/// **The operations the editor knows.**
#[test]
fn the_operations_are_discoverable() {
    let root = project("agentops");
    let mut app = editor_with_bridge(&root);
    let listed = call(&mut app, "grimoire/operation_list", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("the list answers: {e}"));
    let names: Vec<String> = listed["operations"]
        .as_array()
        .expect("the list is an array")
        .iter()
        .map(|o| o["operation"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert!(
        names.contains(&"map_set_settings".to_owned()),
        "the operation list does not name `map_set_settings`, so an agent cannot find the \
         operation. It names: {names:?}"
    );
    // **And the schema is there**, because an agent cannot call an operation whose parameters it
    // cannot describe.
    let schema = &listed["operations"][0]["parameters"];
    assert!(
        schema.is_object(),
        "the operation's parameters have no schema, so an agent would have to guess them: \
         {schema}"
    );
}

/// **A preview changes nothing.**
///
/// **The map, the file and the log are all as they were** — and that is the whole point of a
/// preview: an agent can ask "what would this do" without the author seeing a change.
#[test]
fn a_preview_changes_nothing() {
    let root = project("agentpreview");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    let map_path = map_path(&app);
    let before = std::fs::read_to_string(&map_path).unwrap_or_default();

    let previewed = call(
        &mut app,
        "grimoire/operation_preview",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"bounds": [64.0, 8.0, 64.0]},
        }),
    )
    .unwrap_or_else(|e| panic!("the preview answers: {e}"));
    assert_eq!(
        previewed["outcome"], "previewed",
        "the preview did not preview: {previewed}"
    );
    assert!(
        previewed["would_change"]["bounds"].is_array(),
        "the preview did not say what would change: {previewed}"
    );

    let after = std::fs::read_to_string(&map_path).unwrap_or_default();
    assert_eq!(
        before, after,
        "the preview wrote to the map's file. A preview changes nothing."
    );
}

/// **An apply changes the map, writes the file, and records what it did.**
#[test]
fn an_apply_changes_the_map_and_writes_the_file() {
    let root = project("agentapply");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    let map_path = map_path(&app);
    let before = std::fs::read_to_string(&map_path).unwrap_or_default();

    let applied = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"bounds": [64.0, 8.0, 64.0]},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));
    assert_eq!(
        applied["outcome"], "applied",
        "the apply did not apply: {applied}"
    );
    assert_eq!(
        applied["saved"], true,
        "the apply says it did not write the file, and a change that is only in memory is a change \
         that is lost: {applied}"
    );

    let after = std::fs::read_to_string(&map_path).unwrap_or_default();
    assert_ne!(
        before, after,
        "the apply did not write the map's file. The file and the editor must agree."
    );
    assert!(
        after.contains("64"),
        "the new bounds are not in the file, so the apply did not write what it said it wrote: \
         {after}"
    );

    // **And the log says so**, because the log is the source of truth.
    let log = call(
        &mut app,
        "grimoire/oplog_read",
        serde_json::json!({"since_seq": 0, "limit": 50}),
    )
    .unwrap_or_else(|e| panic!("the log answers: {e}"));
    let entries = log["entries"].as_array().expect("the log is an array");
    assert!(
        !entries.is_empty(),
        "the log is empty after an apply, so the log is not the source of truth: {log}"
    );
    let last = entries.last().expect("there is an entry");
    assert_eq!(
        last["op"], "map_set_settings",
        "the log's last entry is not the operation that was applied: {last}"
    );
    assert_eq!(
        last["source"], "agent",
        "the log does not say an agent did it, so the log cannot tell the human's work from the \
         agent's: {last}"
    );
    assert_eq!(
        last["outcome"], "applied",
        "the log's last entry does not say the operation was applied: {last}"
    );
}

/// **A `null` clears a field, as the schema says, and does not leave it alone.**
///
/// The schema describes `note` and `bash` as `["string", "null"]`, with null meaning "no note" and
/// "every bound kit". The parameters were `Option<Option<String>>` with a plain `#[serde(default)]`,
/// and serde reads an explicit `null` there as **absent**. So an agent that sent `"note": null` was
/// told `applied`, and the note was still there: a refusal would have been better than that.
#[test]
fn a_null_clears_the_note_and_the_bash() {
    let root = project("agentnull");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    let apply = |app: &mut App, parameters: serde_json::Value| {
        let answer = call(
            app,
            "grimoire/operation_apply",
            serde_json::json!({"operation": "map_set_settings", "parameters": parameters, "dry_run": false}),
        )
        .unwrap_or_else(|e| panic!("the apply answers: {e}"));
        assert_eq!(answer["outcome"], "applied", "{answer}");
    };
    let held = |app: &App| {
        let map = &app.world().resource::<bevy_cauldron::project::OpenMap>().map;
        (map.note.clone(), map.bash.clone())
    };

    apply(&mut app, serde_json::json!({"note": "a courtyard", "bash": "site"}));
    assert_eq!(
        held(&app),
        (Some("a courtyard".to_owned()), Some("site".to_owned())),
        "the fields were not set, so this test is not testing the clear"
    );

    apply(&mut app, serde_json::json!({"note": null, "bash": null}));
    assert_eq!(
        held(&app),
        (None, None),
        "`null` was answered `applied` and the note and bash are still set -- serde read the null \
         as an absent field, so the agent was told the map changed when it did not"
    );
    let on_disk = std::fs::read_to_string(map_path(&app)).unwrap_or_default();
    assert!(
        !on_disk.contains("a courtyard"),
        "the cleared note is still in the file: {on_disk}"
    );

    // **And an absent field is still "leave it alone"** -- the other half of the contract.
    apply(&mut app, serde_json::json!({"note": "kept"}));
    apply(&mut app, serde_json::json!({"bounds": [64.0, 8.0, 64.0]}));
    assert_eq!(held(&app).0, Some("kept".to_owned()), "an absent `note` changed the note");
}

/// **A refusal changes nothing, and says why.**
#[test]
fn a_refusal_changes_nothing_and_says_why() {
    let root = project("agentrefuse");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    let map_path = map_path(&app);
    let before = std::fs::read_to_string(&map_path).unwrap_or_default();

    let refused = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"bounds": [0.0, 8.0, 64.0]},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the refusal answers: {e}"));
    assert_eq!(
        refused["outcome"], "refused",
        "a map with a zero bound was accepted: {refused}"
    );
    let why = refused["refusal"].as_str().expect("a refusal says why");
    assert!(
        why.contains("positive"),
        "the refusal does not say what was wrong with the value, so an agent cannot correct it: \
         {why}"
    );

    let after = std::fs::read_to_string(&map_path).unwrap_or_default();
    assert_eq!(
        before, after,
        "a refused operation wrote to the map's file. A refusal changes nothing."
    );
}

/// **An apply changes the RUNNING editor, not only the file.**
///
/// **The reviewer's gap, and it was a real one.** The bridge gave the operation a *clone* of the
/// map, so the operation edited the clone and wrote the file, and **the editor's own map was
/// untouched** — the screen showed the old settings until the watcher noticed the file, up to a
/// quarter of a second later, **and any edit the author made in that window was thrown away** when
/// the watcher swapped the map in.
///
/// **So the resource itself is read after the apply.** Not the file: the file was already checked,
/// and the file is the easy half.
#[test]
fn an_apply_changes_the_running_editor_and_not_only_the_file() {
    let root = project("agentrunning");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    let before = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>()
        .map
        .bounds;
    assert_ne!(
        before,
        (64.0, 8.0, 64.0),
        "the fixture already has the bounds the test sets, so the test would pass without the \
         apply doing anything"
    );

    let applied = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"bounds": [64.0, 8.0, 64.0]},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));
    assert_eq!(applied["outcome"], "applied", "{applied}");

    // **The editor's own map.**
    let after = app
        .world()
        .resource::<bevy_cauldron::project::OpenMap>()
        .map
        .bounds;
    assert_eq!(
        after,
        (64.0, 8.0, 64.0),
        "the file was written but the editor's own map was not changed, so the screen still shows \
         the old settings and an edit the author makes now would be thrown away."
    );
    // **And the operation's id, which the agent cites and the human reverts by.**
    assert!(
        applied["operation_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty()),
        "the apply did not return an operation id, so the agent cannot cite what it did: {applied}"
    );
}

/// **A misspelled field is refused, not ignored.**
///
/// **A field that is silently dropped is the worst outcome an agent can have**: it believes it
/// changed something it did not change, and it says so to the developer. So the parameters
/// deny unknown fields.
#[test]
fn a_misspelled_field_is_refused_rather_than_ignored() {
    let root = project("agenttypo");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    let refused = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"boundz": [64.0, 8.0, 64.0]},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the refusal answers: {e}"));
    assert_eq!(
        refused["outcome"], "refused",
        "a misspelled field was accepted, and the agent would believe it had changed the map's \
         bounds: {refused}"
    );
}

/// **The agent's path is fast, and the file's path is bounded.**
///
/// **Two numbers, because they are two paths.** The agent's path is a dispatch and a write; the
/// file's path is a poll and a re-read. **The target is about a second for a single-asset
/// change**, and the file's path is the one that has to meet it.
#[test]
fn the_two_paths_are_measured() {
    let root = project("agentlatency");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }

    // ── The agent's path: the clock starts at the call and stops when the file has changed. ──
    let map_path = map_path(&app);
    let before = std::fs::read_to_string(&map_path).unwrap_or_default();
    let started = std::time::Instant::now();
    call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "map_set_settings",
            "parameters": {"note": "a note the agent wrote"},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));
    let after = std::fs::read_to_string(&map_path).unwrap_or_default();
    assert_ne!(before, after, "the apply did not write");
    let agent_ms = started.elapsed().as_secs_f64() * 1000.0;

    // ── The file's path: the clock starts at the write and stops when the editor has it. ──
    //
    // **The file is written with a change the editor will notice** — a note the editor has not
    // seen — and the test then waits for the editor's own state to carry it.
    let started = std::time::Instant::now();
    let text = std::fs::read_to_string(&map_path).unwrap_or_default();
    let edited = text.replace("a note the agent wrote", "a note a person wrote");
    std::fs::write(&map_path, edited).unwrap_or_else(|e| panic!("the file is writable: {e}"));
    // **The wait is in wall-clock, not in frames.** A headless frame takes microseconds, so
    // forty of them pass in well under the watcher's 250 ms poll — and a frame-counted wait would
    // measure nothing and say "not seen". **So the test sleeps past one poll and then steps.**
    let mut file_ms = None;
    let deadline = started + std::time::Duration::from_millis(1500);
    while std::time::Instant::now() < deadline {
        app.update();
        if let Some(state) = app
            .world()
            .get_resource::<bevy_cauldron::project::OpenMap>()
            && state.map.note.as_deref() == Some("a note a person wrote")
        {
            file_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
            break;
        }
        // **A frame's worth of sleep**, so the loop is not a busy wait and the editor's delta
        // time is real.
        std::thread::sleep(std::time::Duration::from_millis(4));
    }

    println!("agent path: {agent_ms:.1} ms");
    let file_ms = file_ms.unwrap_or_else(|| {
        panic!(
            "the editor did not notice the file's change within {SETTLE_FRAMES} frames, so the \
             file path does not work at all — and a change on disk that the editor never sees is \
             the whole thing this module exists to fix."
        )
    });
    println!("file path:  {file_ms:.1} ms");

    // ── The target, asserted. ──
    //
    // **About a second for a single-asset change**, for both paths. The agent's path is a
    // dispatch and a write; the file's path is a poll and a re-read. **The file's path is the
    // one that has to meet the target**, and it meets it with room to spare because the poll is
    // a quarter of a second.
    assert!(
        agent_ms < 1000.0,
        "the agent's path took {agent_ms:.1} ms, and the target is about a second."
    );
    assert!(
        file_ms < 1000.0,
        "the file's path took {file_ms:.1} ms, and the target is about a second. The poll is          {POLL_MS} ms, so anything much above that is the re-read, not the wait."
    );
}

/// **The watcher's poll, in milliseconds — so the failure message can name it.**
const POLL_MS: u64 = 250;

/// **The map's file, wherever the editor put it.**
fn map_path(app: &App) -> std::path::PathBuf {
    app.world()
        .resource::<bevy_cauldron::project::OpenMap>()
        .map_path
        .clone()
}

/// **The editor's plugin graph has the watch in it.**
///
/// **A test that cannot fail is not a test**, and this one is the anti-vacuity floor for the
/// watcher: without the plugin there is no watcher, and every measurement above would be
/// measuring nothing.
#[test]
fn the_watcher_is_in_the_editor_s_plugin_graph() {
    let root = project("agentwatch");
    let app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **The resource is the proof.** The plugin is in the shared list, so a headless build has
    // it, and the watcher cannot be silently removed without this failing.
    assert!(
        app.world()
            .contains_resource::<bevy_cauldron::watch::Watched>(),
        "the file watcher is not in the editor's plugin graph, so a change on disk would not reach \
         the editor at all."
    );
}

/// **The keys plugin is in the graph, and the shortcut key is bound.**
///
/// **The floor for the bridge's own test.** `keys::binding` is what the badge test uses to hold
/// `K` down, and a missing binding would make the badge tests pass vacuously.
#[test]
fn the_shortcut_key_is_bound() {
    // **The chord is a real key.** `KeyCode::Unidentified` carries a native code, so it is not
    // comparable — and the honest check is that the binding names a key at all, which the
    // chord's own text says.
    let chord = keys::chord_text(keys::binding(keys::Action::Shortcuts));
    assert!(
        !chord.is_empty(),
        "the shortcut key is bound to nothing, so the badge overlay can never be raised."
    );
}

/// **An agent's turn stands on the screen, and not only in the file.**
///
/// **The regression test for the defect the `placement_set_yaw` run found.** An agent applied a
/// turn: the file said the piece was at the new yaw, the map said so, the log said so — and the
/// piece on screen kept its old rotation until the editor was reopened. The agent's write-back
/// replaced `OpenMap` directly, so the watcher saw nothing to adopt and `redraw_the_map` never
/// ran. The author looked at the picture and saw nothing had happened.
///
/// **The assertion is on the piece's rotation in the world**, not on the map: the map was never
/// the problem.
#[test]
fn an_agent_turn_stands_on_the_screen() {
    let root = project("agentturn");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    let before = piece_rotation(&mut app, "site/wall@0").expect("the piece is spawned");

    let applied = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "placement_set_yaw",
            "parameters": {"id": "site/wall@0", "yaw": 90.0},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));
    assert_eq!(
        applied["outcome"], "applied",
        "the turn did not apply: {applied}"
    );
    for _ in 0..8 {
        app.update();
    }

    let after = piece_rotation(&mut app, "site/wall@0").expect("the piece is still spawned");
    // **The angle of the change, not the quaternion**, because the piece's own mesh rotation is
    // part of `before` and the test should not depend on it.
    let (_, angle) = (after * before.inverse()).to_axis_angle();
    assert!(
        (angle.to_degrees() - 90.0).abs() < 0.5,
        "the file and the map say the piece turned 90 degrees, but the piece on SCREEN turned \
         {:.1} degrees. The agent's turn did not reach the picture, so the author sees nothing \
         happen.",
        angle.to_degrees()
    );
}

/// **A placed piece's rotation in the world, by its id.**
fn piece_rotation(app: &mut App, id: &str) -> Option<Quat> {
    let mut found = None;
    let mut q = app
        .world_mut()
        .query::<(&bevy_cauldron::editor::Placement, &Transform)>();
    for (placement, transform) in q.iter(app.world()) {
        if placement.0 == id {
            found = Some(transform.rotation);
        }
    }
    found
}
