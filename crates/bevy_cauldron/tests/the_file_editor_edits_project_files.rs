//! **The file editor in the Files workspace** (GD-11, #373; series the game as data, #362).
//!
//! What is measured, as a person reaches it:
//!
//! 1. **An unknown key typed into a project file shows a diagnostic on its line**, [`DIAGNOSE_IDLE_S`] after
//!    the last key and not before; fixed and saved (Cmd/Ctrl+S) the file on disk is the person's text and
//!    **the map in the editor shows the saved value**, as a save from an outside editor would (no operation).
//!    This is proved on the map's own file (`m.map.ron`, which the editor still loads) and, for the YAML
//!    spelling a converted project holds (GD-03), on `m.map.yaml`.
//! 2. **Completion on a reference field lists the library's pieces** (a placement's `descriptor` is the field the
//!    schema marks `x-dark-factory-ref: library`); the field names come from the schema too.
//! 3. **A change from outside while the tab has unsaved edits offers Reload or Keep mine, and overwrites
//!    nothing**: not the person's text, not the disk's.
//! 4. **`agent.ron`, `play.ron` and the guide open read-only**, so an edit cannot bypass a trust check.
//! 5. **The tree lists the project's files** except `target/` and what `.gitignore` names.
//! 6. **The languages**: colours by role, `x-dark-factory-ref` fields drawn as links, the loader's own errors,
//!    folds by structure.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::files::{self, ConflictAnswer, FileEditor, Files, DIAGNOSE_IDLE_S, POLL_S};
use bevy_cauldron::files_lang::{self, Lang};
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::session::Session;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::{
    CodeCompletionRequested, CodeCompletions, CodeEditor, CodeEditorValue, CodePos, CompletionTrigger, Diagnostics, Highlights, SyntaxRole,
};

use crate::fixtures;

const SETTLE: usize = 12;

const MAP_RON: &str = "assets/grimoire/maps/m.map.ron";
const MAP_YAML: &str = "assets/grimoire/maps/draft.map.yaml";

/// A map file with one field a line, so a diagnostic has a line of its own.
const MAP_TEXT: &str = "(\n    version: 3,\n    name: \"m\",\n    origin: (0.0, 0.0, 0.0),\n    bounds: (10.0, 3.0, 10.0),\n    bash: None,\n    placements: [\n        (\n            id: \"floor@0\",\n            descriptor: \"alpha/floor\",\n            at: (0.0, 0.0),\n            yaw: 0.0,\n            tip: (0, 0),\n            lift: 0.0,\n            on: None,\n            owned: false,\n            owned_because: None,\n            note: None,\n            patch: None,\n        ),\n    ],\n    stamps: [],\n    locations: [],\n)";

/// The same map as the YAML a converted project holds, with one placement.
const MAP_YAML_TEXT: &str = "version: 3\nname: draft\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 10.0\n- 3.0\n- 10.0\nplacements:\n- id: floor@0\n  descriptor: alpha/floor\n  at:\n  - 0.0\n  - 0.0\n  yaw: 0.0\nstamps: []\nlocations: []\n";

fn project(tag: &str) -> PathBuf {
    let root = fixtures::Fixture::new(&format!("files_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .descriptor("alpha/wall", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join(MAP_RON), MAP_TEXT).unwrap_or_else(|e| panic!("{e}"));
    root
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn run_command(app: &mut App, id: &str) {
    let command = commands::by_id(id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(app);
}

/// An editor on `root`, in the Files workspace, with `rel` open and the code editor focused.
fn files_editor(root: &Path, rel: &str) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // A window to send keys to: the headless harness has none (a key event with no window reaches no widget).
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    settle(&mut app);
    run_command(&mut app, "workspace.files");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "the Files workspace is reachable by its command");
    files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let editor = editor_of(&mut app);
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle(&mut app);
    app
}

fn editor_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one code editor is open: {e}"))
}

fn text(app: &mut App) -> String {
    let editor = editor_of(app);
    app.world().get::<CodeEditorValue>(editor).map(|v| v.text().to_owned()).unwrap_or_default()
}

/// Put the caret at a line and column of the open file.
fn caret_at(app: &mut App, line: usize, col: usize) {
    let editor = editor_of(app);
    let pos = CodePos::new(line, col);
    app.world_mut().get_mut::<CodeEditor>(editor).map(|mut e| e.select(pos, pos)).unwrap_or_else(|| panic!("no editor"));
    settle(app);
}

fn select(app: &mut App, from: CodePos, to: CodePos) {
    let editor = editor_of(app);
    app.world_mut().get_mut::<CodeEditor>(editor).map(|mut e| e.select(from, to)).unwrap_or_else(|| panic!("no editor"));
    settle(app);
}

fn key(app: &mut App, code: KeyCode, logical: Key, state: ButtonState) {
    let window = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PrimaryWindow>>().single(world).unwrap_or(Entity::PLACEHOLDER)
    };
    app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical, state, text: None, repeat: false, window });
    app.update();
}

fn chord_ctrl(app: &mut App, code: KeyCode, name: &str) {
    key(app, KeyCode::ControlLeft, Key::Control, ButtonState::Pressed);
    key(app, code, Key::Character(name.into()), ButtonState::Pressed);
    key(app, code, Key::Character(name.into()), ButtonState::Released);
    key(app, KeyCode::ControlLeft, Key::Unidentified(NativeKey::Unidentified), ButtonState::Released);
    settle(app);
}

/// Wait out the idle the errors wait for, in real time, and let the editor see it.
fn idle(app: &mut App) {
    std::thread::sleep(Duration::from_secs_f64(DIAGNOSE_IDLE_S + 0.05));
    settle(app);
}

fn findings(app: &mut App) -> Vec<(usize, String)> {
    let editor = editor_of(app);
    let lines = text(app);
    let Some(diagnostics) = app.world().get::<Diagnostics>(editor) else { return Vec::new() };
    let value = app.world().get::<CodeEditorValue>(editor).map(CodeEditorValue::revision).unwrap_or(0);
    if diagnostics.revision() != value {
        return Vec::new();
    }
    diagnostics
        .items()
        .iter()
        .map(|d| (lines[..d.range.start].matches('\n').count(), d.message.clone()))
        .collect()
}

/// **Acceptance 1**: an unknown key typed in shows a diagnostic at its line once the person pauses; the fixed
/// text, saved, is what is on disk and what the editor's map shows.
#[test]
fn an_unknown_key_is_a_diagnostic_on_its_line_after_the_idle_and_a_save_reloads_the_map() {
    let root = project("ron");
    let mut app = files_editor(&root, MAP_RON);
    assert_eq!(text(&mut app), MAP_TEXT, "the file's own text is what the editor shows");
    assert!(findings(&mut app).is_empty(), "a map that reads has nothing wrong with it: {:?}", findings(&mut app));

    // Type an unknown key at the start of the `yaw` line (line 11, the twelfth).
    caret_at(&mut app, 11, 12);
    bevy_gooey::testing::type_text(&mut app, "bogus: 7, ");
    assert!(findings(&mut app).is_empty(), "no error is drawn on text the person is still typing");
    idle(&mut app);
    let found = findings(&mut app);
    assert_eq!(found.len(), 1, "one finding: {found:?}");
    assert_eq!(found[0].0, 11, "on the line the key was typed on: {found:?}");
    assert!(found[0].1.contains("bogus"), "and it names the key: {found:?}");
    assert!(app.world().resource::<Files>().is_dirty(MAP_RON), "the tab says it has unsaved edits");
    assert_eq!(std::fs::read_to_string(root.join(MAP_RON)).unwrap_or_default(), MAP_TEXT, "nothing is written until a save");

    // Fix it: take the key away and turn the yaw, then save.
    select(&mut app, CodePos::new(11, 12), CodePos::new(11, 22));
    bevy_gooey::testing::press(&mut app, KeyCode::Backspace);
    select(&mut app, CodePos::new(11, 17), CodePos::new(11, 20));
    bevy_gooey::testing::type_text(&mut app, "90.0");
    idle(&mut app);
    assert!(findings(&mut app).is_empty(), "the fixed file reads: {:?}", findings(&mut app));
    let want = MAP_TEXT.replace("yaw: 0.0", "yaw: 90.0");
    assert_eq!(text(&mut app), want, "the editor holds the fixed text");

    chord_ctrl(&mut app, KeyCode::KeyS, "s");
    assert_eq!(std::fs::read_to_string(root.join(MAP_RON)).unwrap_or_default(), want, "Ctrl+S wrote the person's text, byte for byte");
    assert!(!app.world().resource::<Files>().is_dirty(MAP_RON), "and the tab is clean");
    let leftovers: Vec<_> = std::fs::read_dir(root.join("assets/grimoire/maps"))
        .unwrap_or_else(|e| panic!("{e}"))
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "the write was atomic: no temp file is left: {leftovers:?}");

    // **The map in the editor shows the saved value**, as after a save from an outside editor.
    let mut turned = false;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(60));
        app.update();
        turned = app.world().resource::<OpenMap>().map.placements.first().is_some_and(|p| (p.yaw - 90.0).abs() < 1e-3);
        if turned {
            break;
        }
    }
    assert!(turned, "the open map took the saved yaw through the same watcher an outside save reaches");
}

/// **Acceptance 1, for a YAML project file**: the loader's own reader names the unknown key and its line.
#[test]
fn an_unknown_key_in_a_yaml_project_file_is_named_by_the_loaders_reader() {
    let root = project("yaml");
    std::fs::write(root.join(MAP_YAML), MAP_YAML_TEXT).unwrap_or_else(|e| panic!("{e}"));
    let mut app = files_editor(&root, MAP_YAML);
    assert!(findings(&mut app).is_empty(), "a YAML map that reads has nothing wrong with it: {:?}", findings(&mut app));

    // After `yaw: 0.0` (line 16), a key the type does not have.
    let at = MAP_YAML_TEXT.lines().position(|l| l.trim() == "yaw: 0.0").unwrap_or(0);
    caret_at(&mut app, at, "  yaw: 0.0".len());
    bevy_gooey::testing::press(&mut app, KeyCode::Enter);
    bevy_gooey::testing::type_text(&mut app, "bogus: 1");
    idle(&mut app);
    let found = findings(&mut app);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, at + 1, "on the line of the unknown key: {found:?}");
    assert!(found[0].1.contains("bogus"), "naming it: {found:?}");
}

/// **Acceptance 2**: completion on a `descriptor:` value lists the library's pieces, and on a key position the
/// fields the type has, each with the doc comment.
#[test]
fn completion_on_a_reference_field_lists_the_librarys_pieces() {
    let root = project("complete");
    std::fs::write(root.join(MAP_YAML), MAP_YAML_TEXT).unwrap_or_else(|e| panic!("{e}"));
    let mut app = files_editor(&root, MAP_YAML);
    let editor = editor_of(&mut app);

    let ask = |app: &mut App, offset: usize| {
        // The caret stands where the word being completed is: the list closes when it is anywhere else.
        let source = text(app);
        let line = source[..offset].matches('\n').count();
        let col = offset - source[..offset].rfind('\n').map_or(0, |at| at + 1);
        caret_at(app, line, col);
        let revision = app.world().get::<CodeEditorValue>(editor).map(CodeEditorValue::revision).unwrap_or(0);
        app.world_mut().trigger(CodeCompletionRequested { entity: editor, offset, prefix: String::new(), trigger: CompletionTrigger::Manual, revision });
        app.update();
        app.world().get::<CodeCompletions>(editor).cloned().unwrap_or_default()
    };

    // The end of `  descriptor: alpha/floor`'s colon: the value position.
    let source = text(&mut app);
    let line_start = source.find("  descriptor: ").unwrap_or(0);
    let value_at = line_start + "  descriptor: ".len();
    let offered = ask(&mut app, value_at);
    let labels: Vec<&str> = offered.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"alpha/floor") && labels.contains(&"alpha/wall"), "the library's pieces are offered: {labels:?}");
    assert!(offered.items.iter().all(|i| i.detail == "library"), "each says what it names: {:?}", offered.items);

    // A key position, on a new line under a placement.
    let after_yaw = source.find("  yaw: 0.0").map(|at| at + "  yaw: 0.0".len()).unwrap_or(0);
    app.world_mut().get_mut::<CodeEditorValue>(editor).map(|mut v| v.set(format!("{}\n  {}", &source[..after_yaw], &source[after_yaw..]))).unwrap_or_default();
    settle(&mut app);
    let source = text(&mut app);
    let key_at = source.find("\n  \n").map(|at| at + 3).unwrap_or(0);
    let fields = ask(&mut app, key_at);
    let names: Vec<&str> = fields.items.iter().map(|i| i.label.as_str()).collect();
    assert!(names.contains(&"lift") && names.contains(&"owned"), "the fields a placement has: {names:?}");
    assert!(
        fields.items.iter().any(|i| i.label == "lift" && i.detail.contains("vertical offset")),
        "each with the first sentence of its doc comment: {:?}",
        fields.items.iter().find(|i| i.label == "lift")
    );
}

/// **Acceptance 3**: a change from outside while the tab has unsaved edits asks, and overwrites nothing.
#[test]
fn a_change_from_outside_under_unsaved_edits_asks_and_overwrites_nothing() {
    let root = project("conflict");
    let mut app = files_editor(&root, MAP_RON);
    caret_at(&mut app, 11, 12);
    bevy_gooey::testing::type_text(&mut app, "lift: 0.5, ");
    let mine = text(&mut app);

    let outside = MAP_TEXT.replace("yaw: 0.0", "yaw: 45.0");
    bevy_grimoire_core::atomic_file::write(&root.join(MAP_RON), &outside).unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_secs_f64(POLL_S + 0.1));
    settle(&mut app);

    let waiting = app.world().resource::<Files>().get(MAP_RON).and_then(|f| f.external.clone());
    assert_eq!(waiting.as_deref(), Some(outside.as_str()), "the disk's text waits for an answer");
    assert_eq!(text(&mut app), mine, "the person's text is untouched");
    assert_eq!(std::fs::read_to_string(root.join(MAP_RON)).unwrap_or_default(), outside, "and so is the file");
    let bar: Vec<ConflictAnswer> = {
        let world = app.world_mut();
        world.query::<&ConflictAnswer>().iter(world).copied().collect()
    };
    assert_eq!(bar.len(), 2, "two buttons: {bar:?}");
    assert!(bar.contains(&ConflictAnswer::Reload) && bar.contains(&ConflictAnswer::KeepMine));

    // **Saving while the question is open writes nothing.**
    chord_ctrl(&mut app, KeyCode::KeyS, "s");
    assert_eq!(std::fs::read_to_string(root.join(MAP_RON)).unwrap_or_default(), outside, "a save does not overwrite a change the person has not seen");

    // Keep mine: the text stays, the question goes, and a save is now the person's own decision.
    click_answer(&mut app, ConflictAnswer::KeepMine);
    assert_eq!(text(&mut app), mine, "Keep mine keeps the person's text");
    assert!(app.world().resource::<Files>().get(MAP_RON).is_some_and(|f| f.external.is_none()), "and the question is answered");
    chord_ctrl(&mut app, KeyCode::KeyS, "s");
    assert_eq!(std::fs::read_to_string(root.join(MAP_RON)).unwrap_or_default(), mine, "then a save is theirs");
}

/// **Reload** takes the disk's text.
#[test]
fn reload_takes_the_disks_text_and_a_file_nobody_edited_reloads_by_itself() {
    let root = project("reload");
    let mut app = files_editor(&root, MAP_RON);

    // Nobody has typed: an outside change is simply what the editor shows.
    let first = MAP_TEXT.replace("yaw: 0.0", "yaw: 10.0");
    bevy_grimoire_core::atomic_file::write(&root.join(MAP_RON), &first).unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_secs_f64(POLL_S + 0.1));
    settle(&mut app);
    assert_eq!(text(&mut app), first, "a file with no unsaved edits reloads without asking");

    caret_at(&mut app, 11, 12);
    bevy_gooey::testing::type_text(&mut app, "lift: 0.5, ");
    let second = MAP_TEXT.replace("yaw: 0.0", "yaw: 20.0");
    bevy_grimoire_core::atomic_file::write(&root.join(MAP_RON), &second).unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_secs_f64(POLL_S + 0.1));
    settle(&mut app);
    click_answer(&mut app, ConflictAnswer::Reload);
    assert_eq!(text(&mut app), second, "Reload takes what the disk holds");
    assert!(!app.world().resource::<Files>().is_dirty(MAP_RON), "and the tab is clean");
}

fn click_answer(app: &mut App, answer: ConflictAnswer) {
    let target = {
        let world = app.world_mut();
        world.query::<(Entity, &ConflictAnswer)>().iter(world).find(|(_, a)| **a == answer).map(|(e, _)| e).unwrap_or_else(|| panic!("no {answer:?} button"))
    };
    // The button's own event: the headless layout has no pointer to press it with.
    app.world_mut().trigger(bevy::ui_widgets::Activate { entity: target });
    settle(app);
}

/// **Acceptance 4**: the command files and the guide open read-only, and a save of one refuses.
#[test]
fn the_command_files_and_the_guide_open_read_only() {
    let root = project("readonly");
    std::fs::write(root.join("agent.ron"), "(version: 1)\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("play.ron"), "(version: 1)\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(root.join(".dark_factory")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join(".dark_factory/AGENTS.md"), "# rules\n").unwrap_or_else(|e| panic!("{e}"));
    for rel in ["agent.ron", "play.ron", ".dark_factory/AGENTS.md", "AGENTS.md"] {
        assert!(files_lang::is_command_file(rel), "{rel} is read-only here");
    }
    assert!(!files_lang::is_command_file(MAP_RON), "a map is not");
    assert!(!files_lang::is_command_file("scripts/agent.rhai"), "and a script merely named like one is not");

    let mut app = files_editor(&root, "agent.ron");
    assert!(app.world().resource::<Files>().get("agent.ron").is_some_and(|f| f.read_only), "agent.ron opens read-only");
    let editor = editor_of(&mut app);
    assert!(app.world().get::<bevy::ui::InteractionDisabled>(editor).is_some(), "the widget does not take keys");
    bevy_gooey::testing::type_text(&mut app, "x");
    assert_eq!(text(&mut app), "(version: 1)\n", "typing changes nothing");
    let mut files = app.world_mut().resource_mut::<Files>();
    let refused = files::save_file(&mut files, &root, "agent.ron").expect_err("a save of a command file refuses");
    assert!(refused.contains("Read-only"), "{refused}");
    assert_eq!(std::fs::read_to_string(root.join("agent.ron")).unwrap_or_default(), "(version: 1)\n");
}

/// **Acceptance 5**: the tree lists the project's files, without `target/` and what `.gitignore` names, and
/// a click on a file opens its tab.
#[test]
fn the_tree_lists_the_projects_files_and_a_click_opens_one() {
    let root = project("tree");
    std::fs::create_dir_all(root.join("target/debug")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("target/debug/junk.txt"), "x").unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(root.join("scripts")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("scripts/door.rhai"), "let open = true;\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("notes.md"), "# notes\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("secret.log"), "x").unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(root.join("cache")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("cache/blob.bin"), "x").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join(".gitignore"), "# build\n*.log\n/cache/\n").unwrap_or_else(|e| panic!("{e}"));

    let listed = files::listed_files(&root);
    assert!(listed.contains(&"notes.md".to_owned()) && listed.contains(&"scripts/door.rhai".to_owned()), "{listed:?}");
    assert!(listed.contains(&MAP_RON.to_owned()), "the map's file is a file: {listed:?}");
    assert!(!listed.iter().any(|f| f.starts_with("target/")), "target/ is not listed: {listed:?}");
    assert!(!listed.contains(&"secret.log".to_owned()), "a glob in .gitignore hides a file: {listed:?}");
    assert!(!listed.iter().any(|f| f.starts_with("cache/")), "an anchored folder in .gitignore hides it: {listed:?}");

    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    run_command(&mut app, "workspace.files");
    settle(&mut app);
    // The tree is spawned and read again on a timer; give it its first poll.
    std::thread::sleep(Duration::from_secs_f64(files::TREE_POLL_S + 0.1));
    settle(&mut app);
    let tree = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<files::FilesTree>>().single(world).unwrap_or_else(|e| panic!("the tree is on screen: {e}"))
    };
    let nodes = {
        let table = app.world().get::<bevy_gooey::Table>(tree).unwrap_or_else(|| panic!("a table"));
        table.source::<bevy_gooey::Tree>().map(bevy_gooey::Tree::node_count).unwrap_or(0)
    };
    assert!(nodes >= listed.len(), "the tree holds a row for every file ({nodes} rows for {} files)", listed.len());

    // Picking the row of a file opens its tab, active.
    app.world_mut().commands().entity(tree).insert(bevy_gooey::TableSelection::of(files::key_of("notes.md")));
    app.world_mut().commands().trigger(bevy_gooey::ValueChange { source: tree, value: bevy_gooey::TableSelection::of(files::key_of("notes.md")), is_final: true });
    settle(&mut app);
    assert_eq!(files::active_file(app.world().resource::<Session>()).as_deref(), Some("notes.md"), "the click opened the file");
    assert_eq!(text(&mut app), "# notes\n");
}

/// **The languages**: what each file kind is drawn as, read by the same code the editor uses.
#[test]
fn the_languages_are_coloured_by_role_and_references_are_links() {
    let spans = |lang, text: &str, schema: Option<&serde_json::Value>| files_lang::highlights(lang, text, schema);
    let role_of = |spans: &[(std::ops::Range<usize>, SyntaxRole)], text: &str, needle: &str| {
        let at = text.find(needle).unwrap_or_else(|| panic!("{needle} is in the text"));
        spans.iter().find(|(r, _)| r.start == at && r.end == at + needle.len()).map(|(_, role)| *role)
    };

    // YAML: a key, a number, a boolean, a string, a comment, a tag, an anchor.
    let yaml = "# the door\nname: gate\nopen: true\nspeed: 2.5\nkind: !Near\n  to: desk\n";
    let s = spans(Lang::Yaml, yaml, None);
    assert_eq!(role_of(&s, yaml, "# the door"), Some(SyntaxRole::Comment));
    assert_eq!(role_of(&s, yaml, "name"), Some(SyntaxRole::Key));
    assert_eq!(role_of(&s, yaml, "gate"), Some(SyntaxRole::String));
    assert_eq!(role_of(&s, yaml, "true"), Some(SyntaxRole::Keyword));
    assert_eq!(role_of(&s, yaml, "2.5"), Some(SyntaxRole::Number));
    assert_eq!(role_of(&s, yaml, "!Near"), Some(SyntaxRole::Type));

    // A reference field is a link: the schema's `x-dark-factory-ref` says which.
    let schema = bevy_grimoire_core::schema::for_kind("map").unwrap_or_else(|p| panic!("{}", p.line()));
    let s = spans(Lang::Yaml, MAP_YAML_TEXT, Some(&schema));
    assert_eq!(role_of(&s, MAP_YAML_TEXT, "alpha/floor"), Some(SyntaxRole::Reference), "a placement's descriptor names a library piece");
    assert_eq!(role_of(&s, MAP_YAML_TEXT, "floor@0"), Some(SyntaxRole::String), "and its id is only a string");

    // RON and Rhai and Markdown.
    let ron = "// a comment\n(version: 3, name: \"m\", bash: None, at: (1.0, -2))\n";
    let s = spans(Lang::Ron, ron, None);
    assert_eq!(role_of(&s, ron, "// a comment"), Some(SyntaxRole::Comment));
    assert_eq!(role_of(&s, ron, "version"), Some(SyntaxRole::Key));
    assert_eq!(role_of(&s, ron, "\"m\""), Some(SyntaxRole::String));
    assert_eq!(role_of(&s, ron, "None"), Some(SyntaxRole::Keyword));
    assert_eq!(role_of(&s, ron, "-2"), Some(SyntaxRole::Number));
    let rhai = "fn open(door) {\n    let n = 3; // count\n    log(\"hi\");\n}\n";
    let s = spans(Lang::Rhai, rhai, None);
    assert_eq!(role_of(&s, rhai, "fn"), Some(SyntaxRole::Keyword));
    assert_eq!(role_of(&s, rhai, "open"), Some(SyntaxRole::Function));
    assert_eq!(role_of(&s, rhai, "\"hi\""), Some(SyntaxRole::String));
    assert_eq!(role_of(&s, rhai, "// count"), Some(SyntaxRole::Comment));
    let md = "# Title\n\nSome `code` and [a link](x.md).\n\n```\nfenced\n```\n";
    let s = spans(Lang::Markdown, md, None);
    assert_eq!(role_of(&s, md, "# Title"), Some(SyntaxRole::Keyword));
    assert_eq!(role_of(&s, md, "`code`"), Some(SyntaxRole::String));
    assert_eq!(role_of(&s, md, "[a link](x.md)"), Some(SyntaxRole::Reference));
    assert_eq!(role_of(&s, md, "fenced"), Some(SyntaxRole::String));
}

/// **Errors are the loader's**: a duplicate key, a tab and a second document are named by the event walk; a
/// RON file that does not parse by `ron`; a Rhai bracket that never closes by the scan.
#[test]
fn errors_are_the_loaders_own_with_the_line_they_stand_on() {
    let on = |path: &str, text: &str| files_lang::diagnose(path, text).into_iter().map(|f| (text[..f.range.start].matches('\n').count(), f.message)).collect::<Vec<_>>();

    let dup = on("scratch/a.yaml", "a: 1\nb: 2\na: 3\n");
    assert_eq!(dup.len(), 1, "{dup:?}");
    assert_eq!(dup[0].0, 2, "the second `a` is the fault: {dup:?}");
    assert!(on("scratch/a.yaml", "a: 1\nb: [1, 2]\n").is_empty(), "a file that reads has no findings");
    assert_eq!(on("scratch/a.yaml", "a: &x 1\nb: *x\n").len(), 1, "an anchor is refused where it stands");

    let ron = on("scratch/a.ron", "(a: 1,\n b: ,\n)");
    assert_eq!(ron.len(), 1, "{ron:?}");
    assert_eq!(ron[0].0, 1, "{ron:?}");
    assert!(on("scratch/a.ron", "(a: 1)").is_empty());

    let rhai = on("scripts/a.rhai", "fn a() {\n  let x = (1 + 2;\n}\n");
    assert!(!rhai.is_empty(), "an unclosed bracket is found: {rhai:?}");
    assert!(on("scripts/a.rhai", "fn a() {\n  let x = (1 + 2);\n}\n").is_empty());

    // A project file of a known kind is read by its kind's reader: a RON map with a missing field.
    let missing = on(MAP_RON, &MAP_TEXT.replace("            yaw: 0.0,\n", ""));
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert!(missing[0].1.contains("yaw"), "the missing field is named: {missing:?}");
}

/// **Folds follow the structure**: every YAML mapping and sequence, Rhai blocks, Markdown sections.
#[test]
fn folds_follow_the_structure_of_each_language() {
    let yaml = "a:\n  b: 1\n  c:\n    d: 2\ne: 3\nlist:\n- x\n- y\n";
    let f = files_lang::folds(Lang::Yaml, yaml);
    assert!(f.contains(&(0..4)), "`a` folds over its mapping: {f:?}");
    assert!(f.contains(&(2..4)), "and `c` inside it: {f:?}");
    assert!(f.contains(&(5..8)), "a sequence at its key's own column folds under the key: {f:?}");
    assert!(!f.iter().any(|r| r.start == 4), "`e: 3` has nothing under it: {f:?}");

    let rhai = "fn a() {\n    if x {\n        y();\n    }\n}\n";
    let f = files_lang::folds(Lang::Rhai, rhai);
    assert!(f.contains(&(0..4)) && f.contains(&(1..3)), "blocks fold, the closing line stays: {f:?}");

    let md = "# A\ntext\n## B\nmore\n# C\nend\n";
    let f = files_lang::folds(Lang::Markdown, md);
    assert!(f.contains(&(0..4)), "a section runs to the next heading of its level or higher: {f:?}");
    assert!(f.contains(&(2..4)), "{f:?}");
    assert!(f.contains(&(4..6)), "{f:?}");
}

/// **The decorations are for the revision they describe**: the highlights on a file's editor carry the
/// text's own revision, so a stale one is never drawn on new text.
#[test]
fn the_colours_are_for_the_text_that_is_showing() {
    let root = project("colour");
    let mut app = files_editor(&root, MAP_RON);
    let editor = editor_of(&mut app);
    let revision = app.world().get::<CodeEditorValue>(editor).map(CodeEditorValue::revision).unwrap_or(0);
    let highlights = app.world().get::<Highlights>(editor).unwrap_or_else(|| panic!("the file is coloured"));
    assert_eq!(highlights.revision(), revision, "the colours are for the text that is showing");
    assert!(!highlights.spans().is_empty());
    let _ = app.world().get::<FileEditor>(editor).unwrap_or_else(|| panic!("the editor knows its file"));
}
