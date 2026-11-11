//! **A build an agent starts is watched in the status band, and opens no page** (#371's review).
//!
//! `game_build` answers `Shown::Job { key }`: the builder's work is a background job, and the band is
//! where a person watches it — on every screen, already. Before this, `Shown::Job` fell through
//! `ops_brp::follow`'s two early returns (`Map`-shaped and `Nowhere`) into `reveal`, whose
//! `page_of` refuses a `Shown` it has no page for: **every build an agent applied raised "game_build
//! changed something the editor has no page for"**, a problem about a thing that was working.
//!
//! # What is being proved
//!
//! 1. **An agent's `game_build` raises no such problem**, and none about a page at all.
//! 2. **The job is still in the band**, which is the half that must not be lost: treating a `Job` like
//!    `Nowhere` is a decision about where the person looks, and the band is where.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::jobs::Jobs;
use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

/// **One frame of the test clock**: a reveal is a queue carried out a frame at a time.
const FRAME: f32 = 1.0 / 60.0;

/// **A stub build that takes its time**, so the job is in the band while the test looks. It says what a
/// real cargo says and takes four seconds over it: the test steps 20 frames while the job runs, and under load (the gate shares big with other builds) 20 debug-build frames can take longer than half a second, which ended the job before the test looked.
const STUB_BUILD: &str = r#"
import json, time
print(json.dumps({"reason": "compiler-artifact", "target": {"name": "the_game", "kind": ["lib"]}, "fresh": False, "filenames": []}), flush=True)
time.sleep(4)
print(json.dumps({"reason": "build-finished", "success": True}), flush=True)
"#;

fn fixtures_python() -> &'static str {
    if std::path::Path::new("/usr/bin/python3").exists() { "/usr/bin/python3" } else { "python3" }
}

/// **A project whose build is the stub**, trusted as a person would have allowed it.
fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join("build_stub.py"), STUB_BUILD).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        root.join("play.ron"),
        format!(r#"(version: 2, command: ["{py}", "-c", "pass"], build: ["{py}", "build_stub.py"])"#, py = fixtures_python()),
    )
    .unwrap_or_else(|e| panic!("{e}"));
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
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// **An agent's apply**, which is the path whose follow is under test.
fn agent_apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        json!({"operation": operation, "parameters": parameters, "dry_run": false}),
    )
}

/// **The problems the status line holds.**
fn problems(app: &App) -> Vec<String> {
    app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect()
}

/// **The person's store for this project**, so the trust gate lets the build run.
fn trusted(app: &mut App, root: &std::path::Path) {
    let store = fixtures::trusted_store(root);
    app.world_mut().resource_mut::<EditorState>().trust.file = Some(store);
}

/// **1 and 2. An agent's build raises no no-page problem, and its job is in the band.**
#[test]
fn a_build_an_agent_starts_is_watched_in_the_band_and_opens_no_page() {
    let root = project(&format!("build_follow_{}", std::process::id()));
    let mut app = editor(&root);
    trusted(&mut app, &root);

    // **The person's own trust store is what the gate reads**, and an agent's context carries it here
    // the way the editor's does for a person.
    let store = app.world().resource::<EditorState>().trust.file.clone();
    let mut ctx = Ctx {
        project: app.world().get_resource::<bevy_cauldron::project::Project>().cloned(),
        open: app.world().get_resource::<bevy_cauldron::project::OpenMap>().cloned(),
        source: ops::Source::Agent,
        session: ops::session_id(),
        trust_file: store,
        ..Ctx::default()
    };
    match ops::dispatch(&mut ctx, "game_build", &json!({})) {
        Ok(Applied::Done { .. }) => {}
        Ok(Applied::Refused(e)) | Err(e) => panic!("the build must start: {}", e.0),
    }

    // **The reveal queue is carried out over the next frames**, so a problem about a page would land
    // in one of them; the whole window is stepped before the answer is read.
    for _ in 0..20 {
        app.update();
        let said = problems(&app);
        assert!(
            !said.iter().any(|p| p.contains("no page")),
            "a build raised the no-page problem, which is about a thing that is working: {said:?}"
        );
    }

    // **And the band still has the job**: not revealing a job must not mean not watching it.
    let jobs = app.world().resource::<Jobs>();
    assert!(
        jobs.snapshot().iter().any(|(key, _)| *key == "game_build"),
        "the build left the band, so the person has nowhere to look: {:?}",
        jobs.snapshot().iter().map(|(key, _)| *key).collect::<Vec<_>>()
    );

    // **And the editor is still running with it**: the job's own end is reported, not a page's.
    let _ = agent_apply(&mut app, "game_build", json!({}));
    let _ = std::fs::remove_dir_all(&root);
}
