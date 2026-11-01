//! **An agent can set where a library piece goes, and `M` on the Meshes tab is the same operation
//! (E-002, #129 slice 6).**
//!
//! `M` stepped a piece's mount with a write of its own, named by no operation, so an agent could label
//! a piece's kind and look but not say where it belongs. It is now the registered operation
//! `piece_set_mount`. The first two tests go through the registered BRP methods, the way an agent's
//! request goes; the third presses the real `M` and Cmd+Z on the Meshes tab, the way a person does.
//! All three assert on the library the editor holds and the file on disk.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_grimoire_core::descriptor::{Axis, By, Mount, mount_options};
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Through the registered method**, as an agent's request goes.
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

fn set_mount(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "piece_set_mount", "parameters": parameters, "dry_run": false}),
    )
}

fn mount_of(app: &App, id: &str) -> Option<Mount> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("`{id}` is not in the live measured library"))
        .mount
        .clone()
}

/// **The library file as it is on disk**, so a change the editor holds but never wrote shows.
fn file(app: &App) -> String {
    std::fs::read_to_string(&app.world().resource::<Project>().library_path)
        .expect("the library file")
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **One call sets the mount, the file holds it, the agent is named as its source, and one undo puts
/// the mount and the label record back.**
#[test]
fn an_agent_mounts_a_piece_the_file_follows_and_one_undo_puts_it_back() {
    let mut app = editor("mount-agent");
    assert_eq!(mount_of(&app, "site/wall"), Some(Mount::OnFloor), "the fixture's mount changed");
    let labels_before = app
        .world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == "site/wall")
        .and_then(|d| d.labels.clone());
    let lines_before = log(&app).len();

    let a = set_mount(&mut app, serde_json::json!({"id": "site/wall", "mount": {"OnWall": {"height": 1.4}}}));
    assert_eq!(a["outcome"], "applied", "the mount was not applied: {a}");
    assert_eq!(
        mount_of(&app, "site/wall"),
        Some(Mount::OnWall { height: 1.4 }),
        "the library the editor holds does not carry the mount (an authored height is free)"
    );
    assert!(
        file(&app).contains("OnWall"),
        "the library file does not hold the mount, so a reopen loses it"
    );
    let by = app
        .world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == "site/wall")
        .and_then(|d| d.labels.as_ref())
        .and_then(|l| l.by.get(Axis::Mount));
    assert_eq!(by, Some(By::Agent), "an agent's mount must be stamped as an agent's, not a person's");
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "a mount is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("piece_set_mount"));

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": id, "dry_run": false}),
    );
    assert_eq!(back["outcome"], "applied", "undo was refused: {back}");
    assert_eq!(mount_of(&app, "site/wall"), Some(Mount::OnFloor), "one undo must put the mount back");
    let reread = file(&app);
    assert!(
        !reread.contains("OnWall") && reread.contains("mount: Some(OnFloor)"),
        "undo must put the library file back: {reread}"
    );
    let labels_after = app
        .world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == "site/wall")
        .and_then(|d| d.labels.clone());
    assert_eq!(labels_after, labels_before, "undo must put the label record back with the mount");
}

/// **A mount the project does not offer, a height that is not a length, an unknown piece, a typo'd
/// field and an agent's own put-back are refused, and nothing is written.**
#[test]
fn an_unoffered_mount_a_bad_height_an_unknown_piece_and_a_forged_restore_are_refused() {
    let mut app = editor("mount-refused");
    let before = file(&app);

    let unoffered = set_mount(
        &mut app,
        serde_json::json!({"id": "site/wall", "mount": {"OnSurface": {"class": "no_such_class"}}}),
    );
    assert_eq!(unoffered["outcome"], "refused", "a mount on a surface class nothing declares was applied: {unoffered}");
    assert!(
        unoffered.to_string().contains("OnFloor"),
        "the refusal does not list what the project offers, so an agent cannot correct itself: {unoffered}"
    );
    let height = set_mount(&mut app, serde_json::json!({"id": "site/wall", "mount": {"OnWall": {"height": -2.0}}}));
    assert_eq!(height["outcome"], "refused", "a negative wall height was applied: {height}");
    let unknown = set_mount(&mut app, serde_json::json!({"id": "site/wal", "mount": "OnCeiling"}));
    assert_eq!(unknown["outcome"], "refused", "a typo in the id was applied: {unknown}");
    assert!(
        unknown.to_string().contains("site/wall"),
        "the refusal does not name the closest piece: {unknown}"
    );
    let field = set_mount(&mut app, serde_json::json!({"id": "site/wall", "mount": "OnCeiling", "height": 1.0}));
    assert_eq!(field["outcome"], "refused", "an unknown field was applied: {field}");
    let forged = set_mount(
        &mut app,
        serde_json::json!({"id": "site/wall", "restore": {"mount": "OnCeiling", "labels": null}}),
    );
    assert_eq!(forged["outcome"], "refused", "an agent sent an undo's put-back as a change: {forged}");

    assert_eq!(mount_of(&app, "site/wall"), Some(Mount::OnFloor), "a refusal moved the mount");
    assert_eq!(file(&app), before, "a refusal wrote the library file");

    // **Zero is a legal wall height**, as the Meshes height field allows (a marking at skirting level).
    let skirting = set_mount(&mut app, serde_json::json!({"id": "site/wall", "mount": {"OnWall": {"height": 0.0}}}));
    assert_eq!(skirting["outcome"], "applied", "a wall height of zero was refused: {skirting}");
    assert_eq!(mount_of(&app, "site/wall"), Some(Mount::OnWall { height: 0.0 }));
    let ceiling = set_mount(&mut app, serde_json::json!({"id": "site/wall", "mount": "OnCeiling"}));
    assert_eq!(ceiling["outcome"], "applied", "a mount the project offers was refused: {ceiling}");
    assert_eq!(mount_of(&app, "site/wall"), Some(Mount::OnCeiling));
}

/// **A bare undo takes back the open map's newest change, never a piece's mount.** A mount is a
/// library change with no map, so `operation_undo` with no id has to scope past it, as it does for
/// `label_set`; otherwise it reverts a mount and leaves the turn it was asked for.
#[test]
fn a_bare_undo_takes_back_the_maps_turn_and_leaves_the_mount() {
    let root = fixtures::Fixture::new("mount-bare-undo")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    let placement = app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements[0]
        .id
        .clone();
    let turned = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_set_yaw",
            "parameters": {"id": placement, "yaw": 110.0}, "dry_run": false}),
    );
    assert_eq!(turned["outcome"], "applied", "{turned}");
    let mounted = set_mount(&mut app, serde_json::json!({"id": "site/wall", "mount": "OnCeiling"}));
    assert_eq!(mounted["outcome"], "applied", "{mounted}");

    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"dry_run": false}));
    assert_eq!(back["outcome"], "applied", "a bare undo was refused: {back}");
    assert_eq!(
        app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements[0].yaw,
        0.0,
        "a bare undo must take back the open map's newest change, the turn"
    );
    assert_eq!(
        mount_of(&app, "site/wall"),
        Some(Mount::OnCeiling),
        "a bare undo took back a library change instead of the map's"
    );
}

fn tap(app: &mut App, logical: Key, code: KeyCode) {
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **A person's `M` on a library piece is one `piece_set_mount` line stamped as a person's, it
/// reaches the file, and one Cmd+Z takes it back.** Before slice 6 `M` wrote the library with its own
/// call and no log line named it. A rung the project would refuse is stepped past without a refusal
/// in the log.
#[test]
fn a_persons_m_steps_the_mount_through_the_operation_and_cmd_z_takes_it_back() {
    let mut app = editor("mount-person");
    assert!(file(&app).contains("mount: Some(OnFloor)"), "the fixture's mount changed");
    // **A click on the shelf row**, which is `piece_select` and puts the pane on the piece.
    app.world_mut()
        .resource_scope(|world, mut import: Mut<ImportState>| {
            let project = world.resource::<Project>();
            bevy_cauldron::editor::select_piece(&mut import, project, "site/wall");
        });
    // **Frames for the pane to build**, as a person's eyes would take.
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<ImportState>().selected_library_id.as_deref(),
        Some("site/wall"),
        "a shelf click did not put the pane on the piece"
    );
    let lines_before = log(&app).len();

    let surfaces: Vec<String> = app
        .world()
        .resource::<Project>()
        .vocab
        .surfaces
        .names()
        .map(str::to_owned)
        .collect();
    // **The next rung the project can take.** The fixture declares surface classes and no piece offers
    // one, so the library refuses every rung that rests on a class; `M` steps past those instead of
    // pressing against the first, and lands on the first rung that needs no class.
    let want = mount_options(&surfaces)
        .into_iter()
        .skip(1)
        .find(|o| !matches!(o, Mount::OnSurface { .. } | Mount::OnSurfaceOrFloor { .. } | Mount::OnFace { .. }))
        .expect("the list has a rung that needs no surface class");
    assert!(!surfaces.is_empty(), "the fixture's vocabulary declares no surface class, so the skip is not exercised");

    tap(&mut app, Key::Character("m".into()), KeyCode::KeyM);
    app.update();
    assert_eq!(
        mount_of(&app, "site/wall"),
        Some(want),
        "M must step a piece on the floor to the next mount the project can take"
    );
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one press of M is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("piece_set_mount"));
    assert!(file(&app).contains("OnWall"), "M did not reach the library file");

    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    tap(
        &mut app,
        Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        KeyCode::KeyZ,
    );
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    app.update();
    assert_eq!(mount_of(&app, "site/wall"), Some(Mount::OnFloor), "one Cmd+Z must take the mount back");
    assert!(
        file(&app).contains("mount: Some(OnFloor)") && !file(&app).contains("OnWall"),
        "Cmd+Z must put the library file back"
    );
}
