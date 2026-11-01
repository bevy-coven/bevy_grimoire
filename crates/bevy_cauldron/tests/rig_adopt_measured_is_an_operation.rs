//! **An agent can adopt a rig's measured gait values into `rigs.ron`, and Enter on the Anim tab is the
//! same operation (E-002, #129 slice 13).**
//!
//! Enter on the animation tab measured the selected rig and rewrote `rigs.ron` with a write of its own,
//! named by no operation: an agent could read a rig's measured values and never adopt them, no log line
//! named the adoption, and the registry's two-front-end rule listed the key (`AdoptMeasured`) as
//! unmigrated. It is now the registered operation `rig_adopt_measured`. The first three tests go through
//! the registered BRP methods, the way an agent's request goes; the last presses Enter in the real tab,
//! the way a person does. All assert on the manifest on disk, what the tab holds and the log.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_cauldron::anim_tab::BenchState;
use bevy_cauldron::ops::{self, Entry};
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;

use crate::fixtures;

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

/// An editor whose project holds the shipped manifest and the valkyrie mesh it measures, with the Anim
/// tab entered so it has read the manifest.
fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("site/a", "alpha").build("m");
    std::fs::copy(workspace_asset("grimoire/rigs.ron"), root.join("assets/grimoire/rigs.ron")).expect("rigs.ron");
    std::fs::create_dir_all(root.join("assets/characters")).expect("characters dir");
    std::fs::copy(workspace_asset("characters/valkyrie.glb"), root.join("assets/characters/valkyrie.glb"))
        .expect("valkyrie.glb");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app.world_mut().insert_resource(Mode::Anim);
    for _ in 0..6 {
        app.update();
    }
    assert!(app.world().resource::<BenchState>().loaded, "the tab loaded the manifest");
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

fn apply(app: &mut App, parameters: serde_json::Value, dry_run: bool) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "rig_adopt_measured", "parameters": parameters, "dry_run": dry_run}),
    )
}

fn manifest(app: &App) -> String {
    std::fs::read_to_string(app.world().resource::<Project>().root.join("assets/grimoire/rigs.ron"))
        .expect("rigs.ron")
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

fn comments(text: &str) -> Vec<String> {
    text.lines().filter(|l| l.trim_start().starts_with("//")).map(str::to_owned).collect()
}

/// **One call adopts the values and stamps the rig, every comment survives, one log line names it, and
/// one undo puts the manifest back byte for byte.**
#[test]
fn an_agent_adopts_a_rigs_measured_values_and_one_undo_takes_them_back() {
    let mut app = editor("rig-adopt-agent");
    let before = manifest(&app);
    let lines_before = log(&app).len();

    let a = apply(&mut app, serde_json::json!({"rig": "valkyrie"}), false);
    assert_eq!(a["outcome"], "applied", "{a}");
    let after = manifest(&app);
    assert_ne!(before, after, "an adoption that changed nothing wrote nothing");
    assert_eq!(comments(&before), comments(&after), "every comment survives the rewrite");
    assert!(after.contains("carries the body LEFTWARD"), "a slot's note died");
    let parsed = bevy_grimoire_core::rigs::Rigs::parse(&after).expect("the written manifest reads");
    let stamp = parsed.get("valkyrie").and_then(|r| r.provenance.as_ref()).expect("provenance was stamped");
    assert_eq!(stamp.clips, 20, "the stamp counts the mesh's clips");

    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one call is one log line");
    let last = lines.last().expect("a line");
    assert_eq!((last.op.as_str(), format!("{:?}", last.source)), ("rig_adopt_measured", "Agent".to_owned()));

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let back = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(back["outcome"], "applied", "the undo was refused: {back}");
    assert_eq!(manifest(&app), before, "the manifest must come back exactly as it was");
}

/// **A preview writes nothing; a second adoption of the same values is refused rather than logged as a
/// change that changed nothing; skipped slots keep their bytes.**
#[test]
fn a_preview_writes_nothing_and_a_second_adoption_changes_nothing() {
    let mut app = editor("rig-adopt-preview");
    let before = manifest(&app);
    let lines_before = log(&app).len();

    let preview = apply(&mut app, serde_json::json!({"rig": "valkyrie"}), true);
    assert_ne!(preview["outcome"], "refused", "{preview}");
    assert_eq!(manifest(&app), before, "a preview wrote rigs.ron");
    assert_eq!(log(&app).len(), lines_before, "a preview logged a line");

    // Skipping every slot leaves nothing to write: the refusal says so and names the rig.
    let slots = bevy_grimoire_core::rigs::Rigs::parse(&before).expect("manifest").get("valkyrie").expect("rig").slots.len();
    let all: Vec<usize> = (0..slots).collect();
    let none = apply(&mut app, serde_json::json!({"rig": "valkyrie", "skip_slots": all}), false);
    assert_eq!(none["outcome"], "refused", "{none}");
    assert_eq!(manifest(&app), before, "a refused adoption wrote rigs.ron");

    assert_eq!(apply(&mut app, serde_json::json!({"rig": "valkyrie"}), false)["outcome"], "applied");
    let adopted = manifest(&app);
    let again = apply(&mut app, serde_json::json!({"rig": "valkyrie"}), false);
    assert_eq!(again["outcome"], "refused", "adopting the values it already holds is not a change: {again}");
    assert!(again.to_string().contains("already holds"), "{again}");
    assert_eq!(manifest(&app), adopted);
    assert_eq!(log(&app).len(), lines_before + 1, "the refusal logged a change");
}

/// **A rig that is not in the manifest, a slot it does not have, a typo'd field and an agent's put-back are
/// refused, and nothing is written.**
#[test]
fn a_bad_request_is_refused_and_writes_nothing() {
    let mut app = editor("rig-adopt-refused");
    let before = manifest(&app);
    let lines_before = log(&app).len();
    for (what, params) in [
        ("an unknown rig", serde_json::json!({"rig": "no-such-rig"})),
        ("a slot the rig lacks", serde_json::json!({"rig": "valkyrie", "skip_slots": [9999]})),
        ("a typo'd field", serde_json::json!({"rig": "valkyrie", "skipp_slots": [1]})),
        ("no rig named", serde_json::json!({})),
        ("a put-back from an agent", serde_json::json!({"rig": "valkyrie", "restore": "(version: 3, rigs: {})"})),
    ] {
        let r = apply(&mut app, params, false);
        assert_eq!(r["outcome"], "refused", "{what} was accepted: {r}");
    }
    assert_eq!(manifest(&app), before, "a refused request wrote rigs.ron");
    assert_eq!(log(&app).len(), lines_before, "a refusal logged a line");
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

/// **A person's Enter on the Anim tab is one `rig_adopt_measured` line stamped as a person's, and the tab
/// holds what the file holds.** Before slice 13 the key wrote `rigs.ron` itself and logged nothing.
#[test]
fn a_persons_enter_on_the_anim_tab_adopts_through_the_operation() {
    let mut app = editor("rig-adopt-person");
    let before = manifest(&app);
    let lines_before = log(&app).len();
    let valkyrie = {
        let bench = app.world().resource::<BenchState>();
        bench.names().iter().position(|n| *n == "valkyrie").expect("the valkyrie rig")
    };
    app.world_mut().resource_mut::<BenchState>().selected = valkyrie;
    app.update();

    tap(&mut app, Key::Enter, KeyCode::Enter);
    for _ in 0..2 {
        app.update();
    }
    let after = manifest(&app);
    assert_ne!(before, after, "Enter did not reach rigs.ron");
    assert_eq!(
        app.world().resource::<BenchState>().text.as_deref(),
        Some(after.as_str()),
        "the tab must hold the manifest the file holds"
    );
    let lines = log(&app);
    assert_eq!(lines.len(), lines_before + 1, "one Enter is one log line");
    let last = lines.last().expect("a line");
    assert_eq!(last.op, "rig_adopt_measured");
    assert_eq!(format!("{:?}", last.source), "Human", "a person's adoption must be stamped as a person's");
}

/// **A bare `operation_undo` takes back the open map's newest change, never a rig adoption.** The adoption
/// is a kit-level write scoped to its own rig, so with no change to the map in effect the bare undo has
/// nothing to take back, and `rigs.ron` stays as the adoption left it until its own id is named.
#[test]
fn a_bare_undo_does_not_revert_a_rig_adoption() {
    let mut app = editor("rig-adopt-bare-undo");
    let before = manifest(&app);
    let a = apply(&mut app, serde_json::json!({"rig": "valkyrie"}), false);
    assert_eq!(a["outcome"], "applied", "{a}");
    let adopted = manifest(&app);

    let bare = call(&mut app, "grimoire/operation_undo", serde_json::json!({"dry_run": false}));
    assert_ne!(bare["outcome"], "applied", "a bare undo reverted the adoption: {bare}");
    assert_eq!(manifest(&app), adopted, "the bare undo wrote rigs.ron");

    let id = a["operation_id"].as_str().expect("an apply returns its id").to_owned();
    let named = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": id, "dry_run": false}));
    assert_eq!(named["outcome"], "applied", "{named}");
    assert_eq!(manifest(&app), before, "the named undo puts the manifest back");
}
