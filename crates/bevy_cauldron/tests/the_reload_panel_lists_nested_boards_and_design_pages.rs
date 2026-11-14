//! **The Reload panel lists a place's board, its pictures and a nested design page** (#420, found by the #402 and #403 reviews).
//!
//! The panel's walk stopped at the deepest row pattern (three segments for `design/board/board.yaml`), so a level's
//! `design/levels/<map>/board/…` and a nested `design/` page were classified by `reload::kind_of` and never listed. The
//! walk now enters a folder by the shape `kind_of` classifies by (`reload::holds_place_files`), with no depth of its own.

use bevy::prelude::*;
use bevy_cauldron::project::Project;
use bevy_cauldron::reload_files::{self, FileReload};
use bevy_grimoire_core::board::{Board, Entry};
use serde_json::{Value, json};

use crate::fixtures;

const NESTED_PAGE: &str = "design/regions/north/levels/m/areas/docks/style.md";
const LEVEL_PAGE: &str = "design/levels/m/style.md";
const LEVEL_BOARD: &str = "design/levels/m/board/board.yaml";
const LEVEL_PICTURE: &str = "design/levels/m/board/fog.png";

fn editor(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    (app, root)
}

/// A project with a nested board, a picture on it, a page of the level and a page of an area of a region's level.
fn write_the_nested_files(root: &std::path::Path) {
    let put = |rel: &str, bytes: &[u8]| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    };
    let mut board = Board::default();
    board.add(Entry::new(LEVEL_PICTURE, "the fog's colour, not its density"), None).unwrap();
    board.save(&root.join(LEVEL_BOARD)).unwrap();
    put(LEVEL_PICTURE, b"not a real picture; the walk only looks at its name");
    put(LEVEL_PAGE, b"# the level's feel\n");
    put(NESTED_PAGE, b"# the docks\n");
    // What is not a place's file is not listed.
    put("design/levels/m/board/notes.txt", b"no kind");
    put("design/topics/x/board/board.yaml", b"not a place");
}

fn listed<'a>(files: &'a [FileReload], file: &str) -> Option<&'a FileReload> {
    files.iter().find(|f| f.file == file)
}

#[test]
fn a_nested_board_its_picture_and_a_nested_design_page_are_listed_with_their_kinds() {
    let (app, root) = editor("reload_nested");
    write_the_nested_files(&root);
    let project = app.world().resource::<Project>();
    let files = reload_files::files(&project.project_dir, &project.root, None, &[]);
    for (file, kind) in [(LEVEL_BOARD, "board"), (LEVEL_PICTURE, "reference_images"), (LEVEL_PAGE, "design"), (NESTED_PAGE, "design")] {
        let row = listed(&files, file).unwrap_or_else(|| panic!("{file} is not listed: {:?}", files.iter().map(|f| &f.file).collect::<Vec<_>>()));
        assert_eq!(row.kind, kind, "{file}");
        assert!(row.modified_ms.is_some(), "{file}: a listed file says when it was written");
    }
    for file in ["design/levels/m/board/notes.txt", "design/topics/x/board/board.yaml"] {
        assert!(listed(&files, file).is_none(), "{file} is no kind's, so it is not a row");
    }
}

#[test]
fn the_panels_header_counts_the_nested_files_and_the_agents_state_lists_them() {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let (mut app, root) = editor("reload_nested_state");
    let before = {
        let project = app.world().resource::<Project>();
        reload_files::files(&project.project_dir, &project.root, None, &[]).len()
    };
    write_the_nested_files(&root);
    let project = app.world().resource::<Project>().clone();
    let files = reload_files::files(&project.project_dir, &project.root, None, &[]);
    assert_eq!(files.len(), before + 4, "the four nested files are four more rows");
    let lines = reload_files::panel_lines(&files, None, true, reload_files::now_ms());
    assert_eq!(lines[0].0, format!("no game connected ({} files)", before + 4), "the panel's header counts them");

    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    let state: Value = app.world_mut().run_system_with(id, Some(json!({}))).unwrap_or_else(|e| panic!("{e:?}")).unwrap_or_else(|e| panic!("{e:?}"));
    let rows = state["reload_files"]["files"].as_array().unwrap_or_else(|| panic!("no reload_files.files in {state}"));
    for file in [LEVEL_BOARD, LEVEL_PICTURE, LEVEL_PAGE, NESTED_PAGE] {
        assert!(rows.iter().any(|r| r["file"] == file), "{file} is not in grimoire_state.reload_files");
    }
}
