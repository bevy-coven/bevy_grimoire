//! **The Files tree shows the row of the file a follow opened** (AF-03, #359; series agent follow, #356).
//!
//! `Shown::File` takes the person to the Files workspace at the file and the line. The tree beside the editor
//! selected the file's row, but a file in a folder the tree had not opened has no row to select, and a long tree
//! left the selected row below its fold: the change was open and the tree named nothing the person could see. The
//! acceptance says the item's row lies inside its list's visible area, **measured from the row's own
//! `ComputedNode`**, so this finds the row by what it says and compares it with the tree's own scrolled box.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};

use bevy_cauldron::files::FilesTree;
use bevy_cauldron::harness;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

const FRAME: f32 = 1.0 / 60.0;

/// Frames a follow may take: the crossing, the tree's first walk (a thread, polled), and the scroll a frame late.
const SETTLE: usize = 90;

/// Folders that fill the tree past its fold, so the file is neither the first row nor on screen by luck.
const FILLER_FOLDERS: usize = 40;

const TARGET: &str = "zz/deep/nest/target.yaml";

fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("lamp_tall", "alpha").place("lamp_tall", (2.0, 2.0)).build("m");
    for n in 0..FILLER_FOLDERS {
        let dir = root.join(format!("docs_{n:02}"));
        std::fs::create_dir_all(&dir).expect("filler folder");
        std::fs::write(dir.join("note.txt"), "filler\n").expect("filler file");
    }
    let target = root.join(TARGET);
    std::fs::create_dir_all(target.parent().expect("parent")).expect("target folder");
    std::fs::write(target, "# a file the agent edits\na: 1\nb: 2\n").expect("target");
    root
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Map, "the test starts on the Map door");
    app
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/operation_apply").cloned() else {
        panic!("grimoire/operation_apply is not registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false})))
        .unwrap_or_else(|e| panic!("{operation}: {e:?}"))
        .unwrap_or_else(|e| panic!("{operation}: {e:?}"))
}

fn box_of(app: &App, e: Entity) -> Option<(Vec2, Vec2)> {
    let node = app.world().get::<ComputedNode>(e)?;
    let at = app.world().get::<UiGlobalTransform>(e)?;
    Some((at.translation, node.size() * 0.5))
}

/// **The row's text lies inside the tree's scrolled box**, from the two nodes' own geometry.
fn inside(app: &App, row: Entity, list: Entity) -> Result<(), String> {
    let (row_mid, row_half) = box_of(app, row).ok_or_else(|| "the row is not laid out".to_owned())?;
    let (list_mid, list_half) = box_of(app, list).ok_or_else(|| "the list is not laid out".to_owned())?;
    if row_half.y <= 0.0 || list_half.y <= 0.0 {
        return Err(format!("a zero-height node: row {row_half:?}, list {list_half:?}"));
    }
    let (top, bottom) = (list_mid.y - list_half.y, list_mid.y + list_half.y);
    let (row_top, row_bottom) = (row_mid.y - row_half.y, row_mid.y + row_half.y);
    if row_top >= top && row_bottom <= bottom {
        Ok(())
    } else {
        Err(format!("the row spans {row_top:.1}..{row_bottom:.1} and the list {top:.1}..{bottom:.1}"))
    }
}

/// The nearest ancestor of `from` that scrolls, and whether the Files tree is above it.
fn scroller_in_the_tree(app: &mut App, from: Entity) -> Option<Entity> {
    let world = app.world_mut();
    let mut owner = world.query::<&ChildOf>();
    let mut scrolls = world.query_filtered::<Entity, With<ScrollPosition>>();
    let trees: Vec<Entity> = world.query_filtered::<Entity, With<FilesTree>>().iter(world).collect();
    let (mut at, mut scroller) = (from, None);
    loop {
        if scroller.is_none() && scrolls.get(world, at).is_ok() {
            scroller = Some(at);
        }
        if trees.contains(&at) {
            return scroller;
        }
        at = owner.get(world, at).ok()?.0;
    }
}

/// **The text of the row the tree draws for `name`**, inside the Files tree.
fn tree_row_text(app: &mut App, name: &str) -> Option<Entity> {
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &Text)>();
    let found: Vec<Entity> = texts.iter(world).filter(|(_, t)| t.0 == name).map(|(e, _)| e).collect();
    found.into_iter().find(|e| scroller_in_the_tree(app, *e).is_some())
}

#[test]
fn a_followed_files_row_is_opened_and_inside_the_trees_visible_area() {
    let root = project("files_tree_row");
    let mut app = editor(&root);
    let answer = apply(&mut app, "file_edit", serde_json::json!({"path": TARGET, "edits": [{"at": "a", "value": 2}]}));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    for _ in 0..SETTLE {
        app.update();
    }
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "the follow opened the Files workspace");
    assert_eq!(
        bevy_cauldron::files::active_file(app.world().resource::<bevy_cauldron::session::Session>()).as_deref(),
        Some(TARGET),
        "on the file the operation wrote"
    );

    let row = tree_row_text(&mut app, "target.yaml").unwrap_or_else(|| panic!("the Files tree draws no row for target.yaml: its folders are shut, or the row is not built"));
    let list = scroller_in_the_tree(&mut app, row).expect("the row is in a scrolled box of the tree");
    if let Err(why) = inside(&app, row, list) {
        panic!("target.yaml's row is not inside the Files tree's visible area: {why}");
    }
}
