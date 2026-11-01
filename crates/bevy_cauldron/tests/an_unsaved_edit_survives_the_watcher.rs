//! **An unsaved edit is not overwritten by the next file poll (B-049).**
//!
//! # What the bug is
//!
//! `watch::adopt_map` compared the file it re-read with `open.map` — **the in-memory map, which
//! holds the person's unsaved edits** — and replaced it whenever the two differed. So any save from
//! anywhere (an agent's operation, another window, a `cp`) followed by an unsaved human edit inside
//! the 250 ms poll replaced the human's edit with the file. Silently: the map on screen and the map
//! on disk then agree, so nothing looks wrong, and the work is simply gone.
//!
//! This is the real cause of the intermittent
//! `a_person_removes_a_piece_and_undoes_it::an_agents_undo_of_a_removal_waits_while_the_person_has_moved_a_piece_since`
//! failure, which was reported as flakiness.
//!
//! # What is being proved
//!
//! 1. **A save, then an unsaved edit, survives the poll.** The row the person moved is still where
//!    they put it, and the map is still dirty.
//! 2. **A file edited underneath unsaved work keeps the work and says so.** The person is told the
//!    file changed on disk, in the problem slot, because a silent choice between two versions of
//!    their work is the defect this test exists for.
//!
//! The map is moved in memory exactly as the unsaved `drop_piece` does it — writing `at.0` on the
//! row — so this is the real path and not a stand-in for it.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

/// **A project with three walls on a line, saved.**
///
/// Three, not one: the test moves the *far* row, so a fix that only kept the newest edit — or that
/// re-adopted a file whose prefix agreed — fails here rather than passing quietly.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .place("site/wall", (3.0, 0.0))
        .place("site/wall", (6.0, 0.0))
        .build("m")
}

/// **The editor, with the agent's door open.** Same plugin graph as
/// `an_agent_change_is_on_the_receipt`.
fn editor_with_bridge(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

/// **Call one of the editor's BRP commands through the registered method**, as the MCP server does.
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

/// **Move one row in memory, without saving** — exactly what the unsaved `drop_piece` does.
fn move_row_unsaved(app: &mut App, id: &str, x: f32) {
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    let row = open
        .map
        .placements
        .iter_mut()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("`{id}` is not on the map"));
    row.at.0 = x;
}

/// **The x of one row as the editor holds it.**
fn row_x(app: &App, id: &str) -> f32 {
    app.world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("`{id}` is not on the map"))
        .at
        .0
}

/// **Let the watcher poll, and prove it did.**
///
/// The poll is on a wall-clock 250 ms stamp and adopts only the file whose **stamp changed since the
/// previous tick**, so a single 300 ms sleep plus two updates is not enough: measured on the unfixed
/// code, the sequence `sleep 300 ms; update(); update()` reported the row unchanged — the bug was
/// present and the test said nothing. Instrumenting `tick_watch`/`adopt_changes` on the base commit
/// showed the real sequence: the tick that follows the save records the *new* stamp (so its own
/// `previous` already matches), and only a tick two polls later has a stale stamp to compare. Four
/// updates across two poll windows is what reaches `adopt_map`.
fn wait_for_a_poll(app: &mut App) {
    for _ in 0..2 {
        std::thread::sleep(std::time::Duration::from_millis(300));
        app.update();
        app.update();
    }
}

/// **The watcher really did look, and really did take a clean file.**
///
/// **This test exists because the other two passed on the unfixed code**, which means they could
/// pass without the watcher ever running — a vacuous test is worse than no test, because it reads as
/// coverage. This is the control: a clean editor and a file that says something new must be adopted.
/// If this goes red, the two tests below are proving nothing and must not be cited.
#[test]
fn a_clean_editor_takes_a_file_that_says_something_new() {
    let root = project("watchcontrol");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    assert!(
        !app.world().resource::<OpenMap>().dirty(),
        "the control starts from a clean editor, or it is not a control"
    );

    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut fresh: bevy_grimoire_core::map::Map =
        bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"));
    fresh.note = Some("edited outside".into());
    let written = ron::ser::to_string_pretty(&fresh, ron::ser::PrettyConfig::default())
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&path, &written).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    wait_for_a_poll(&mut app);

    assert_eq!(
        app.world().resource::<OpenMap>().map.note.as_deref(),
        Some("edited outside"),
        "a clean editor did not take a file that says something new, so the watcher never ran and \
         the two unsaved-edit tests below are vacuous. written file: {written}"
    );
}

/// **A save, then an unsaved edit, is not overwritten by the poll.**
#[test]
fn a_save_then_an_unsaved_edit_is_not_overwritten_by_the_poll() {
    let root = project("watchsave");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }

    // A save — this is an agent's operation, and it rewrites the file, which is what arms the trap:
    // the watcher's stamp changes, so the next poll reads the file it just wrote.
    let applied = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "placement_remove",
            "parameters": {"id": "site/wall@0"},
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"));
    assert_eq!(applied["outcome"], "applied", "the remove did not apply: {applied}");

    // The person then moves a piece and has not saved. The row is written the way `drop_piece`
    // writes it.
    move_row_unsaved(&mut app, "site/wall@2", 9.0);

    wait_for_a_poll(&mut app);

    assert_eq!(
        row_x(&app, "site/wall@2"),
        9.0,
        "the person's unsaved edit was replaced by the file 250 ms later. The map on screen and \
         the map on disk now agree, so nothing looks wrong — and the work is gone."
    );
    assert!(
        app.world().resource::<OpenMap>().dirty(),
        "the map holds the person's edit but does not report it as unsaved, so the next save \
         would leave it behind"
    );
}

/// **A file edited underneath unsaved work keeps the work and says so.**
#[test]
fn a_file_edit_over_unsaved_work_keeps_the_work_and_says_so() {
    let root = project("watchfile");
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }

    // The person moves a piece and has not saved.
    move_row_unsaved(&mut app, "site/wall@2", 9.0);

    // Something else edits the file — a second editor, an agent on the same project, `cp`.
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut fresh: bevy_grimoire_core::map::Map =
        bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"));
    fresh.note = Some("edited outside".into());
    let written = ron::ser::to_string_pretty(&fresh, ron::ser::PrettyConfig::default())
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&path, written).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

    wait_for_a_poll(&mut app);

    assert_eq!(
        row_x(&app, "site/wall@2"),
        9.0,
        "the file was adopted over the person's unsaved move, without asking and without saying"
    );
    assert_eq!(
        app.world().resource::<OpenMap>().map.note.as_deref(),
        None,
        "the file's note was adopted while the person had unsaved work, so the editor now holds a \
         mixture of two versions of the map"
    );
    let problem = app
        .world()
        .resource::<bevy_cauldron::editor::EditorState>()
        .status
        .problem_text()
        .to_owned();
    assert!(
        problem.contains("changed on disk while this map has unsaved edits"),
        "the editor kept the person's work but did not say the file had changed, so the person \
         cannot know their save will overwrite somebody else's. problem: {problem:?}"
    );
}
