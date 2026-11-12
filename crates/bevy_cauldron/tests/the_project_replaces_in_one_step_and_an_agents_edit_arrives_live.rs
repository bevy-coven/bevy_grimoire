//! **Find and replace across the project, and an agent's edits arriving live in the file editor** (GD-13, #375; GD-14, #376).
//!
//! GD-13: a replace across three files is **one operation** (`files_replace`) with **one undo** that leaves every file byte for byte
//! as it was; a replace that would break a YAML file is refused before anything is written, naming the file and the line; the search
//! runs as a background job that streams its results and can be cancelled; a result opens its file at the match.
//!
//! GD-14: `file_edit` changes an open file on disk with its comments intact, the open tab shows the change **without moving the
//! person's caret** (gooey's `apply_external_edit`), and `grimoire_undo_operation`'s own `ops::undo` restores the file byte for byte;
//! an edit that breaks the schema is refused before anything is written, and so is an edit to a file the person has unsaved edits in,
//! and to the command files.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::files::{self, Files, POLL_S};
use bevy_cauldron::files_panel::{self, SearchPanel, SearchResults, Target};
use bevy_cauldron::files_search::{self, Query};
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::ops::{self, Applied, Ctx, Source};
use bevy_cauldron::project::Project;
use bevy_cauldron::session::Session;
use bevy_gooey::{CodeEditor, CodeEditorValue, CodePos};

use crate::fixtures;

const SETTLE: usize = 12;
const YAML: &str = "assets/grimoire/maps/draft.map.yaml";
const YAML_TEXT: &str = "# the draft\nversion: 3\nname: draft\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 10.0\n- 3.0\n- 10.0\nplacements:\n- id: floor@0\n  descriptor: alpha/floor\n  at:\n  - 0.0\n  - 0.0\n  yaw: 0.0 # turn\nstamps: []\nlocations: []\n";

fn project(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("replace_{tag}_{}", std::process::id())).descriptor("alpha/floor", "alpha").place_as("floor@0", "alpha/floor", (0.0, 0.0)).build("m")
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn editor_app(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("workspace.files"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(&mut app);
    app
}

fn ctx(app: &App, source: Source) -> Ctx {
    Ctx { project: Some(app.world().resource::<Project>().clone()), source, session: "files_test".to_owned(), ..Ctx::default() }
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{e}"));
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

fn editor_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one code editor: {e}"))
}

fn wait_for_the_poll(app: &mut App) {
    std::thread::sleep(Duration::from_secs_f64(POLL_S + 0.1));
    settle(app);
}

/// **GD-13, acceptance 1**: one replace over three files, one undo, every file byte for byte as before.
#[test]
fn a_replace_across_three_files_is_one_operation_and_one_undo_puts_every_file_back() {
    let root = project("three");
    let (a, b, c) = ("notes/a.md", "scripts/b.rhai", "data/c.txt");
    let before = [(a, "The gate opens.\nA gate again.\n"), (b, "fn gate() {\n    log(\"gate\");\n}\n"), (c, "gate\r\nno change here\r\n")];
    for (rel, text) in before {
        write(&root, rel, text);
    }
    write(&root, "notes/untouched.md", "nothing to see\n");
    let mut app = editor_app(&root);
    let mut ctx = ctx(&app, Source::Human);
    let params = serde_json::json!({"query": "gate", "replacement": "portal", "whole_word": true});

    let Ok(Applied::Done { outcome, operation_id, .. }) = ops::dispatch(&mut ctx, "files_replace", &params) else { panic!("the replace applies") };
    assert_eq!(outcome.affected, 5, "five matches: {}", outcome.summary);
    assert_eq!(read(&root, a), "The portal opens.\nA portal again.\n");
    assert_eq!(read(&root, b), "fn portal() {\n    log(\"portal\");\n}\n");
    assert_eq!(read(&root, c), "portal\r\nno change here\r\n", "a CRLF file keeps its line breaks");
    assert_eq!(read(&root, "notes/untouched.md"), "nothing to see\n");
    let log = ops::read_entries(&ops::oplog_path_at(&root));
    assert_eq!(log.iter().filter(|e| e.op == "files_replace").count(), 1, "one log line for the whole replace: {log:?}");

    let (_, undone) = ops::undo(&mut ctx, Some(&operation_id)).unwrap_or_else(|e| panic!("one undo: {e}"));
    assert!(matches!(undone, Applied::Done { .. }));
    for (rel, text) in before {
        assert_eq!(read(&root, rel), text, "{rel} is byte for byte as before");
    }
    settle(&mut app);
}

/// **GD-13, acceptance 2**: a replace that would break one YAML file writes nothing, and names that file and line.
#[test]
fn a_replace_that_would_break_a_yaml_file_writes_nothing_and_names_the_file_and_line() {
    let root = project("breaks");
    write(&root, "data/ok.txt", "b c\n");
    write(&root, "data/pair.yaml", "a: 1\nb: 2\n");
    let app = editor_app(&root);
    let mut ctx = ctx(&app, Source::Agent);
    // `b` becomes `a`: the second key of the file is now a duplicate of the first.
    let params = serde_json::json!({"query": "b", "replacement": "a", "whole_word": true, "case_sensitive": true});
    let refused = ops::dispatch(&mut ctx, "files_replace", &params).expect_err("the replace is refused");
    assert!(refused.0.contains("data/pair.yaml"), "it names the file: {refused}");
    assert!(refused.0.contains("line 2"), "and the line: {refused}");
    assert_eq!(read(&root, "data/ok.txt"), "b c\n", "the file that would have read is not written either");
    assert_eq!(read(&root, "data/pair.yaml"), "a: 1\nb: 2\n");
    let log = ops::read_entries(&ops::oplog_path_at(&root));
    assert!(log.iter().all(|e| e.op != "files_replace" || e.outcome != "applied"), "nothing applied is logged");
}

/// **The command files are never written by a replace**, and the replace says it left them alone.
#[test]
fn a_replace_leaves_the_command_files_alone() {
    let root = project("command");
    write(&root, "agent.yaml", "version: 1\nnote: gate\n");
    write(&root, "notes.md", "gate\n");
    let app = editor_app(&root);
    let mut ctx = ctx(&app, Source::Human);
    let Ok(Applied::Done { outcome, .. }) = ops::dispatch(&mut ctx, "files_replace", &serde_json::json!({"query": "gate", "replacement": "portal"})) else { panic!("applies") };
    assert!(outcome.summary.contains("read-only"), "{}", outcome.summary);
    assert_eq!(read(&root, "agent.yaml"), "version: 1\nnote: gate\n");
    assert_eq!(read(&root, "notes.md"), "portal\n");
}

/// **The search streams its results and can be cancelled** (a background job), and a result opens its file at the match.
#[test]
fn the_search_streams_into_a_panel_can_be_cancelled_and_a_result_opens_its_file_at_the_match() {
    let root = project("panel");
    for n in 0..30 {
        write(&root, &format!("data/f{n:02}.txt"), &format!("line one\nneedle {n}\nline three\n"));
    }
    let mut app = editor_app(&root);
    // Ctrl+Shift+F opens the block.
    chord_ctrl_shift(&mut app, KeyCode::KeyF, "f");
    let panel = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<SearchPanel>>().single(world).unwrap_or_else(|e| panic!("the search block opens on Ctrl+Shift+F: {e}"))
    };
    app.world_mut().get_mut::<SearchPanel>(panel).map(|mut p| p.query.text = "needle".to_owned()).unwrap_or_default();
    files_panel::start_the_search(app.world_mut());
    let mut found = 0;
    for _ in 0..200 {
        std::thread::sleep(Duration::from_millis(20));
        app.update();
        found = app.world().get::<SearchPanel>(panel).map_or(0, |p| p.matches);
        if app.world().get::<SearchPanel>(panel).is_some_and(|p| p.done) {
            break;
        }
    }
    assert_eq!(found, 30, "every file's match arrived: {}", app.world().get::<SearchPanel>(panel).map(|p| p.status.clone()).unwrap_or_default());
    let rows = {
        let world = app.world_mut();
        let results = world.query_filtered::<Entity, With<SearchResults>>().single(world).unwrap_or_else(|e| panic!("{e}"));
        world.get::<bevy_gooey::Table>(results).and_then(|t| t.source::<bevy_gooey::Tree>().map(bevy_gooey::Tree::node_count)).unwrap_or(0)
    };
    assert!(rows >= 30 * 2, "a file row and a hit row each, with their context: {rows}");

    // Cancel stops it: a stopped search says so.
    app.world_mut().get_mut::<SearchPanel>(panel).map(|mut p| p.query.text = "line".to_owned()).unwrap_or_default();
    files_panel::start_the_search(app.world_mut());
    files_panel::stop_the_search(app.world_mut());
    settle(&mut app);
    assert!(app.world().get::<SearchPanel>(panel).is_some_and(|p| p.done && p.status.starts_with("Stopped")), "a cancelled search says it stopped");
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    let mut seen = 0;
    files_search::search(&root, &Query { text: "line".into(), ..Query::default() }, &cancelled, |_| seen += 1, |_, _| {}).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(seen, 0, "a search whose flag is set reads nothing");

    // A click on a result: the file opens at the match, with it selected.
    files_panel::open_at(app.world_mut(), &Target { rel: "data/f07.txt".into(), line: 1, col: 0, len: 6 });
    settle(&mut app);
    assert_eq!(files::active_file(app.world().resource::<Session>()).as_deref(), Some("data/f07.txt"));
    let editor = editor_of(&mut app);
    let buffer = app.world().get::<CodeEditor>(editor).map(|e| (e.buffer().selection(), e.buffer().selected_text())).unwrap_or_else(|| panic!("an editor"));
    assert_eq!(buffer.1, "needle", "the match is selected");
}

fn key(app: &mut App, code: KeyCode, logical: Key, state: ButtonState) {
    let window = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PrimaryWindow>>().single(world).unwrap_or(Entity::PLACEHOLDER)
    };
    app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical, state, text: None, repeat: false, window });
    app.update();
}

fn chord_ctrl_shift(app: &mut App, code: KeyCode, name: &str) {
    key(app, KeyCode::ControlLeft, Key::Control, ButtonState::Pressed);
    key(app, KeyCode::ShiftLeft, Key::Shift, ButtonState::Pressed);
    key(app, code, Key::Character(name.into()), ButtonState::Pressed);
    key(app, code, Key::Character(name.into()), ButtonState::Released);
    key(app, KeyCode::ShiftLeft, Key::Shift, ButtonState::Released);
    key(app, KeyCode::ControlLeft, Key::Control, ButtonState::Released);
    settle(app);
}

fn open(app: &mut App, rel: &str) -> Entity {
    files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    settle(app);
    let editor = editor_of(app);
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle(app);
    editor
}

/// **GD-14, acceptance 1**: `file_edit` on an open file changes it on disk with its comments intact; the tab shows the change and
/// the person's caret has not moved; one undo restores the file byte for byte.
#[test]
fn an_agents_file_edit_lands_in_the_open_tab_without_moving_the_caret_and_undoes_byte_for_byte() {
    let root = project("agent");
    write(&root, YAML, YAML_TEXT);
    let mut app = editor_app(&root);
    let editor = open(&mut app, YAML);
    app.world_mut().get_mut::<CodeEditor>(editor).map(|mut e| e.select(CodePos::new(3, 2), CodePos::new(3, 2))).unwrap_or_default();
    settle(&mut app);
    let caret = app.world().get::<CodeEditor>(editor).map(|e| e.buffer().caret()).unwrap_or_default();

    let mut ctx = ctx(&app, Source::Agent);
    let params = serde_json::json!({"path": YAML, "edits": [{"at": "placements[0].yaw", "value": 90.0}]});
    let Ok(Applied::Done { outcome, operation_id, shown, .. }) = ops::dispatch(&mut ctx, "file_edit", &params) else { panic!("the edit applies") };
    assert!(outcome.saved);
    assert_eq!(read(&root, YAML), YAML_TEXT.replace("yaw: 0.0 # turn", "yaw: 90.0 # turn"), "the file changed and its comments are intact");
    assert!(matches!(&shown, ops::Shown::File { path, lines: Some([l, _]) } if path == YAML && *l == 18), "it says where it is seen: {shown:?}");

    wait_for_the_poll(&mut app);
    let shown_text = app.world().get::<CodeEditorValue>(editor).map(|v| v.text().to_owned()).unwrap_or_default();
    assert!(shown_text.contains("yaw: 90.0 # turn"), "the open tab shows the change: {shown_text}");
    assert_eq!(app.world().get::<CodeEditor>(editor).map(|e| e.buffer().caret()), Some(caret), "the person's caret did not move");
    assert_eq!(app.world().resource::<Files>().get(YAML).map(|f| f.agent_lines.clone()), Some(vec![[18, 18]]), "the changed line is marked as the Familiar's");

    let (_, undone) = ops::undo(&mut ctx, Some(&operation_id)).unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(undone, Applied::Done { .. }));
    assert_eq!(read(&root, YAML), YAML_TEXT, "one undo restores the file byte for byte");
}

/// **GD-14, acceptance 2**: an edit that breaks the schema is refused before anything is written; so is an edit to a file the person
/// has unsaved edits in, and to the command files.
#[test]
fn an_edit_that_breaks_the_schema_or_meets_a_persons_unsaved_edits_or_a_command_file_is_refused() {
    let root = project("refused");
    write(&root, YAML, YAML_TEXT);
    write(&root, "agent.yaml", "version: 1\n");
    let mut app = editor_app(&root);
    let editor = open(&mut app, YAML);
    let mut ctx = ctx(&app, Source::Agent);

    let broken = serde_json::json!({"path": YAML, "edits": [{"at": "placements[0].yaw", "value": "north"}]});
    let refused = ops::dispatch(&mut ctx, "file_edit", &broken).expect_err("a string where a number stands is refused");
    assert!(refused.0.contains("Nothing was changed"), "{refused}");
    let unknown = serde_json::json!({"path": YAML, "edits": [{"at": {"lines": [2, 2]}, "value": "bogus: 1"}]});
    let refused = ops::dispatch(&mut ctx, "file_edit", &unknown).expect_err("a line edit that leaves the file unreadable is refused");
    assert!(refused.0.contains("unreadable") && refused.0.contains("bogus"), "{refused}");
    assert_eq!(read(&root, YAML), YAML_TEXT, "nothing was written");

    // The person types: an agent never merges into unsaved text.
    app.world_mut().get_mut::<CodeEditor>(editor).map(|mut e| e.select(CodePos::new(0, 0), CodePos::new(0, 0))).unwrap_or_default();
    bevy_gooey::testing::type_text(&mut app, "# mine\n");
    assert!(app.world().resource::<Files>().is_dirty(YAML));
    settle(&mut app);
    let good = serde_json::json!({"path": YAML, "edits": [{"at": "placements[0].yaw", "value": 45.0}]});
    let refused = ops::dispatch(&mut ctx, "file_edit", &good).expect_err("the person is editing");
    assert!(refused.0.contains("the person is editing") && refused.0.contains("wait or ask"), "{refused}");
    assert_eq!(read(&root, YAML), YAML_TEXT);

    let command = serde_json::json!({"path": "agent.yaml", "edits": [{"at": {"line": 1}, "value": "version: 2"}]});
    let refused = ops::dispatch(&mut ctx, "file_edit", &command).expect_err("a command file is refused");
    assert!(refused.0.contains("Read-only"), "{refused}");
    assert_eq!(read(&root, "agent.yaml"), "version: 1\n");
}

/// **GD-14**: the edit lands as an external edit, so the person's unsaved typing is untouched by an outside write to another place.
#[test]
fn diff_edit_is_the_one_span_that_turns_one_text_into_the_other() {
    let edit = files::diff_edit("a: 1\nb: 2\nc: 3\n", "a: 1\nb: 20\nc: 3\n").unwrap_or_else(|| panic!("they differ"));
    assert_eq!(edit.range, 9..9, "{edit:?}");
    assert_eq!(edit.text, "0");
    assert!(files::diff_edit("same", "same").is_none());
    let edit = files::diff_edit("héllo wörld", "héllo wörld!").unwrap_or_else(|| panic!("they differ"));
    assert_eq!((edit.range.start, edit.text.as_str()), ("héllo wörld".len(), "!"));
}

/// **A replace covers every file, not the tree's first 5,000 of a folder** (review): more files than the tree's per-folder cap are all
/// replaced, and one undo puts them all back.
#[test]
fn a_replace_over_more_files_than_the_trees_cap_covers_every_one() {
    let root = project("many");
    let count = files::TREE_DIR_LIMIT + 100;
    for n in 0..count {
        write(&root, &format!("many/f{n:05}.txt"), "needle\n");
    }
    let app = editor_app(&root);
    let mut ctx = ctx(&app, Source::Human);
    let Ok(Applied::Done { outcome, operation_id, .. }) = ops::dispatch(&mut ctx, "files_replace", &serde_json::json!({"query": "needle", "replacement": "found"})) else { panic!("applies") };
    assert_eq!(outcome.affected, count, "every file was replaced: {}", outcome.summary);
    assert!((0..count).all(|n| read(&root, &format!("many/f{n:05}.txt")) == "found\n"));
    ops::undo(&mut ctx, Some(&operation_id)).unwrap_or_else(|e| panic!("{e}"));
    assert!((0..count).all(|n| read(&root, &format!("many/f{n:05}.txt")) == "needle\n"), "one undo puts every file back");
}

/// **A path through a linked folder that leaves the project is refused, and so are the paths the tree never lists** (review): `.git`,
/// `target`, a `.gitignore`d file and the operation log are not an agent's to edit, and a replace does not follow a link out.
#[test]
fn file_edit_refuses_a_link_out_of_the_project_and_the_files_the_tree_never_lists() {
    let root = project("paths");
    write(&root, ".gitignore", "secret.log\n");
    write(&root, "secret.log", "a: 1\n");
    write(&root, ".git/hooks/pre-commit", "x: 1\n");
    write(&root, "target/x.yaml", "a: 1\n");
    let app = editor_app(&root);
    let mut ctx = ctx(&app, Source::Agent);
    let edit = |path: &str| serde_json::json!({"path": path, "edits": [{"at": "a", "value": 2}]});
    for path in [".git/hooks/pre-commit", "target/x.yaml", "secret.log", "assets/grimoire/oplog.jsonl"] {
        let refused = ops::dispatch(&mut ctx, "file_edit", &edit(path)).expect_err(path);
        assert!(refused.0.contains("not a file the project's tree lists") || refused.0.contains("ignored"), "{path}: {refused}");
    }
    assert_eq!(read(&root, "secret.log"), "a: 1\n");
    #[cfg(unix)]
    {
        let outside = std::env::temp_dir().join(format!("files_link_{}", std::process::id()));
        std::fs::create_dir_all(&outside).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(outside.join("x.yaml"), "a: 1\nmarker: 1\n").unwrap_or_else(|e| panic!("{e}"));
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap_or_else(|e| panic!("{e}"));
        let refused = ops::dispatch(&mut ctx, "file_edit", &edit("link/x.yaml")).expect_err("a path through a link out is refused");
        assert!(refused.0.contains("outside the project"), "{refused}");
        write(&root, "inside.txt", "marker\n");
        let mut human = ctx_for(&app);
        let Ok(Applied::Done { .. }) = ops::dispatch(&mut human, "files_replace", &serde_json::json!({"query": "marker", "replacement": "changed"})) else { panic!("applies") };
        assert_eq!(read(&root, "inside.txt"), "changed\n");
        assert_eq!(std::fs::read_to_string(outside.join("x.yaml")).unwrap_or_default(), "a: 1\nmarker: 1\n", "a replace does not follow the link out");
        let _ = std::fs::remove_dir_all(&outside);
    }
}

fn ctx_for(app: &App) -> Ctx {
    ctx(app, Source::Human)
}

/// **A preview belongs to one query** (review): changing the query, the paths or the replacement takes the preview away, so Replace all
/// never applies a replace the person did not preview.
#[test]
fn changing_the_query_takes_the_preview_away() {
    let root = project("preview");
    write(&root, "a.txt", "foo\n");
    let mut app = editor_app(&root);
    chord_ctrl_shift(&mut app, KeyCode::KeyF, "f");
    let panel = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<SearchPanel>>().single(world).unwrap_or_else(|e| panic!("{e}"))
    };
    let field = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<files_panel::QueryField>>().single(world).unwrap_or_else(|e| panic!("{e}"))
    };
    app.world_mut().get_mut::<SearchPanel>(panel).map(|mut p| p.preview = Some(files_search::Plan::default())).unwrap_or_default();
    app.world_mut().trigger(bevy::ui_widgets::ValueChange { source: field, value: "bar".to_owned(), is_final: false });
    settle(&mut app);
    assert!(app.world().get::<SearchPanel>(panel).is_some_and(|p| p.preview.is_none()), "the preview of the old query is gone");
}

/// **A failed write names what could not be put back** (review): the files before it are restored, and the message says so.
#[test]
fn a_failed_write_puts_the_earlier_files_back_and_says_so() {
    let root = project("apply");
    write(&root, "a.txt", "old a\n");
    write(&root, "b.txt", "old b\n");
    std::fs::create_dir_all(root.join("dir_in_the_way")).unwrap_or_else(|e| panic!("{e}"));
    let file = |rel: &str, before: &str, after: &str| files_search::PlannedFile { rel: rel.to_owned(), before: before.to_owned(), after: after.to_owned(), replacements: 1, changes: Vec::new() };
    let plan = files_search::Plan { files: vec![file("a.txt", "old a\n", "new a\n"), file("b.txt", "old b\n", "new b\n"), file("dir_in_the_way", "", "x")], skipped: Vec::new() };
    let refused = files_search::apply(&root, &plan).expect_err("a directory in the way refuses");
    assert!(refused.contains("dir_in_the_way") && refused.contains("put back") && !refused.contains("NOT"), "{refused}");
    assert_eq!(read(&root, "a.txt"), "old a\n");
    assert_eq!(read(&root, "b.txt"), "old b\n");
}
