//! **A reference opens what it names** (NV-07, #384; series getting around, #377).
//!
//! `reading_a_reference_opens_what_it_names.rs` pins the query a `Used by` asks. This file proves the rest of the issue's acceptance, each
//! item by what a person sees:
//!
//! 1. **Cmd/Ctrl+click on a `cue:` value in a YAML file lands in Sound with that cue selected**; F12 does the same from the keyboard.
//! 2. **Open file on a placement lands in Files at that placement's line**, and Open piece file at the piece's entry in the library.
//! 3. **A cue's Used by lists the files that name it**, from the finder's own index.
//! 4. **A reference to something that does not exist is never a dead link**: it is a diagnostic on its line in the file editor, and the
//!    follow says what is missing instead of doing nothing.
//! 5. **A piece's Used by in the Kit lists the maps that place it.**

use std::path::{Path, PathBuf};

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::editor::{self, EditorState};
use bevy_cauldron::files;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::ops::Shown;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::reveal::Revealing;
use bevy_cauldron::session::Session;
use bevy_cauldron::workspace::Workspace;
use bevy_cauldron::world_lists::SelectedCue;
use bevy_gooey::{CodeEditor, CodePos, Diagnostics};

use crate::fixtures;

/// Frames the editor is given to settle after each step.
const SETTLE: usize = 40;

/// The project's sound file, in the crate's own shape, with the two cues the Sound list shows.
const CUES: &str = "(cues: (cues: [(name: \"door.creak\"), (name: \"wind.loop\")]))";

/// A game data file that names a cue, on one line with no line break at the end: a click in the empty part of the editor puts the caret
/// at the end of that line, which is the end of the value.
const DOOR: &str = "cue: door.creak";

/// The same, naming a cue the project does not hold.
const GHOST: &str = "cue: ghost.sound";

fn project(tag: &str, extra: &[(&str, &str)]) -> PathBuf {
    let root = fixtures::Fixture::new(&format!("xref_{tag}_{}", std::process::id()))
        .sized_descriptor("bed", "alpha", 2.0, 1.0)
        .place_as("bed@0", "bed", (2.0, 2.0))
        .side(64.0)
        .build("m");
    for (rel, text) in extra {
        let path = root.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        }
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    }
    root
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn editor(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // A window to send keys to: the headless harness has none.
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    settle(&mut app);
    app
}

fn run_command(app: &mut App, id: &str) {
    let command = commands::by_id(id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(app);
}

/// **The Files workspace with `rel` open and its editor focused**, the way a person opens a file from the tree.
fn files_editor(root: &Path, rel: &str) -> App {
    let mut app = editor(root);
    run_command(&mut app, "workspace.files");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "the Files workspace is reachable by its command");
    files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let editor = editor_entity(&mut app);
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle(&mut app);
    app
}

fn editor_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one code editor is open: {e}"))
}

fn caret_at(app: &mut App, line: usize, col: usize) {
    let editor = editor_entity(app);
    let pos = CodePos::new(line, col);
    app.world_mut().get_mut::<CodeEditor>(editor).map(|mut e| e.select(pos, pos)).unwrap_or_else(|| panic!("no editor"));
    settle(app);
}

fn caret_line(app: &mut App) -> usize {
    let editor = editor_entity(app);
    app.world().get::<CodeEditor>(editor).map(|e| e.buffer().caret().line).unwrap_or_else(|| panic!("no editor"))
}

fn key(app: &mut App, code: KeyCode, logical: Key, state: ButtonState) {
    let window = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PrimaryWindow>>().single(world).unwrap_or(Entity::PLACEHOLDER)
    };
    app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical, state, text: None, repeat: false, window });
    app.update();
}

fn press(app: &mut App, code: KeyCode, logical: Key) {
    key(app, code, logical.clone(), ButtonState::Pressed);
    key(app, code, logical, ButtonState::Released);
    settle(app);
}

/// Whether a person can read exactly these words somewhere on screen.
fn drawn(app: &mut App, words: &str) -> bool {
    let world = app.world_mut();
    world.query::<(&Text, &InheritedVisibility)>().iter(world).any(|(text, shown)| shown.get() && text.0.trim() == words)
}

fn on_screen(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world.query::<(&Text, &InheritedVisibility)>().iter(world).filter(|(_, shown)| shown.get()).map(|(t, _)| t.0.clone()).collect()
}

fn selected_cue(app: &App) -> Option<String> {
    app.world().resource::<SelectedCue>().0.clone()
}

/// The column (0-based) inside `cue: door.creak` that the value occupies: the `d` is at 5, so 8 is inside the name and 1 is on `c`.
const IN_THE_VALUE: usize = 8;
const OFF_THE_VALUE: usize = 1;

/// **Acceptance 1a — Cmd/Ctrl+click.** With the primary modifier held, a press puts the caret where the pointer went down and the release
/// follows the reference under it to the Sound workspace with the cue selected.
///
/// **The caret is placed here, as the F12 test places it**: `bevy_gooey::testing::click` sends the press and the release at the editor's
/// centre without moving a caret, so a test that relied on the click to place it would be measuring the harness. What the product does
/// is read the reference under the caret **at the release**, which is asserted here and, for the stale case, just below.
#[test]
fn a_primary_click_on_a_cue_value_lands_in_sound_with_that_cue_selected() {
    let root = project("click", &[("family/bevy_siren.ron", CUES), ("data/door.yaml", DOOR)]);
    let mut app = files_editor(&root, "data/door.yaml");
    assert_eq!(selected_cue(&app), None, "no cue is chosen before the click");
    caret_at(&mut app, 0, IN_THE_VALUE);

    let primary = if cfg!(target_os = "macos") { (KeyCode::SuperLeft, Key::Super) } else { (KeyCode::ControlLeft, Key::Control) };
    key(&mut app, primary.0, primary.1, ButtonState::Pressed);
    let editor = editor_entity(&mut app);
    bevy_gooey::testing::click(&mut app, editor);
    settle(&mut app);

    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Sound, "the click followed the reference to the Sound workspace");
    assert_eq!(selected_cue(&app).as_deref(), Some("door.creak"), "and it is the cue the value names that is selected");
    assert!(drawn(&mut app, "▸ door.creak"), "the Sound list marks the selected cue; on screen: {:?}", on_screen(&mut app));
    assert!(drawn(&mut app, "wind.loop"), "the other cue is listed and not marked");
}

/// **The click reads the caret it finds, not one it remembers.** A caret that was on the reference and has since moved off it follows
/// nothing: the path asks the caret at the release, so a remembered one would be the dead link this series forbids.
#[test]
fn a_primary_click_off_the_reference_does_not_follow_a_caret_that_has_moved() {
    let root = project("click_stale", &[("family/bevy_siren.ron", CUES), ("data/door.yaml", DOOR)]);
    let mut app = files_editor(&root, "data/door.yaml");
    // **On the value, then off it**: the second place is the one the click must read.
    caret_at(&mut app, 0, IN_THE_VALUE);
    caret_at(&mut app, 0, OFF_THE_VALUE);

    let primary = if cfg!(target_os = "macos") { (KeyCode::SuperLeft, Key::Super) } else { (KeyCode::ControlLeft, Key::Control) };
    key(&mut app, primary.0, primary.1, ButtonState::Pressed);
    let editor = editor_entity(&mut app);
    bevy_gooey::testing::click(&mut app, editor);
    settle(&mut app);

    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "a caret off the value follows nothing; it stays in the editor");
    assert_eq!(selected_cue(&app), None, "and no cue is marked");
}

/// **Cmd/Ctrl+clicking the file tree does not follow the caret's reference** (review M2): the tree, the header and the notice live
/// in the same workspace, so a click outside the editor's own bounds is a click on the workspace, not on a value.
#[test]
fn a_primary_click_on_the_tree_stays_in_files() {
    let root = project("click_tree", &[("family/bevy_siren.ron", CUES), ("data/door.yaml", DOOR)]);
    let mut app = files_editor(&root, "data/door.yaml");
    caret_at(&mut app, 0, IN_THE_VALUE);
    assert_eq!(selected_cue(&app), None, "nothing is followed before the click");

    // **The pointer is put over the tree**, which the harness's own click does not do: it clicks an entity's node directly.
    let tree = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<files::FilesTree>>().single(world).unwrap_or_else(|e| panic!("the tree is on screen: {e}"))
    };
    let tree_place = {
        let world = app.world();
        world.get::<UiGlobalTransform>(tree).unwrap_or_else(|| panic!("the tree is laid out")).translation
    };
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window");
    let scale = app.world().get::<Window>(window).map_or(1.0, Window::scale_factor);
    app.world_mut().write_message(bevy::window::CursorMoved { window, position: tree_place / scale, delta: Some(Vec2::ZERO) });

    let primary = if cfg!(target_os = "macos") { (KeyCode::SuperLeft, Key::Super) } else { (KeyCode::ControlLeft, Key::Control) };
    key(&mut app, primary.0, primary.1, ButtonState::Pressed);
    bevy_gooey::testing::click(&mut app, tree);
    settle(&mut app);

    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "a Cmd+click on the tree is not a click on a reference");
    assert_eq!(selected_cue(&app), None, "and no cue is marked");
}

/// **F12 while the finder is open does nothing** (review M3): the finder's own box holds the keyboard, and every other chord
/// respects that. Without the gate F12 would follow the reference behind the overlay the person is typing into.
#[test]
fn f12_with_the_finder_open_does_nothing() {
    let root = project("f12_finder", &[("family/bevy_siren.ron", CUES), ("data/door.yaml", DOOR)]);
    let mut app = files_editor(&root, "data/door.yaml");
    caret_at(&mut app, 0, IN_THE_VALUE);
    // Cmd/Ctrl+P opens the finder, whose search field takes the keyboard.
    let finder_key = if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft };
    let logical = if cfg!(target_os = "macos") { Key::Super } else { Key::Control };
    key(&mut app, finder_key, logical, ButtonState::Pressed);
    press(&mut app, KeyCode::KeyP, Key::Character("p".into()));
    settle(&mut app);

    press(&mut app, KeyCode::F12, Key::F12);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "the editor stays where it is");
    assert_eq!(selected_cue(&app), None, "and F12 followed nothing behind the finder");
}

/// **A click without the modifier only edits**: the caret goes where it was clicked and the editor stays where it is.
#[test]
fn a_plain_click_does_not_follow_the_reference() {
    let root = project("plain", &[("family/bevy_siren.ron", CUES), ("data/door.yaml", DOOR)]);
    let mut app = files_editor(&root, "data/door.yaml");
    let editor = editor_entity(&mut app);
    bevy_gooey::testing::click(&mut app, editor);
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "a plain click is an edit's click");
    assert_eq!(selected_cue(&app), None);
}

/// **Acceptance 1b — F12.** The same follow from the keyboard, with the caret inside the value.
#[test]
fn f12_on_a_cue_value_lands_in_sound_with_that_cue_selected() {
    let root = project("f12", &[("family/bevy_siren.ron", CUES), ("data/door.yaml", DOOR)]);
    let mut app = files_editor(&root, "data/door.yaml");
    caret_at(&mut app, 0, 8);
    press(&mut app, KeyCode::F12, Key::F12);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Sound, "F12 followed the reference");
    assert_eq!(selected_cue(&app).as_deref(), Some("door.creak"));
}

/// **F12 off a reference says so** rather than doing nothing.
#[test]
fn f12_where_nothing_is_named_says_so() {
    let root = project("f12_none", &[("family/bevy_siren.ron", CUES), ("data/door.yaml", DOOR)]);
    let mut app = files_editor(&root, "data/door.yaml");
    caret_at(&mut app, 0, 1);
    press(&mut app, KeyCode::F12, Key::F12);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "F12 off a reference stays in the editor");
    let note = app.world().resource::<EditorState>().status.note_text().to_owned();
    assert!(note.contains("no reference"), "the status line said: {note:?}");
}

/// **Acceptance 4 — never a dead link.** A cue the project does not hold is a warning on its own line, and following it says what is missing.
#[test]
fn a_reference_to_a_missing_cue_is_a_diagnostic_and_a_refusal_not_a_dead_link() {
    let root = project("ghost", &[("family/bevy_siren.ron", CUES), ("data/ghost.yaml", GHOST)]);
    let mut app = files_editor(&root, "data/ghost.yaml");
    // The errors wait for the person to stop typing; the clock is real, so wait it out.
    std::thread::sleep(std::time::Duration::from_millis(250));
    settle(&mut app);
    let editor = editor_entity(&mut app);
    let diagnostics = app.world().get::<Diagnostics>(editor).map(|d| d.items().iter().map(|i| i.message.clone()).collect::<Vec<_>>()).unwrap_or_default();
    assert!(
        diagnostics.iter().any(|m| m.contains("ghost.sound") && m.contains("not a cue")),
        "the editor marks the missing cue on its line: {diagnostics:?}"
    );

    caret_at(&mut app, 0, 8);
    press(&mut app, KeyCode::F12, Key::F12);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "a reference to nothing goes nowhere");
    assert_eq!(selected_cue(&app), None);
    let said = app.world().resource::<EditorState>().status.problem_text().to_owned();
    assert!(said.contains("ghost.sound") && said.contains("sound cue"), "the problem names what is missing: {said:?}");
}

/// **Acceptance 3 — a cue's Used by**: the files that name it, with their lines, from the finder's index.
#[test]
fn a_cues_used_by_lists_the_files_that_name_it() {
    let root = project(
        "used_by",
        &[("family/bevy_siren.ron", CUES), ("data/door.yaml", "name: door\ncue: door.creak\n"), ("data/wind.yaml", "cue: wind.loop\n")],
    );
    let mut app = editor(&root);
    app.world_mut().resource_mut::<Revealing>().request_by_person("test", &Shown::Cue { name: "door.creak".to_owned() });
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Sound, "the reveal of a cue opens the Sound workspace");
    assert_eq!(selected_cue(&app).as_deref(), Some("door.creak"));
    assert!(drawn(&mut app, "used by data/door.yaml:2"), "the file and line that name the cue; on screen: {:?}", on_screen(&mut app));
    assert!(!drawn(&mut app, "used by data/wind.yaml:1"), "a file that names another cue is not listed");
}

/// **A cue nothing names says so**, rather than drawing an empty list.
#[test]
fn a_cue_nothing_names_says_so() {
    let root = project("unused", &[("family/bevy_siren.ron", CUES)]);
    let mut app = editor(&root);
    app.world_mut().resource_mut::<Revealing>().request_by_person("test", &Shown::Cue { name: "wind.loop".to_owned() });
    settle(&mut app);
    assert!(drawn(&mut app, "no file names `wind.loop`"), "on screen: {:?}", on_screen(&mut app));
}

/// **Acceptance 2 — Open file on a placement** lands in the Files workspace at the placement's line, and **Open piece file** at the piece's entry.
#[test]
fn open_file_on_a_placement_lands_in_files_at_its_line() {
    let root = project("open_file", &[]);
    let mut app = editor(&root);
    let map_rel = {
        let project = app.world().resource::<Project>();
        let open = app.world().resource::<OpenMap>();
        open.map_path.strip_prefix(&project.root).unwrap_or_else(|e| panic!("{e}")).to_string_lossy().replace('\\', "/")
    };
    assert!(editor::lock_on_placement(app.world_mut(), "bed@0"), "the fixture places `bed@0`");
    settle(&mut app);
    assert!(drawn(&mut app, "placed in m"), "the inspector says which maps place the selected piece; on screen: {:?}", on_screen(&mut app));

    let button = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_cauldron::crosslinks::OpenFileButton>>().single(world).unwrap_or_else(|e| panic!("one Open file button: {e}"))
    };
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: button });
    settle(&mut app);

    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "Open file leaves the Map for the Files workspace");
    assert_eq!(files::active_file(app.world().resource::<Session>()).as_deref(), Some(map_rel.as_str()), "on the map's own file");
    let text = std::fs::read_to_string(root.join(&map_rel)).unwrap_or_else(|e| panic!("{e}"));
    let want = text.lines().position(|l| l.contains("\"bed@0\"")).expect("the map's file writes the placement's id");
    assert_eq!(caret_line(&mut app), want, "the caret is on the line that writes the placement (0-based), not at the top");
}

/// **Open piece file** goes to the library's entry for the placement's piece.
#[test]
fn open_piece_file_lands_in_the_library_at_the_piece() {
    let root = project("open_piece", &[]);
    let mut app = editor(&root);
    let piece_rel = {
        let project = app.world().resource::<Project>();
        project.grimoire_dir.join("library.ron").strip_prefix(&project.root).unwrap_or_else(|e| panic!("{e}")).to_string_lossy().replace('\\', "/")
    };
    assert!(editor::lock_on_placement(app.world_mut(), "bed@0"));
    settle(&mut app);
    let button = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_cauldron::crosslinks::OpenPieceFileButton>>().single(world).unwrap_or_else(|e| panic!("one Open piece file button: {e}"))
    };
    app.world_mut().commands().trigger(bevy::ui_widgets::Activate { entity: button });
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files);
    assert_eq!(files::active_file(app.world().resource::<Session>()).as_deref(), Some(piece_rel.as_str()), "on the library's file");
    let text = std::fs::read_to_string(root.join(&piece_rel)).unwrap_or_else(|e| panic!("{e}"));
    let want = text.lines().position(|l| l.contains("\"bed\"")).expect("the library writes the piece's id");
    assert_eq!(caret_line(&mut app), want, "the caret is on the piece's own entry");
}

/// **Acceptance 5 — a piece's Used by in the Kit** lists the maps that place it.
#[test]
fn a_pieces_used_by_in_the_kit_lists_the_maps_that_place_it() {
    let root = project("kit", &[]);
    let mut app = editor(&root);
    app.world_mut().resource_mut::<Revealing>().request_by_person("test", &Shown::Piece { id: "bed".to_owned() });
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit, "a piece is shown on the Kit workspace");
    assert!(drawn(&mut app, "placed in m"), "the Kit's piece pane says which maps place it; on screen: {:?}", on_screen(&mut app));
}
