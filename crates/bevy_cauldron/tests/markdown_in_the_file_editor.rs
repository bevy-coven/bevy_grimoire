//! **Markdown in the file editor** (SG-04, #403; the series the game's style, #399).
//!
//! What is measured, as a person reaches it:
//!
//! 1. **A path that is not there is an error on its line**, [`DIAGNOSE_IDLE_S`] after the last key and not before,
//!    for a picture and for a link; a URL, a `mailto:` and a bare `#section` are not this project's to check. The
//!    reader is the design tree's (`bevy_grimoire_core::design::cites`), so the editor and `grimoire_state.design`
//!    say one thing.
//! 2. **Completing after `](` lists the place's board first**, then the project's files; **after `![`** the board's
//!    pictures with what to take from each. The broken picture is mended by taking the first board picture.
//! 3. **Hover on a picture's path says its caption and licence**, from the board.
//! 4. **The outline lists the page's headings**, and not a `#` inside a fence.
//! 5. **The registry**: one language per extension, behind one trait.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::files::{self, DIAGNOSE_IDLE_S, FileOutline};
use bevy_cauldron::files_lang::{self, Env, Lang};
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::workspace::Workspace;
use bevy_grimoire_core::board::{self, Board, Entry};
use bevy_gooey::{CodeCompletionRequested, CodeCompletions, CodeEditor, CodeEditorValue, CodeHover, CodeHoverRequested, CodePos, CompletionTrigger, Diagnostics};

use crate::fixtures;

const SETTLE: usize = 12;

/// The page the tests edit: the level `m`'s own style, which sits beside its board.
const PAGE: &str = "design/levels/m/style.md";

const FOG: &str = "design/levels/m/board/fog.png";
const FOG_CAPTION: &str = "the fog's colour, not its density";
const DUSK: &str = "design/levels/m/board/dusk.png";

const PAGE_TEXT: &str = "# Yard\n\nGrey concrete.\n\n## Light\n\n![fog](board/fog.png)\n\n```\n# not a heading\n```\n";

fn project(tag: &str) -> PathBuf {
    let root = fixtures::Fixture::new(&format!("md_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    let write = |rel: &str, bytes: &[u8]| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(path, bytes).unwrap_or_else(|e| panic!("{e}"));
    };
    write(PAGE, PAGE_TEXT.as_bytes());
    write(FOG, b"a picture");
    write(DUSK, b"another picture");
    write("design/levels/m/notes.txt", b"words");
    let mut board_of_m = Board::default();
    let mut fog = Entry::new(FOG, FOG_CAPTION);
    fog.licence = Some("CC BY 4.0".to_owned());
    fog.creator = Some("a gull".to_owned());
    board_of_m.add(fog, None).unwrap_or_else(|e| panic!("{e}"));
    let mut dusk = Entry::new(DUSK, "how the light leaves the roofs first");
    dusk.restricted = true;
    board_of_m.add(dusk, None).unwrap_or_else(|e| panic!("{e}"));
    board_of_m.save(&root.join(board::file_of("design/levels/m"))).unwrap_or_else(|e| panic!("{e}"));
    root
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

/// An editor on `root`, in the Files workspace, with `rel` open and the code editor focused.
fn files_editor(root: &Path, rel: &str) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files);
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

/// Wait out the idle the errors wait for, in real time, and let the editor see it.
fn idle(app: &mut App) {
    std::thread::sleep(Duration::from_secs_f64(DIAGNOSE_IDLE_S + 0.05));
    settle(app);
}

/// `(line, message)` of the findings drawn for the text the editor holds now.
fn findings(app: &mut App) -> Vec<(usize, String)> {
    let editor = editor_of(app);
    let lines = text(app);
    let Some(diagnostics) = app.world().get::<Diagnostics>(editor) else { return Vec::new() };
    let value = app.world().get::<CodeEditorValue>(editor).map(CodeEditorValue::revision).unwrap_or(0);
    if diagnostics.revision() != value {
        return Vec::new();
    }
    diagnostics.items().iter().map(|d| (lines[..d.range.start].matches('\n').count(), d.message.clone())).collect()
}

fn ask_for_completions(app: &mut App, offset: usize) -> CodeCompletions {
    let editor = editor_of(app);
    let source = text(app);
    let line = source[..offset].matches('\n').count();
    let col = offset - source[..offset].rfind('\n').map_or(0, |at| at + 1);
    caret_at(app, line, col);
    let revision = app.world().get::<CodeEditorValue>(editor).map(CodeEditorValue::revision).unwrap_or(0);
    app.world_mut().trigger(CodeCompletionRequested { entity: editor, offset, prefix: String::new(), trigger: CompletionTrigger::Manual, revision });
    app.update();
    app.world().get::<CodeCompletions>(editor).cloned().unwrap_or_default()
}

/// **Acceptance 1**: a picture path typed that is not there is an error on its line once the person pauses and not
/// before; a board picture taken from the completion mends it.
#[test]
fn a_picture_path_that_does_not_exist_is_an_error_on_its_line_and_the_boards_completion_mends_it() {
    assert_eq!(DIAGNOSE_IDLE_S, 0.15, "the issue's 150 ms of idle");
    let root = project("fix");
    let mut app = files_editor(&root, PAGE);
    assert_eq!(text(&mut app), PAGE_TEXT);
    assert!(findings(&mut app).is_empty(), "the page cites a picture that is there: {:?}", findings(&mut app));

    // Type a picture that is not there on a line of its own at the end.
    let broken_line = PAGE_TEXT.matches('\n').count();
    caret_at(&mut app, broken_line, 0);
    bevy_gooey::testing::type_text(&mut app, "![a gull](board/gul.png)");
    assert!(findings(&mut app).is_empty(), "no error is drawn on text the person is still typing");
    idle(&mut app);
    let found = findings(&mut app);
    assert_eq!(found.len(), 1, "one finding: {found:?}");
    assert_eq!(found[0].0, broken_line, "on the line the picture was typed on: {found:?}");
    assert!(found[0].1.contains("board/gul.png") && found[0].1.contains("picture"), "and it names the path: {found:?}");

    // Take the path away, and ask what goes after `](`: the board's pictures, the first one first.
    let line_text = "![a gull](board/gul.png)";
    let from = "![a gull](".len();
    select(&mut app, CodePos::new(broken_line, from), CodePos::new(broken_line, line_text.len() - 1));
    bevy_gooey::testing::press(&mut app, KeyCode::Backspace);
    let source = text(&mut app);
    let after_open = source.rfind("](").map_or(0, |at| at + 2);
    let offered = ask_for_completions(&mut app, after_open);
    assert_eq!(offered.items.first().map(|i| i.label.as_str()), Some("board/fog.png"), "the board's first picture leads: {:?}", offered.items);
    bevy_gooey::testing::press(&mut app, KeyCode::Enter);
    idle(&mut app);
    assert!(text(&mut app).contains("![a gull](board/fog.png)"), "the chosen picture is in the page: {}", text(&mut app));
    assert!(findings(&mut app).is_empty(), "and the page reads: {:?}", findings(&mut app));
}

/// **Acceptance 1, the order**: after `](` the place's board first, then the project's files; after `![` the board's
/// pictures with their captions.
#[test]
fn completing_after_a_link_lists_the_boards_pictures_then_the_projects_files_and_after_a_picture_the_captions() {
    let root = project("complete");
    let mut app = files_editor(&root, PAGE);
    let editor = editor_of(&mut app);
    let broken_line = PAGE_TEXT.matches('\n').count();
    caret_at(&mut app, broken_line, 0);
    app.world_mut().get_mut::<CodeEditorValue>(editor).map(|mut v| v.set(format!("{PAGE_TEXT}[a page](\n![\n"))).unwrap_or_default();
    settle(&mut app);
    let source = text(&mut app);

    // The project's files come from the tree, which is read off the UI thread: wait for it.
    let after_link = source.find("[a page](").map_or(0, |at| at + "[a page](".len());
    let mut offered = ask_for_completions(&mut app, after_link);
    for _ in 0..40 {
        if offered.items.iter().any(|i| i.label == "notes.txt") {
            break;
        }
        std::thread::sleep(Duration::from_millis(60));
        offered = ask_for_completions(&mut app, after_link);
    }
    let labels: Vec<&str> = offered.items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(&labels[..2], ["board/fog.png", "board/dusk.png"], "the board's pictures lead, in the board's order: {labels:?}");
    assert_eq!(offered.items[0].detail, FOG_CAPTION, "each says what to take from it");
    assert!(offered.items[1].detail.contains("restricted"), "and a restricted one says it never ships: {:?}", offered.items[1]);
    let notes = labels.iter().position(|l| *l == "notes.txt").unwrap_or_else(|| panic!("the project's files follow: {labels:?}"));
    assert!(notes >= 2, "after the board: {labels:?}");
    assert_eq!(labels.iter().filter(|l| **l == "board/fog.png").count(), 1, "a picture on the board is not offered twice: {labels:?}");
    assert!(!labels.contains(&"style.md"), "the page itself is not offered: {labels:?}");

    // After `![`: the captions, each inserting the whole picture.
    let after_bang = source.rfind("![").map_or(0, |at| at + 2);
    let offered = ask_for_completions(&mut app, after_bang);
    assert_eq!(offered.items.first().map(|i| i.label.as_str()), Some(FOG_CAPTION), "{:?}", offered.items);
    assert_eq!(offered.items[0].text(), format!("{FOG_CAPTION}](board/fog.png)"), "a caption inserts the picture it belongs to");
}

/// **Acceptance 2**: hover on a picture's path says its caption and licence; the outline lists the headings.
#[test]
fn hover_on_a_pictures_path_says_its_caption_and_licence_and_the_outline_lists_the_headings() {
    let root = project("hover");
    let mut app = files_editor(&root, PAGE);
    let editor = editor_of(&mut app);
    let source = text(&mut app);
    let at = source.find("board/fog.png").map_or(0, |at| at + 3);
    let revision = app.world().get::<CodeEditorValue>(editor).map(CodeEditorValue::revision).unwrap_or(0);
    app.world_mut().trigger(CodeHoverRequested { entity: editor, offset: at, revision });
    app.update();
    let said = app.world().get::<CodeHover>(editor).cloned().unwrap_or_else(|| panic!("a picture on the board has something to say"));
    assert!(said.text.contains(FOG_CAPTION) && said.text.contains("CC BY 4.0"), "{}", said.text);
    assert_eq!(&source[said.range], "board/fog.png", "the note is about the path");

    let outline = app.world().get::<FileOutline>(editor).cloned().unwrap_or_else(|| panic!("a page has an outline"));
    let said: Vec<(usize, &str, usize)> = outline.headings.iter().map(|h| (h.level, h.title.as_str(), h.line)).collect();
    assert_eq!(said, [(1, "Yard", 0), (2, "Light", 4)], "the headings, and not the `#` inside the fence");
}

/// A link is judged as a picture is; what names no file of the project is left alone.
#[test]
fn a_link_to_a_page_that_is_not_there_is_an_error_and_a_url_an_anchor_and_a_mail_address_are_not() {
    let root = project("links");
    let env = Env { root: Some(&root), refs: &|_| Vec::new(), files: &Vec::new };
    let text = "[gone](missing.md) [here](notes.txt) [part](notes.txt#top) [web](https://example.com/a.md) [mail](mailto:a@b.c) [up](#heading) [dir](board)\n";
    let found = files_lang::diagnose_in(&env, PAGE, text);
    let said: Vec<&str> = found.iter().map(|f| &text[f.range.clone()]).collect();
    assert_eq!(said, ["missing.md"], "only the page that is not there: {found:?}");
    assert!(found[0].message.contains("page"), "{}", found[0].message);
    assert!(files_lang::diagnose(PAGE, text).is_empty(), "with no project around the page nothing is looked for");

    // A path that climbs out of the project names no file of it: no finding, and nothing to say on hover.
    let outside = "[x](../../../../etc/passwd)\n";
    assert!(files_lang::diagnose_in(&env, PAGE, outside).is_empty(), "{:?}", files_lang::diagnose_in(&env, PAGE, outside));
    assert!(files_lang::hover(PAGE, outside, 6, None, &env).is_none());
}

/// A section folds from its heading, whichever way the heading is written.
#[test]
fn a_setext_heading_folds_as_it_is_listed() {
    let text = "Title\n=====\nbody\n\nNext\n-----\nmore\n";
    assert_eq!(files_lang::folds(Lang::Markdown, text), vec![0..7, 4..7]);
    let outline = files_lang::outline(Lang::Markdown, text);
    let said: Vec<(usize, &str, usize)> = outline.iter().map(|h| (h.level, h.title.as_str(), h.line)).collect();
    assert_eq!(said, [(1, "Title", 0), (2, "Next", 4)], "the outline lists the same headings the folds start from");
}

/// **The registry**: an extension is one language, and every language answers through the one trait.
#[test]
fn every_extension_is_one_language_in_one_registry() {
    for (path, lang) in [("a.yaml", Lang::Yaml), ("a.YML", Lang::Yaml), ("a.ron", Lang::Ron), ("a.rhai", Lang::Rhai), ("a.md", Lang::Markdown), ("a.markdown", Lang::Markdown), ("a.txt", Lang::Plain), ("noext", Lang::Plain)] {
        assert_eq!(Lang::of(path), lang, "{path}");
        assert_eq!(lang.language().lang(), lang, "{path}: the language answers for itself");
    }
    let mut seen: Vec<&str> = Vec::new();
    for language in files_lang::REGISTRY.iter() {
        for ext in language.extensions() {
            assert!(!seen.contains(ext), "`{ext}` is in two languages");
            seen.push(ext);
        }
    }
    assert_eq!(files_lang::outline(Lang::Markdown, "# a\n## b\n").len(), 2);
    assert!(files_lang::outline(Lang::Yaml, "a: 1\n").is_empty(), "a language with no headings has none");
}
