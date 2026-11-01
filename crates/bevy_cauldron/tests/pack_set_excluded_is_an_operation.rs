//! **An agent can leave a pack out of the kit, and `Shift+R` on the Meshes tab is the same operation
//! (E-002, #129 slice 7).**
//!
//! `Shift+R` spliced the kit's `project.ron` with a write of its own, named by no operation, so an
//! agent could label a piece and not say that a folder of art is not what the kit is built from, no log
//! line named the change and Cmd+Z could not take it back. It is now the registered operation
//! `pack_set_excluded`. The first three tests go through the registered BRP methods, the way an
//! agent's request goes; the fourth presses the real `Shift+R` and Cmd+Z on the Meshes tab, the way a
//! person does. All assert on the policy the editor holds and the file on disk.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::{ImportState, Mode};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .pack("aaa", &["a1", "a2"])
        .pack("bbb", &["b1", "b2"])
        .pack("ccc/inner", &["c1"])
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

fn set_excluded(app: &mut App, pack: &str, excluded: bool) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "pack_set_excluded",
            "parameters": {"pack": pack, "excluded": excluded},
            "dry_run": false,
        }),
    )
}

fn excluded(app: &App) -> Vec<String> {
    app.world().resource::<Project>().policy.exclude.clone()
}

/// **The policy file as it is on disk**, so a change the editor holds but never wrote shows.
fn file(app: &App) -> String {
    std::fs::read_to_string(
        app.world()
            .resource::<Project>()
            .grimoire_dir
            .join(bevy_grimoire_core::policy::POLICY_FILE),
    )
    .expect("the policy file")
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **One call excludes the pack, the file and the editor's policy hold it, one log line names it, and
/// one undo brings it back.** A pack inside another folder is a pack too.
#[test]
fn an_agent_excludes_a_pack_the_file_follows_and_one_undo_brings_it_back() {
    let mut app = editor("pack-agent");
    assert!(excluded(&app).is_empty(), "the fixture excludes nothing to begin with");
    let lines_before = log(&app).len();

    let a = set_excluded(&mut app, "bbb", true);
    assert_eq!(a["outcome"], "applied", "the exclusion was not applied: {a}");
    assert_eq!(excluded(&app), vec!["bbb".to_owned()], "the editor's policy does not hold the exclusion");
    assert!(
        app.world().resource::<Project>().policy.excludes("bbb/b1.glb"),
        "the pack's meshes are still offered"
    );
    assert!(file(&app).contains("\"bbb\""), "the policy file does not hold the exclusion: {}", file(&app));
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "an exclusion is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("pack_set_excluded"));

    let nested = set_excluded(&mut app, "ccc/inner/", true);
    assert_eq!(nested["outcome"], "applied", "a nested pack was refused: {nested}");
    assert_eq!(
        excluded(&app),
        vec!["bbb".to_owned(), "ccc/inner".to_owned()],
        "the list is kept sorted and without the trailing slash"
    );

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let blocked = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(
        blocked["outcome"], "applied",
        "another pack's exclusion must not block this one's undo: {blocked}"
    );
    assert_eq!(excluded(&app), vec!["ccc/inner".to_owned()], "one undo must bring `bbb` back and only it");
    assert!(
        !file(&app).contains("\"bbb\"") && file(&app).contains("\"ccc/inner\""),
        "undo must put the policy file back: {}",
        file(&app)
    );

    let restored = set_excluded(&mut app, "ccc/inner", false);
    assert_eq!(restored["outcome"], "applied", "a restore was refused: {restored}");
    assert!(excluded(&app).is_empty(), "a restore must bring the pack back");
    assert!(file(&app).contains("exclude: []"), "the file still lists the pack: {}", file(&app));
}

/// **A name that is not a pack, a folder that does not exist, an exclusion already in force, a restore
/// of a pack nobody excluded and a typo'd field are refused, and nothing is written.**
#[test]
fn a_bad_pack_a_missing_folder_a_repeat_and_a_typo_are_refused() {
    let mut app = editor("pack-refused");
    let before = file(&app);
    let lines_before = log(&app).len();

    for pack in ["", "/etc", "../aaa", "aaa/../bbb", "aaa//a1", ".", "bad\"quote", "back\\slash"] {
        let r = set_excluded(&mut app, pack, true);
        assert_eq!(r["outcome"], "refused", "`{pack}` is not a pack and was applied: {r}");
        // **The name's own guard, not the missing folder's**: a quote or a backslash is refused as a
        // name, because `rewrite_exclude` writes a string unescaped.
        assert!(
            r.to_string().contains("is not a pack"),
            "`{pack}` was refused for another reason, so the guard on the name is untested: {r}"
        );
    }
    let missing = set_excluded(&mut app, "no_such_pack", true);
    assert_eq!(missing["outcome"], "refused", "a folder that does not exist was excluded: {missing}");
    assert!(
        missing.to_string().contains("no_such_pack"),
        "the refusal does not name the pack it could not find: {missing}"
    );
    let offered = set_excluded(&mut app, "aaa", false);
    assert_eq!(offered["outcome"], "refused", "restoring a pack nobody excluded was applied: {offered}");
    let field = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "pack_set_excluded",
            "parameters": {"pack": "aaa", "excluded": true, "folder": "aaa"},
            "dry_run": false,
        }),
    );
    assert_eq!(field["outcome"], "refused", "an unknown field was applied: {field}");
    let absent = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "pack_set_excluded", "parameters": {"pack": "aaa"}, "dry_run": false}),
    );
    assert_eq!(absent["outcome"], "refused", "a call that did not say which way was applied: {absent}");
    assert_eq!(file(&app), before, "a refusal wrote the policy file");
    assert!(excluded(&app).is_empty(), "a refusal moved the editor's policy");
    assert_eq!(log(&app).len(), lines_before, "a refusal logged a change");

    let first = set_excluded(&mut app, "aaa", true);
    assert_eq!(first["outcome"], "applied", "{first}");
    let again = set_excluded(&mut app, "aaa/", true);
    assert_eq!(again["outcome"], "refused", "excluding a pack twice was applied: {again}");
    assert_eq!(excluded(&app), vec!["aaa".to_owned()], "a repeat changed the list");

    // **A folder above it already leaves it out**, so excluding the inner pack is no change.
    let parent = set_excluded(&mut app, "ccc", true);
    assert_eq!(parent["outcome"], "applied", "{parent}");
    let inner = set_excluded(&mut app, "ccc/inner", true);
    assert_eq!(inner["outcome"], "refused", "a pack its parent already excludes was logged as a change: {inner}");
    assert!(inner.to_string().contains("`ccc`"), "the refusal does not name the folder that covers it: {inner}");
    assert_eq!(excluded(&app), vec!["aaa".to_owned(), "ccc".to_owned()], "the refused inner pack was listed");

    // **A put-back is an undo's form**, never an agent's change.
    let forged = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "pack_set_excluded",
            "parameters": {"pack": "no_such_pack", "excluded": true, "put_back": true},
            "dry_run": false,
        }),
    );
    assert_eq!(forged["outcome"], "refused", "an agent sent an undo's put-back as a change: {forged}");
    assert_eq!(excluded(&app), vec!["aaa".to_owned(), "ccc".to_owned()], "a forged put-back was listed");
}

/// **A restore is always undoable, even when the folder has gone since.** The undo of a restore lists
/// the pack again, and asking the disk for the folder there would refuse the one Cmd+Z the author
/// expects to work.
#[test]
fn a_restore_is_undone_even_when_its_folder_is_gone() {
    let mut app = editor("pack-put-back");
    let on = set_excluded(&mut app, "bbb", true);
    assert_eq!(on["outcome"], "applied", "{on}");
    let off = set_excluded(&mut app, "bbb", false);
    assert_eq!(off["outcome"], "applied", "{off}");
    assert!(excluded(&app).is_empty(), "the restore did not take the pack off the list");
    let folder = app.world().resource::<Project>().root.join("assets").join("bbb");
    std::fs::remove_dir_all(&folder).expect("remove the pack's folder");

    let id = off["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "applied", "the undo of a restore was refused because the folder is gone: {back}");
    assert_eq!(excluded(&app), vec!["bbb".to_owned()], "the undo did not list the pack again");
    assert!(file(&app).contains("\"bbb\""), "the policy file does not list the pack again: {}", file(&app));
}

/// **A bare undo takes back the open map's newest change, never a pack's exclusion.** An exclusion is
/// a kit change with no map, so `operation_undo` with no id has to scope past it, as it does for a
/// library write; otherwise it reverts an exclusion and leaves the turn it was asked for.
#[test]
fn a_bare_undo_takes_back_the_maps_turn_and_leaves_the_exclusion() {
    let root = fixtures::Fixture::new("pack-bare-undo")
        .descriptor("site/wall", "alpha")
        .pack("bbb", &["b1"])
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
    let out = set_excluded(&mut app, "bbb", true);
    assert_eq!(out["outcome"], "applied", "{out}");

    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"dry_run": false}));
    assert_eq!(back["outcome"], "applied", "a bare undo was refused: {back}");
    assert_eq!(
        app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements[0].yaw,
        0.0,
        "a bare undo must take back the open map's newest change, the turn"
    );
    assert_eq!(excluded(&app), vec!["bbb".to_owned()], "a bare undo took back a pack's exclusion instead of the map's");
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

/// **A person's `Shift+R` on a pack is one `pack_set_excluded` line stamped as a person's, it reaches
/// the file, a second press brings the pack back, and Cmd+Z takes the newest press back.** Before
/// slice 7 the key wrote `project.ron` with its own splice, logged nothing and had no undo.
#[test]
fn a_persons_shift_r_excludes_through_the_operation_and_cmd_z_takes_it_back() {
    let mut app = editor("pack-person");
    // **The cursor on `bbb`'s heading**, where the audit's M2 put it: the key acts on the pack the
    // highlight is on.
    app.world_mut().resource_mut::<ImportState>().focused_pack = Some("bbb".to_owned());
    app.update();
    let lines_before = log(&app).len();

    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ShiftLeft);
    tap(&mut app, Key::Character("R".into()), KeyCode::KeyR);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(KeyCode::ShiftLeft);
    app.update();
    assert_eq!(excluded(&app), vec!["bbb".to_owned()], "Shift+R must exclude the pack under the cursor");
    assert!(file(&app).contains("\"bbb\""), "Shift+R did not reach the policy file: {}", file(&app));
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one press of Shift+R is one log line");
    assert_eq!(lines.last().map(|l| l.op.as_str()), Some("pack_set_excluded"));
    assert_eq!(
        lines.last().map(|l| format!("{:?}", l.source)),
        Some("Human".to_owned()),
        "a person's exclusion must be stamped as a person's"
    );

    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    tap(
        &mut app,
        Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        KeyCode::KeyZ,
    );
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    app.update();
    assert!(excluded(&app).is_empty(), "one Cmd+Z must bring the pack back");
    assert!(
        !file(&app).contains("\"bbb\""),
        "Cmd+Z must put the policy file back: {}",
        file(&app)
    );
}
