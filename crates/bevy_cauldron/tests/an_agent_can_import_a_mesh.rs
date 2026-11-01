//! **An agent can import a mesh as a kit piece (S2.2, `piece_import`).**
//!
//! The emergence chain is mesh → piece → composition → generated layout → scene. `layout_propose`
//! covers the end and `placement_add` the middle; nothing joined golem's output to the palette. This
//! is that bridge — and it is a **bridge** and not a second importer: golem makes the mesh
//! (`AGENTS.md`, "Blender is a tool we own"), and this measures a `.glb` already inside the project
//! and runs the Meshes tab's own four admissions.
//!
//! # What is being proved
//!
//! 1. **It is in the registry**, so an agent can find it.
//! 2. **An imported `.glb` is in the library and on disk**, under the id it was given.
//! 3. **A path outside the project's assets folder is refused** — the brief's rule for a path
//!    parameter.
//! 4. **A file that is not a `.glb` is refused**, because a `.gltf` is a file plus its `.bin`.
//! 5. **Undo takes the piece out of the library.**
//! 6. **Undo is refused while a map places it**, naming the map — an orphaned placement is not an
//!    error a map can carry.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::{OpenMap, Project};

use crate::fixtures;

/// **A project whose own fixture wrote a `.glb`**, copied into `assets/inbox/` under a name the test
/// chose. The fixture's `descriptor` writes a mesh, so there is a real GLB to import without
/// inventing one here.
fn project(name: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(name)
        .descriptor("crate", "alpha")
        .build("m");
    let written = root
        .join("assets")
        .join("alpha")
        .join("crate.glb");
    let inbox = root.join("assets").join("inbox");
    std::fs::create_dir_all(&inbox).unwrap_or_else(|e| panic!("{e}"));
    std::fs::copy(&written, inbox.join("box.glb"))
        .unwrap_or_else(|e| panic!("{}: {e}", written.display()));
    root
}

fn editor_with_bridge(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

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

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": operation,
            "parameters": parameters,
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

fn ready(name: &str) -> (std::path::PathBuf, App) {
    let root = project(name);
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    (root, app)
}

/// **1. The operation is listed.**
#[test]
fn piece_import_is_listed() {
    let (_root, mut app) = ready("implist");
    let listed = call(&mut app, "grimoire/operation_list", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("{e}"));
    let names: Vec<&str> = listed["operations"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|o| o.get("operation").and_then(|n| n.as_str()))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        names.contains(&"piece_import"),
        "nothing joins a golem .glb to the palette, so the emergence chain has a hole at its first \
         scale. The registry holds: {names:?}"
    );
}

/// **2. An imported `.glb` is in the library and on disk.**
#[test]
fn an_agent_imports_a_glb_and_it_is_in_the_library() {
    let (_root, mut app) = ready("impagent");
    let applied = apply(
        &mut app,
        "piece_import",
        serde_json::json!({"glb": "assets/inbox/box.glb", "id": "inbox_box"}),
    );
    assert_eq!(applied["outcome"], "applied", "the import did not apply: {applied}");

    let project = app.world().resource::<Project>();
    let imported = project
        .library
        .get("inbox_box")
        .unwrap_or_else(|| panic!("`inbox_box` is not in the library: {applied}"));
    assert_eq!(
        imported.mesh.as_deref(),
        Some("inbox/box.glb"),
        "the descriptor does not name the mesh it was made from"
    );

    // **On disk.** The library file is what the next launch reads.
    let lib = project.root.join("assets/grimoire").join(&project.namespace).join("library.ron");
    let text = std::fs::read_to_string(&lib).unwrap_or_else(|e| panic!("{}: {e}", lib.display()));
    assert!(
        text.contains("inbox_box"),
        "the library holds the piece and the file does not"
    );
    assert!(
        text.contains("inbox/box.glb"),
        "the file does not name the mesh"
    );
}

/// **3. A path outside the assets folder is refused.**
#[test]
fn a_path_outside_the_assets_folder_is_refused() {
    let (_root, mut app) = ready("impoutside");
    let refused = apply(
        &mut app,
        "piece_import",
        serde_json::json!({"glb": "../outside.glb", "id": "nope"}),
    );
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("outside") || why.contains("assets"),
        "the refusal does not say the path left the project: {why:?}"
    );
}

/// **4. A file that is not a `.glb` is refused.**
#[test]
fn a_file_that_is_not_a_glb_is_refused() {
    let (root, mut app) = ready("impnotglb");
    // A real file, so the refusal is about its extension and not about it being missing.
    let path = root.join("assets").join("inbox").join("box.gltf");
    std::fs::write(&path, b"{}").unwrap_or_else(|e| panic!("{e}"));
    let refused = apply(
        &mut app,
        "piece_import",
        serde_json::json!({"glb": "assets/inbox/box.gltf", "id": "nope"}),
    );
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains(".glb"),
        "the refusal does not say what it wanted: {why:?}"
    );
}

/// **5. Undo takes the piece out of the library.**
#[test]
fn undo_takes_the_piece_out_of_the_library() {
    let (_root, mut app) = ready("impundo");
    let imported = apply(
        &mut app,
        "piece_import",
        serde_json::json!({"glb": "assets/inbox/box.glb", "id": "inbox_box"}),
    );
    let id = imported["operation_id"]
        .as_str()
        .unwrap_or_else(|| panic!("the import names no operation id: {imported}"))
        .to_owned();
    assert!(app.world().resource::<Project>().library.get("inbox_box").is_some());

    // **By id, because a library change is the Meshes history, not the map's.** `operation_undo`
    // with no id resolves against the open map and answers *"no change to the open map is still in
    // effect"* — true, and useless here.
    let undone = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(undone["outcome"], "applied", "the undo did not apply: {undone}");
    assert!(
        app.world().resource::<Project>().library.get("inbox_box").is_none(),
        "one undo did not take the imported piece out of the library"
    );
}

/// **6. Undo is refused while a map places it**, naming the map.
#[test]
fn undo_is_refused_while_a_map_places_it() {
    let (_root, mut app) = ready("impused");
    let imported = apply(
        &mut app,
        "piece_import",
        serde_json::json!({"glb": "assets/inbox/box.glb", "id": "inbox_box"}),
    );
    let placed = apply(
        &mut app,
        "placement_add",
        serde_json::json!({"piece": "inbox_box", "at": [2.5, 2.5]}),
    );
    assert_eq!(placed["outcome"], "applied", "could not place the import: {placed}");
    let _ = app.world().resource::<OpenMap>();

    let import_id = imported["operation_id"]
        .as_str()
        .expect("the import names its id")
        .to_owned();
    let undone = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": import_id, "dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        undone["outcome"], "refused",
        "an import a map places was taken out, so the map now names a piece nothing defines: \
         {undone}"
    );
    let why = undone["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("inbox_box"),
        "the refusal does not name the piece: {why:?}"
    );
    assert!(
        app.world().resource::<Project>().library.get("inbox_box").is_some(),
        "a refused undo took the piece out anyway"
    );
}
