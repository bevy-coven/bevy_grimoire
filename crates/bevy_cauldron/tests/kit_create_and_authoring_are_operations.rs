//! **An agent can make a kit and say where new work lands, and the project chooser's two verbs are the
//! same operations (E-002, #129 slice 20).**
//!
//! The chooser's `+ new kit` row and `A` on the kits column wrote `assets/grimoire` with writes of their own,
//! so an agent could import meshes and place pieces and never start the kit they belong in. They are now
//! `kit_create` and `kit_set_authoring`. The agent tests go through the registered BRP methods; the person
//! tests use the project page's New kit dialog and its Make default item, from a page that has a
//! root and no loaded project, and assert on `kits.yaml`, the files and the log.

use bevy::prelude::*;
use bevy::ui_widgets::{Activate, ValueChange};
use bevy_gooey::FieldProblem;
use bevy_cauldron::project_page::{CreateButton, NameField, PageAction, PageDialog};
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (root, app)
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

fn kits_ron(root: &std::path::Path) -> String {
    std::fs::read_to_string(root.join("assets/grimoire/kits.yaml")).expect("kits.yaml")
}

fn log_at(root: &std::path::Path) -> Vec<Entry> {
    std::fs::read_to_string(ops::oplog_path_at(root))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **One call makes the kit, binds it, and one undo takes it away and puts kits.yaml back byte for byte.**
#[test]
fn an_agent_makes_a_kit_and_one_undo_takes_it_away() {
    let (root, mut app) = editor("kit-agent");
    let kits_before = kits_ron(&root);
    let lines = log_at(&root).len();

    let a = apply(&mut app, "kit_create", serde_json::json!({"name": "Tavern Props"}));
    assert_eq!(a["outcome"], "applied", "{a}");
    let dir = root.join("assets/grimoire/tavern_props");
    assert!(dir.join("library.yaml").exists() && dir.join("project.yaml").exists(), "the kit's two files are not there");
    assert!(kits_ron(&root).contains("tavern_props"), "kits.yaml does not bind it: {}", kits_ron(&root));
    assert_eq!(log_at(&root).len(), lines + 1, "a kit is one log line");

    let again = apply(&mut app, "kit_create", serde_json::json!({"name": "tavern props"}));
    assert_eq!(again["outcome"], "refused", "a second kit of one name was made: {again}");

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert!(!dir.exists(), "one undo takes the kit's directory away");
    assert_eq!(kits_ron(&root), kits_before, "and puts kits.yaml back byte for byte");
}

/// **An undo never deletes what an author has put in the kit since.**
#[test]
fn an_undo_leaves_a_kit_someone_has_added_to() {
    let (root, mut app) = editor("kit-touched");
    let a = apply(&mut app, "kit_create", serde_json::json!({"name": "shed"}));
    assert_eq!(a["outcome"], "applied", "{a}");
    let dir = root.join("assets/grimoire/shed");
    std::fs::write(dir.join("notes.txt"), "mine").expect("a file in the kit");
    let id = a["operation_id"].as_str().expect("id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "refused", "an undo deleted a kit with a file in it: {back}");
    assert!(dir.join("notes.txt").exists() && dir.join("library.yaml").exists(), "the kit lost files");
}

/// **A bad name, an existing kit, an undo an agent forges and a typo are refused, and nothing is written.**
#[test]
fn bad_kits_are_refused_and_nothing_is_written() {
    let (root, mut app) = editor("kit-refused");
    let kits_before = kits_ron(&root);
    let lines = log_at(&root).len();
    for (what, params) in [
        ("no usable name", serde_json::json!({"name": "!!!"})),
        ("a kit that exists", serde_json::json!({"name": "site"})),
        ("a typo'd field", serde_json::json!({"nmae": "x"})),
        ("an undo from an agent", serde_json::json!({"remove": "site", "kits": "()"})),
    ] {
        let r = apply(&mut app, "kit_create", params);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    assert_eq!(kits_ron(&root), kits_before, "a refusal wrote kits.yaml");
    assert_eq!(log_at(&root).len(), lines, "a refusal logged a line");
    assert!(root.join("assets/grimoire/site").exists(), "the existing kit was touched");
}

/// **An agent points authoring at a bound kit, and one undo points it back; an unbound kit, the kit that is
/// already authoring and an agent's own undo form are refused.**
#[test]
fn an_agent_chooses_the_authoring_kit_and_one_undo_chooses_it_back() {
    let (root, mut app) = editor("kit-authoring");
    let made = apply(&mut app, "kit_create", serde_json::json!({"name": "second"}));
    assert_eq!(made["outcome"], "applied", "{made}");
    let authoring = |app: &App| -> Option<String> {
        let _ = app;
        bevy_grimoire_core::kits::Kits::parse(&kits_ron(&root)).expect("kits.yaml").authoring
    };
    let first = authoring(&app).expect("the fixture names an authoring kit");
    assert_ne!(first, "second", "the new kit took the authoring seat from the one that had it");
    let lines = log_at(&root).len();

    let a = apply(&mut app, "kit_set_authoring", serde_json::json!({"kit": "second"}));
    assert_eq!(a["outcome"], "applied", "{a}");
    assert_eq!(authoring(&app).as_deref(), Some("second"));
    assert_eq!(log_at(&root).len(), lines + 1, "choosing is one log line");

    for (what, params) in [
        ("a kit that is not bound", serde_json::json!({"kit": "nope"})),
        ("the kit that is already authoring", serde_json::json!({"kit": "second"})),
        ("an undo's form from an agent", serde_json::json!({"none": true})),
        ("no kit", serde_json::json!({})),
    ] {
        let r = apply(&mut app, "kit_set_authoring", params);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    assert_eq!(authoring(&app).as_deref(), Some("second"), "a refusal moved authoring");

    let id = a["operation_id"].as_str().expect("id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "applied", "{back}");
    assert_eq!(authoring(&app).as_deref(), Some(first.as_str()), "one undo chooses the first kit again");
    let _ = app.world().resource::<Project>();
}

/// **The page's New kit dialog is one `kit_create` line stamped as a person's, in the root's log**, from a screen that has a root and no
/// loaded project.
#[test]
fn the_pages_new_kit_dialog_makes_the_kit_through_the_operation() {
    let root = fixtures::Fixture::new("kit-page").descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    harness::stand_on_the_project_page(&mut app, &root);
    let lines = log_at(&root).len();

    make_kit(&mut app, "cellar");

    assert!(root.join("assets/grimoire/cellar/library.yaml").exists(), "the dialog made no kit");
    assert!(kits_ron(&root).contains("cellar"), "kits.yaml does not bind it");
    let entries = log_at(&root);
    assert_eq!(entries.len(), lines + 1, "one dialog is one log line");
    let last = entries.last().expect("a line");
    assert_eq!(last.op, "kit_create");
    assert_eq!(format!("{:?}", last.source), "Human", "a person's kit must be stamped as a person's");
    assert!(
        app.world_mut().query_filtered::<(), With<PageDialog>>().iter(app.world()).next().is_none(),
        "and the dialog is gone"
    );

    // A second kit of that name is a problem on the field, Create makes nothing, and no line is logged.
    make_kit(&mut app, "cellar");
    assert_eq!(log_at(&root).len(), lines + 1, "a refusal logged a line");
    let problem = app.world_mut().query_filtered::<&FieldProblem, With<NameField>>().iter(app.world()).next().map(|p| p.text.clone());
    assert!(problem.is_some_and(|p| p.contains("already here")), "the dialog says why, beside the name");
}

/// Opens the New kit dialog, types `name` and presses Create.
fn make_kit(app: &mut App, name: &str) {
    app.world_mut().write_message(PageAction::NewKit);
    for _ in 0..3 {
        app.update();
    }
    let field = app.world_mut().query_filtered::<Entity, With<NameField>>().single(app.world()).expect("the dialog has a name field");
    app.world_mut().commands().trigger(ValueChange::<String> { source: field, value: name.to_owned(), is_final: false });
    for _ in 0..3 {
        app.update();
    }
    let create = app.world_mut().query_filtered::<Entity, With<CreateButton>>().single(app.world()).expect("and a Create button");
    app.world_mut().commands().trigger(Activate { entity: create });
    for _ in 0..3 {
        app.update();
    }
}
