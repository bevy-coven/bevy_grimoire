//! **The file editor in the Files workspace** (GD-11, #373; series the game as data, #362).
//!
//! What is measured, as a person reaches it:
//!
//! 1. **An unknown key typed into a project file shows a diagnostic on its line**, [`DIAGNOSE_IDLE_S`] after
//!    the last key and not before; fixed and saved (Cmd/Ctrl+S) the file on disk is the person's text and
//!    **the map in the editor shows the saved value**, as a save from an outside editor would (no operation).
//!    This is proved on the map's own file (`m.map.yaml`, the one the editor loads) and on a second map.
//! 2. **Completion on a reference field lists the library's pieces** (a placement's `descriptor` is the field the
//!    schema marks `x-dark-factory-ref: library`); the field names come from the schema too.
//! 3. **A change from outside while the tab has unsaved edits offers Reload or Keep mine, and overwrites
//!    nothing**: not the person's text, not the disk's.
//! 4. **`agent.yaml`, `play.yaml` and the guide open read-only**, so an edit cannot bypass a trust check.
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
use bevy_cauldron::files::{self, ConflictAnswer, Files, DIAGNOSE_IDLE_S, POLL_S};
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

const MAP: &str = "assets/grimoire/maps/m.map.yaml";
const MAP_YAML: &str = "assets/grimoire/maps/draft.map.yaml";

/// A map file with one field a line, so a diagnostic has a line of its own: the project's own map, as the YAML a project holds.
const MAP_TEXT: &str = "version: 3\nname: m\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 10.0\n- 3.0\n- 10.0\nplacements:\n- id: floor@0\n  descriptor: alpha/floor\n  at:\n  - 0.0\n  - 0.0\n  yaw: 0.0\nstamps: []\nlocations: []\n";

/// The line `yaw: 0.0` is on in [`MAP_TEXT`], and its text, so a test types after it.
const YAW_LINE: usize = 16;
const YAW_TEXT: &str = "  yaw: 0.0";

/// The same map under another name, for the tests that open a second one.
const MAP_YAML_TEXT: &str = "version: 3\nname: draft\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 10.0\n- 3.0\n- 10.0\nplacements:\n- id: floor@0\n  descriptor: alpha/floor\n  at:\n  - 0.0\n  - 0.0\n  yaw: 0.0\nstamps: []\nlocations: []\n";

fn project(tag: &str) -> PathBuf {
    let root = fixtures::Fixture::new(&format!("files_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .descriptor("alpha/wall", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join(MAP), MAP_TEXT).unwrap_or_else(|e| panic!("{e}"));
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
    let root = project("map");
    let mut app = files_editor(&root, MAP);
    assert_eq!(text(&mut app), MAP_TEXT, "the file's own text is what the editor shows");
    assert!(findings(&mut app).is_empty(), "a map that reads has nothing wrong with it: {:?}", findings(&mut app));

    // Type an unknown key on the line after `yaw`.
    type_a_key(&mut app, "bogus: 7");
    assert!(findings(&mut app).is_empty(), "no error is drawn on text the person is still typing");
    idle(&mut app);
    let found = findings(&mut app);
    assert_eq!(found.len(), 1, "one finding: {found:?}");
    assert_eq!(found[0].0, YAW_LINE + 1, "on the line the key was typed on: {found:?}");
    assert!(found[0].1.contains("bogus"), "and it names the key: {found:?}");
    assert!(app.world().resource::<Files>().is_dirty(MAP), "the tab says it has unsaved edits");
    assert_eq!(std::fs::read_to_string(root.join(MAP)).unwrap_or_default(), MAP_TEXT, "nothing is written until a save");

    // Fix it: take the key away and turn the yaw, then save.
    select(&mut app, CodePos::new(YAW_LINE, YAW_TEXT.len()), CodePos::new(YAW_LINE + 1, "  bogus: 7".len()));
    bevy_gooey::testing::press(&mut app, KeyCode::Backspace);
    select(&mut app, CodePos::new(YAW_LINE, YAW_TEXT.len() - 3), CodePos::new(YAW_LINE, YAW_TEXT.len()));
    bevy_gooey::testing::type_text(&mut app, "90.0");
    idle(&mut app);
    assert!(findings(&mut app).is_empty(), "the fixed file reads: {:?}", findings(&mut app));
    let want = MAP_TEXT.replace("yaw: 0.0", "yaw: 90.0");
    assert_eq!(text(&mut app), want, "the editor holds the fixed text");

    chord_ctrl(&mut app, KeyCode::KeyS, "s");
    assert_eq!(std::fs::read_to_string(root.join(MAP)).unwrap_or_default(), want, "Ctrl+S wrote the person's text, byte for byte");
    assert!(!app.world().resource::<Files>().is_dirty(MAP), "and the tab is clean");
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
    let mut app = files_editor(&root, MAP);
    type_a_key(&mut app, "lift: 0.5");
    let mine = text(&mut app);

    let outside = MAP_TEXT.replace("yaw: 0.0", "yaw: 45.0");
    bevy_grimoire_core::atomic_file::write(&root.join(MAP), &outside).unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_secs_f64(POLL_S + 0.1));
    settle(&mut app);

    let waiting = app.world().resource::<Files>().get(MAP).and_then(|f| f.external.clone());
    assert_eq!(waiting.as_deref(), Some(outside.as_str()), "the disk's text waits for an answer");
    assert_eq!(text(&mut app), mine, "the person's text is untouched");
    assert_eq!(std::fs::read_to_string(root.join(MAP)).unwrap_or_default(), outside, "and so is the file");
    let bar: Vec<ConflictAnswer> = {
        let world = app.world_mut();
        world.query::<&ConflictAnswer>().iter(world).copied().collect()
    };
    assert_eq!(bar.len(), 2, "two buttons: {bar:?}");
    assert!(bar.contains(&ConflictAnswer::Reload) && bar.contains(&ConflictAnswer::KeepMine));

    // **Saving while the question is open writes nothing.**
    chord_ctrl(&mut app, KeyCode::KeyS, "s");
    assert_eq!(std::fs::read_to_string(root.join(MAP)).unwrap_or_default(), outside, "a save does not overwrite a change the person has not seen");

    // Keep mine: the text stays, the question goes, and a save is now the person's own decision.
    click_answer(&mut app, ConflictAnswer::KeepMine);
    assert_eq!(text(&mut app), mine, "Keep mine keeps the person's text");
    assert!(app.world().resource::<Files>().get(MAP).is_some_and(|f| f.external.is_none()), "and the question is answered");
    chord_ctrl(&mut app, KeyCode::KeyS, "s");
    assert_eq!(std::fs::read_to_string(root.join(MAP)).unwrap_or_default(), mine, "then a save is theirs");
}

/// **Reload** takes the disk's text.
#[test]
fn reload_takes_the_disks_text_and_a_file_nobody_edited_reloads_by_itself() {
    let root = project("reload");
    let mut app = files_editor(&root, MAP);

    // Nobody has typed: an outside change is simply what the editor shows.
    let first = MAP_TEXT.replace("yaw: 0.0", "yaw: 10.0");
    bevy_grimoire_core::atomic_file::write(&root.join(MAP), &first).unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_secs_f64(POLL_S + 0.1));
    settle(&mut app);
    assert_eq!(text(&mut app), first, "a file with no unsaved edits reloads without asking");

    type_a_key(&mut app, "lift: 0.5");
    let second = MAP_TEXT.replace("yaw: 0.0", "yaw: 20.0");
    bevy_grimoire_core::atomic_file::write(&root.join(MAP), &second).unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_secs_f64(POLL_S + 0.1));
    settle(&mut app);
    click_answer(&mut app, ConflictAnswer::Reload);
    assert_eq!(text(&mut app), second, "Reload takes what the disk holds");
    assert!(!app.world().resource::<Files>().is_dirty(MAP), "and the tab is clean");
}

/// Type a new key on a line of its own after `yaw`, the way a person adds one.
fn type_a_key(app: &mut App, key: &str) {
    caret_at(app, YAW_LINE, YAW_TEXT.len());
    bevy_gooey::testing::press(app, KeyCode::Enter);
    bevy_gooey::testing::type_text(app, key);
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
    std::fs::write(root.join("agent.yaml"), "version: 1\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("play.yaml"), "version: 1\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(root.join(".dark_factory")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join(".dark_factory/AGENTS.md"), "# rules\n").unwrap_or_else(|e| panic!("{e}"));
    for rel in ["agent.yaml", "play.yaml", ".dark_factory/AGENTS.md", "AGENTS.md"] {
        assert!(files_lang::is_command_file(rel), "{rel} is read-only here");
    }
    assert!(!files_lang::is_command_file(MAP), "a map is not");
    assert!(!files_lang::is_command_file("scripts/agent.rhai"), "and a script merely named like one is not");

    let mut app = files_editor(&root, "agent.yaml");
    assert!(app.world().resource::<Files>().get("agent.yaml").is_some_and(|f| f.read_only), "agent.yaml opens read-only");
    let editor = editor_of(&mut app);
    assert!(app.world().get::<bevy::ui::InteractionDisabled>(editor).is_some(), "the widget does not take keys");
    bevy_gooey::testing::type_text(&mut app, "x");
    assert_eq!(text(&mut app), "version: 1\n", "typing changes nothing");
    let mut files = app.world_mut().resource_mut::<Files>();
    let refused = files::save_file(&mut files, &root, "agent.yaml").expect_err("a save of a command file refuses");
    assert!(refused.contains("Read-only"), "{refused}");
    assert_eq!(std::fs::read_to_string(root.join("agent.yaml")).unwrap_or_default(), "version: 1\n");
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
    assert!(listed.contains(&MAP.to_owned()), "the map's file is a file: {listed:?}");
    assert!(!listed.iter().any(|f| f.starts_with("target/")), "target/ is not listed: {listed:?}");
    assert!(!listed.contains(&"secret.log".to_owned()), "a glob in .gitignore hides a file: {listed:?}");
    assert!(!listed.iter().any(|f| f.starts_with("cache/")), "an anchored folder in .gitignore hides it: {listed:?}");

    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    run_command(&mut app, "workspace.files");
    settle(&mut app);
    // The tree is read on a thread of its own, started on a timer: step until its rows are there.
    let tree = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<files::FilesTree>>().single(world).unwrap_or_else(|e| panic!("the tree is on screen: {e}"))
    };
    let tree_has = |app: &App, key: u64| app.world().get::<bevy_gooey::Table>(tree).and_then(|t| t.source::<bevy_gooey::Tree>().map(|t| t.contains(key))).unwrap_or(false);
    for _ in 0..200 {
        if tree_has(&app, files::key_of("notes.md")) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        app.update();
    }
    for file in &listed {
        assert!(tree_has(&app, files::key_of(file)), "the tree has a row for {file}");
    }
    for hidden in ["target/debug/junk.txt", "secret.log", "cache/blob.bin", "target", "cache"] {
        assert!(!tree_has(&app, files::key_of(hidden)), "the tree has no row for {hidden}");
    }

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

    // RON (a game's own file: no project file is RON now), Rhai and Markdown.
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
/// RON or Rhai bracket that never closes by the scan.
#[test]
fn errors_are_the_loaders_own_with_the_line_they_stand_on() {
    let on = |path: &str, text: &str| files_lang::diagnose(path, text).into_iter().map(|f| (text[..f.range.start].matches('\n').count(), f.message)).collect::<Vec<_>>();

    let dup = on("scratch/a.yaml", "a: 1\nb: 2\na: 3\n");
    assert_eq!(dup.len(), 1, "{dup:?}");
    assert_eq!(dup[0].0, 2, "the second `a` is the fault: {dup:?}");
    assert!(on("scratch/a.yaml", "a: 1\nb: [1, 2]\n").is_empty(), "a file that reads has no findings");
    assert_eq!(on("scratch/a.yaml", "a: &x 1\nb: *x\n").len(), 1, "an anchor is refused where it stands");
    // Owner exception (2026-11-12): the RON error case the `ron` crate used to find (`b: ,` on line 2) is restored as YAML cases the
    // loader's own reader reports with a line: a tab that indents, and a second document.
    let tab = on("scratch/a.yaml", "a:\n\tb: 1\n");
    assert_eq!(tab.len(), 1, "{tab:?}");
    assert_eq!(tab[0].0, 1, "a tab is refused on the line it is on: {tab:?}");
    let second = on("scratch/a.yaml", "a: 1\n---\nb: 2\n");
    assert_eq!(second.len(), 1, "a second document is refused: {second:?}");

    // Owner exception (2026-11-12): no project file is RON and the `ron` crate is out of the editor (#365), so a RON file's
    // errors are what a bracket scan is sure of, as a Rhai file's are: a bracket that never closes, on the line it opened.
    let ron = on("scratch/a.ron", "(a: 1)\nb: (2,\n");
    assert_eq!(ron.len(), 1, "{ron:?}");
    assert_eq!(ron[0].0, 1, "{ron:?}");
    assert!(on("scratch/a.ron", "(a: 1)").is_empty());

    let rhai = on("scripts/a.rhai", "fn a() {\n  let x = (1 + 2;\n}\n");
    assert!(!rhai.is_empty(), "an unclosed bracket is found: {rhai:?}");
    assert!(on("scripts/a.rhai", "fn a() {\n  let x = (1 + 2);\n}\n").is_empty());

    // A project file of a known kind is read by its kind's reader: a map with a missing field.
    let missing = on(MAP, &MAP_TEXT.replace("  yaw: 0.0\n", ""));
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

/// **The decorations are for the revision they describe**: typing moves the text's revision and the colours move with it, so a
/// stale set is never drawn on new text.
#[test]
fn the_colours_are_for_the_text_that_is_showing() {
    let root = project("colour");
    let mut app = files_editor(&root, MAP);
    let editor = editor_of(&mut app);
    let revision = |app: &App| app.world().get::<CodeEditorValue>(editor).map(CodeEditorValue::revision).unwrap_or(0);
    let drawn = |app: &App| app.world().get::<Highlights>(editor).map(|h| (h.revision(), h.spans().len()));
    let first = drawn(&app).unwrap_or_else(|| panic!("the file is coloured"));
    assert_eq!(first.0, revision(&app), "the colours are for the text that is showing");
    assert!(first.1 > 0);

    // Typing moves the text's revision, and the colours move with it in the same frames: never a stale set on new text.
    let before = revision(&app);
    // Owner exception (2026-11-12): the typed text is a YAML key on its own line, not a RON field.
    type_a_key(&mut app, "lift: 0.5");
    assert!(revision(&app) > before, "typing changed the text");
    let after = drawn(&app).unwrap_or_else(|| panic!("still coloured"));
    assert_eq!(after.0, revision(&app), "the colours follow the text's revision");
    assert!(after.1 > first.1, "and cover the new words ({} then {})", first.1, after.1);
}
/// **GD-12 (#374), acceptance 1**: a mapping folded in a long YAML file hides its lines, and **the lines below move up
/// by the folded height**; unfolded, they come back. Measured on where the caret of a line below the fold stands in the text (its drawn place plus the scroll).
#[test]
fn a_folded_mapping_moves_the_lines_below_up_by_its_height_and_unfolding_brings_them_back() {
    let root = project("fold");
    let mut long = String::from("version: 3\nname: draft\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 10.0\n- 3.0\n- 10.0\nplacements:\n");
    for n in 0..12 {
        long.push_str(&format!("- id: floor@{n}\n  descriptor: alpha/floor\n  at:\n  - {n}.0\n  - 0.0\n  yaw: 0.0\n"));
    }
    long.push_str("stamps: []\nlocations: []\n");
    std::fs::write(root.join(MAP_YAML), &long).unwrap_or_else(|e| panic!("{e}"));
    let mut app = files_editor(&root, MAP_YAML);
    let editor = editor_of(&mut app);
    let header = long.lines().position(|l| l == "placements:").unwrap_or(0);
    let below = long.lines().position(|l| l == "stamps: []").unwrap_or(0);
    let folds = files_lang::folds(Lang::Yaml, &long);
    let range = folds.iter().find(|r| r.start == header).cloned().unwrap_or_else(|| panic!("`placements` folds: {folds:?}"));
    let hidden = range.end - range.start - 1;
    assert!(hidden >= 60, "the mapping is long: {hidden} lines hide");

    let y = |app: &mut App| {
        caret_at(app, below, 0);
        for _ in 0..60 {
            app.update();
        }
        app.world().get::<CodeEditor>(editor).map(|e| e.caret_at().y + e.scroll().y).unwrap_or(f32::NAN)
    };
    let open = y(&mut app);
    let line_px = {
        caret_at(&mut app, below - 1, 0);
        for _ in 0..60 {
            app.update();
        }
        open - app.world().get::<CodeEditor>(editor).map(|e| e.caret_at().y + e.scroll().y).unwrap_or(f32::NAN)
    };
    assert!(line_px > 1.0, "a line has a height: {line_px}");

    // Close the fold at the caret (Cmd/Ctrl+Shift+[), with the caret on the header.
    let fold_chord = |app: &mut App, code: KeyCode, name: &str| {
        key(app, KeyCode::ControlLeft, Key::Control, ButtonState::Pressed);
        key(app, KeyCode::ShiftLeft, Key::Shift, ButtonState::Pressed);
        key(app, code, Key::Character(name.into()), ButtonState::Pressed);
        key(app, code, Key::Character(name.into()), ButtonState::Released);
        key(app, KeyCode::ShiftLeft, Key::Shift, ButtonState::Released);
        key(app, KeyCode::ControlLeft, Key::Unidentified(NativeKey::Unidentified), ButtonState::Released);
        settle(app);
    };
    caret_at(&mut app, header, 0);
    fold_chord(&mut app, KeyCode::BracketLeft, "[");
    let folded = y(&mut app);
    let moved = open - folded;
    assert!(
        (moved - hidden as f32 * line_px).abs() < line_px,
        "the line below moved up by the {hidden} hidden lines ({} px), not {moved} px (a line is {line_px} px)",
        hidden as f32 * line_px
    );

    caret_at(&mut app, header, 0);
    fold_chord(&mut app, KeyCode::BracketRight, "]");
    let unfolded = y(&mut app);
    assert!((unfolded - open).abs() < 1.0, "unfolding gives the lines back: {open} then {unfolded}");
    assert_eq!(text(&mut app), long, "folding is never written into the file");
}

/// **GD-12 (#374)**: the editor draws a minimap, and the lines an agent changed are marked on it (a hint on each, through the one
/// channel gooey marks a line by).
#[test]
fn the_editor_has_a_minimap_and_a_change_of_the_familiars_is_marked_on_it() {
    let root = project("minimap");
    let mut app = files_editor(&root, MAP);
    let editor = editor_of(&mut app);
    assert!(app.world().get::<bevy_gooey::CodeMinimap>(editor).is_some(), "the editor carries a minimap");
    let marks = bevy_cauldron::files::agent_marks_for_test(MAP_TEXT, &[[11, 12]]);
    assert_eq!(marks.len(), 2, "one mark a changed line: {marks:?}");
    assert!(marks.iter().all(|m| m.message == bevy_cauldron::files::AGENT_CHANGED && m.severity == bevy_gooey::CodeSeverity::Hint));
}

/// **What the editor refuses to open, and what it does when a file goes away or grows** (review M1, M2, M9): a path out of
/// the project, a link out of it, a file that is not text, one that is too big, one that grows past the limit while open,
/// and one that is deleted (a save does not bring it back, and does not make its folder).
#[test]
fn a_path_out_of_the_project_a_non_text_file_a_huge_one_and_a_vanished_one_are_refused() {
    let root = project("refusals");
    std::fs::write(root.join("notes.md"), "# notes\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = files_editor(&root, "notes.md");
    let open = |app: &mut App, rel: &str| files::open_file(app.world_mut(), rel);

    assert!(open(&mut app, "../outside.txt").is_err_and(|e| e.contains("not a path inside the project")), "a `..` path is refused");
    #[cfg(unix)]
    {
        let outside = std::env::temp_dir().join(format!("files_out_{}.txt", std::process::id()));
        std::fs::write(&outside, "outside\n").unwrap_or_else(|e| panic!("{e}"));
        std::os::unix::fs::symlink(&outside, root.join("link.txt")).unwrap_or_else(|e| panic!("{e}"));
        assert!(open(&mut app, "link.txt").is_err_and(|e| e.contains("outside the project")), "a link out of the project is refused");
        let _ = std::fs::remove_file(&outside);
    }
    std::fs::write(root.join("bin.dat"), [0xff_u8, 0xfe, 0x00, 0x80]).unwrap_or_else(|e| panic!("{e}"));
    assert!(open(&mut app, "bin.dat").is_err_and(|e| e.contains("not text")), "a file that is not UTF-8 is refused");
    let big = vec![b'a'; files::MAX_FILE_BYTES as usize + 1];
    std::fs::write(root.join("big.txt"), &big).unwrap_or_else(|e| panic!("{e}"));
    assert!(open(&mut app, "big.txt").is_err_and(|e| e.contains("KiB")), "a file over the limit is refused");

    // A file that grows past the limit while it is open is not loaded by the poll: the tab says why.
    std::fs::write(root.join("grows.txt"), "small\n").unwrap_or_else(|e| panic!("{e}"));
    open(&mut app, "grows.txt").unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    std::fs::write(root.join("grows.txt"), &big).unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_secs_f64(POLL_S + 0.1));
    settle(&mut app);
    let file = app.world().resource::<Files>().get("grows.txt").cloned().unwrap_or_else(|| panic!("open"));
    assert!(file.problem.as_deref().is_some_and(|p| p.contains("KiB")), "the tab says it is too big: {:?}", file.problem);
    assert_eq!(file.text, "small\n", "and the 2 MB+ file was not read into the editor");

    // A file that is deleted: the tab says so, a save writes nothing, and the folder is not made again.
    std::fs::create_dir_all(root.join("scripts")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("scripts/a.rhai"), "let a = 1;\n").unwrap_or_else(|e| panic!("{e}"));
    open(&mut app, "scripts/a.rhai").unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app.world_mut().resource_mut::<Files>().get_mut("scripts/a.rhai").map(|f| f.text.push_str("let b = 2;\n")).unwrap_or_default();
    std::fs::remove_dir_all(root.join("scripts")).unwrap_or_else(|e| panic!("{e}"));
    std::thread::sleep(Duration::from_secs_f64(POLL_S + 0.1));
    settle(&mut app);
    assert!(app.world().resource::<Files>().get("scripts/a.rhai").is_some_and(|f| f.problem.as_deref().is_some_and(|p| p.contains("deleted"))), "the tab says the file was deleted");
    let mut files_res = app.world_mut().resource_mut::<Files>();
    let refused = files::save_file(&mut files_res, &root, "scripts/a.rhai").expect_err("a save of a deleted file refuses");
    assert!(refused.contains("deleted") || refused.contains("no longer exists"), "{refused}");
    assert!(!root.join("scripts").exists(), "the file and its folder were not made again");
}

/// **A tab with unsaved edits is not closed under the person**, and a clean file's save writes nothing.
#[test]
fn a_dirty_tab_keeps_its_place_and_a_clean_save_writes_nothing() {
    let root = project("dirtytab");
    let mut app = files_editor(&root, MAP);
    caret_at(&mut app, 0, 0);
    bevy_gooey::testing::type_text(&mut app, "x");
    assert!(app.world().resource::<Files>().is_dirty(MAP));
    bevy_cauldron::session::close_tab(app.world_mut(), Workspace::Files, &bevy_cauldron::session::Doc::new(bevy_cauldron::session::Kind::File, MAP));
    assert_eq!(files::active_file(app.world().resource::<Session>()).as_deref(), Some(MAP), "the tab is still there");

    std::fs::write(root.join("notes.md"), "# n\n").unwrap_or_else(|e| panic!("{e}"));
    files::open_file(app.world_mut(), "notes.md").unwrap_or_else(|e| panic!("{e}"));
    let stamp = std::fs::metadata(root.join("notes.md")).and_then(|m| m.modified()).ok();
    let mut files_res = app.world_mut().resource_mut::<Files>();
    files::save_file(&mut files_res, &root, "notes.md").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(std::fs::metadata(root.join("notes.md")).and_then(|m| m.modified()).ok(), stamp, "a clean file is not rewritten");
}

/// **Unsaved files are unsaved work** (review M8): leaving for the chooser asks even when the map is clean, and "save and
/// leave" saves the files while "discard" lets them go.
#[test]
fn a_file_with_unsaved_edits_makes_leaving_ask_and_save_all_and_discard_all_do_what_they_say() {
    let root = project("unsaved");
    let mut app = files_editor(&root, MAP);
    assert!(!files::any_unsaved(), "nothing is unsaved yet");
    caret_at(&mut app, 0, 0);
    bevy_gooey::testing::type_text(&mut app, "# mine\n");
    settle(&mut app);
    assert!(files::any_unsaved(), "a typed file is unsaved work");

    let mut state = bevy_cauldron::editor::EditorState::default();
    let mut confirm = bevy_cauldron::confirm::Confirm::default();
    bevy_cauldron::editor::leave_for_menu(false, &mut state, &mut confirm);
    assert!(confirm.is_open(), "leaving asks, though the map is clean");

    let typed = text(&mut app);
    {
        let mut res = app.world_mut().resource_mut::<Files>();
        res.discard_all(&root);
    }
    settle(&mut app);
    assert_ne!(text(&mut app), typed, "discard lets the typed text go");
    assert_eq!(std::fs::read_to_string(root.join(MAP)).unwrap_or_default(), MAP_TEXT);
    caret_at(&mut app, 0, 0);
    bevy_gooey::testing::type_text(&mut app, "# kept\n");
    let typed = text(&mut app);
    app.world_mut().resource_mut::<Files>().save_all(&root).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(std::fs::read_to_string(root.join(MAP)).unwrap_or_default(), typed, "save all writes the typed text");
    assert!(!files::any_unsaved());
}

/// **Only the project's own `.gitignore` is read** (a nested one does not hide anything): stated and tested.
#[test]
fn only_the_roots_gitignore_is_read() {
    let root = project("gitignore");
    std::fs::create_dir_all(root.join("sub")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("sub/.gitignore"), "*.txt\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("sub/a.txt"), "x").unwrap_or_else(|e| panic!("{e}"));
    assert!(files::listed_files(&root).contains(&"sub/a.txt".to_owned()), "a nested .gitignore hides nothing");
}

/// **A YAML map the loader refuses is an error in the editor** (review M6): the file reads as YAML, and the map's own
/// validation (its name must be snake_case) is what refuses it, for any map the project holds.
#[test]
fn a_yaml_map_the_loader_refuses_is_an_error_in_the_editor_and_so_is_the_projects_own_map() {
    let bad_yaml = MAP_YAML_TEXT.replace("name: draft", "name: BadName");
    let found = files_lang::diagnose(MAP_YAML, &bad_yaml);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].message.contains("snake_case"), "{found:?}");
    let bad_map = MAP_TEXT.replace("name: m", "name: BadName");
    let found = files_lang::diagnose(MAP, &bad_map);
    assert_eq!(found.len(), 1, "the project's own map is refused the same way: {found:?}");
    assert!(found[0].message.contains("snake_case"));
    assert!(files_lang::diagnose(MAP_YAML, MAP_YAML_TEXT).is_empty(), "and the good one reads");
}

/// **The scanner does not invent errors in valid code, and keeps its lines straight** (review M7).
#[test]
fn a_character_literal_and_a_backslash_newline_do_not_confuse_the_scanner() {
    let rhai = "fn a() {\n    let c = '\"';\n    let d = '(';\n    let e = '\\n';\n}\n";
    assert!(files_lang::diagnose("scripts/a.rhai", rhai).is_empty(), "{:?}", files_lang::diagnose("scripts/a.rhai", rhai));
    assert_eq!(files_lang::folds(Lang::Rhai, rhai), vec![0..4], "the literals add no bracket");
    // A string continued over a line break with a backslash: the lines after it are still counted.
    let continued = "let s = \"a\\\nb\";\nfn x() {\n  1;\n}\n";
    assert_eq!(files_lang::folds(Lang::Rhai, continued), vec![2..4], "line numbers do not drift");
}

/// **The colours of a YAML file are the loader's own nodes** (review M11a): every key the highlights draw is a key the loader's
/// walk reports, and a file the loader refuses is still coloured, from the line it stopped at.
#[test]
fn yaml_colours_are_the_loaders_nodes() {
    let (spans, fault) = bevy_grimoire_core::yaml::spans(MAP_YAML_TEXT);
    assert_eq!(fault, None);
    let keys: Vec<std::ops::Range<usize>> = spans.iter().filter(|s| s.kind == bevy_grimoire_core::yaml::SpanKind::Key).map(|s| s.range.clone()).collect();
    let drawn: Vec<std::ops::Range<usize>> = files_lang::highlights(Lang::Yaml, MAP_YAML_TEXT, None).into_iter().filter(|(_, r)| *r == SyntaxRole::Key).map(|(r, _)| r).collect();
    assert_eq!(keys, drawn, "the keys drawn are the keys the walk reads");
    let broken = "a: 1\na: 2\nb: 3\n";
    let drawn = files_lang::highlights(Lang::Yaml, broken, None);
    assert!(drawn.iter().any(|(r, role)| *role == SyntaxRole::Key && &broken[r.clone()] == "b"), "a file the loader refuses is coloured past its fault: {drawn:?}");
}

/// **A 10,000-line file stays inside a time budget** (review M5): colours, folds and the loader's reading of it, best of
/// three, on the test clock's own wall time.
#[test]
fn a_ten_thousand_line_map_is_coloured_folded_and_read_inside_a_budget() {
    let mut long = String::from("version: 3\nname: draft\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 100.0\n- 3.0\n- 100.0\nplacements:\n");
    for n in 0..1700 {
        long.push_str(&format!("- id: floor@{n}\n  descriptor: alpha/floor\n  at:\n  - {n}.0\n  - 0.0\n  yaw: 0.0\n"));
    }
    long.push_str("stamps: []\nlocations: []\n");
    assert!(long.lines().count() >= 10_000);
    let schema = bevy_grimoire_core::schema::for_kind("map").unwrap_or_else(|p| panic!("{}", p.line()));
    let best = |f: &dyn Fn()| (0..3).map(|_| { let t = std::time::Instant::now(); f(); t.elapsed() }).min().unwrap_or_default();
    let colour = best(&|| {
        let _ = files_lang::highlights(Lang::Yaml, &long, Some(&schema));
        let _ = files_lang::folds(Lang::Yaml, &long);
    });
    assert!(colour < Duration::from_millis(500), "colours and folds of 10k lines took {colour:?}");
    let read = best(&|| {
        let _ = files_lang::diagnose(MAP_YAML, &long);
    });
    assert!(read < Duration::from_millis(1500), "the loader's reading of 10k lines took {read:?}");
}
