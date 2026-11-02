//! **The wall-height field on the Meshes tab sets the mount through the operation an agent calls
//! (E-002, #129 slice 16).**
//!
//! A typed height rewrote the piece's mount and the library with a write of its own (`mount_height_keys`),
//! so the same change an agent makes with `piece_set_mount {"OnWall": {"height": h}}` was, for a person,
//! a change no log line named and the operation's refusals did not guard. It now dispatches the operation.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{HeightEdit, ImportState, Mode};
use bevy_grimoire_core::descriptor::{Axis, By, Mount};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn mount_of(app: &App, id: &str) -> Option<Mount> {
    app.world().resource::<Project>().measured.descriptors.iter().find(|d| d.id == id).expect("the piece").mount.clone()
}

fn by_of(app: &App, id: &str) -> Option<By> {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .and_then(|d| d.labels.as_ref())
        .and_then(|l| l.by.get(Axis::Mount))
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
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

fn select_and_mount(app: &mut App) {
    let a = call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "piece_set_mount", "parameters": {"id": "site/wall", "mount": {"OnWall": {"height": 1.4}}}, "dry_run": false}),
    );
    assert_eq!(a["outcome"], "applied", "{a}");
    app.world_mut().resource_scope(|world, mut import: Mut<ImportState>| {
        let project = world.resource::<Project>();
        bevy_cauldron::editor::select_piece(&mut import, project, "site/wall");
    });
    for _ in 0..4 {
        app.update();
    }
}

/// **A person's typed height is one `piece_set_mount` line stamped as a person's**, reaches the file,
/// and one Cmd+Z puts the old height back.
#[test]
fn a_typed_wall_height_is_one_logged_mount_change_and_cmd_z_takes_it_back() {
    let mut app = editor("height-person");
    select_and_mount(&mut app);
    let lines_before = log(&app).len();
    app.world_mut().resource_mut::<HeightEdit>().open_for_test("site/wall");
    app.update();

    for (ch, code) in [('2', KeyCode::Digit2), ('.', KeyCode::Period), ('1', KeyCode::Digit1)] {
        tap(&mut app, Key::Character(ch.to_string().into()), code);
    }
    tap(&mut app, Key::Enter, KeyCode::Enter);
    app.update();

    assert_eq!(mount_of(&app, "site/wall"), Some(Mount::OnWall { height: 2.1 }), "Enter did not set the height");
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one Enter is one log line");
    let last = lines.last().expect("a line");
    assert_eq!(last.op, "piece_set_mount");
    assert_eq!(format!("{:?}", last.source), "Human", "a person's height must be stamped as a person's");
    assert_eq!(by_of(&app, "site/wall"), Some(By::Human));
    assert!(
        std::fs::read_to_string(&app.world().resource::<Project>().library_path).expect("library").contains("2.1"),
        "the library file does not hold the height"
    );

    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    tap(&mut app, Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified), KeyCode::KeyZ);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    app.update();
    assert_eq!(mount_of(&app, "site/wall"), Some(Mount::OnWall { height: 1.4 }), "one Cmd+Z must take the height back");
}
