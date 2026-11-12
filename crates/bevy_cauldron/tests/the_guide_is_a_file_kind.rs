//! **The agent's guide is a file kind, and `grimoire_state.reloads` says what became of it** (#347, AG-02).
//!
//! `.dark_factory/AGENTS.md` is a file of the project, read by the editor and served to an agent, so it is
//! one row of `bevy_grimoire_core::reload::KINDS`: `Root`, `Live`, edited by hand, like `agent.ron`. The
//! registry is what a caller asks whether a run picks a file up, and a file the editor reads on every
//! `grimoire/agent_guide` call has to be in it — otherwise "does a reload reach this?" has an answer for
//! every file but the one a person is most likely to hand-edit while an agent works.
//!
//! **And the editor decides it.** A kind in the registry that no watcher reads is a claim about a reload
//! nothing makes; `grimoire_state.reloads` was the place a person looked to know whether their edit
//! landed, and the guide's refusal (a file that is not UTF-8, one over `GUIDE_MAX_BYTES`, one that is not
//! a regular file) is exactly what that list is for.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::ops_brp::OpsBridgePlugin;
use bevy_cauldron::project::Project;
use bevy_grimoire_core::agent_guide;
use bevy_grimoire_core::reload::{self, Base, Lifetime, ReloadClass};
use serde_json::{Value, json};

use crate::fixtures;

/// **The guide is one row of the reload registry**: the game root's own file, live, one per project.
#[test]
fn the_agents_guide_is_a_kind_of_the_reload_contract() {
    let kind = reload::kind_by_id("guide")
        .unwrap_or_else(|| panic!("`guide` is no kind of bevy_grimoire_core::reload::KINDS, so nothing says whether an edit reaches the agent"));
    assert_eq!(kind.base, Base::Root, "`.dark_factory/AGENTS.md` hangs off the game's root, not the grimoire directory");
    assert_eq!(kind.pattern, agent_guide::PROJECT_PATH, "the row's pattern is the path the reader opens");
    assert_eq!(kind.class, ReloadClass::Live, "both files are read on each call, so an edit is already live");
    assert_eq!(kind.lifetime, Lifetime::Global, "one guide per project, not one per map");
    assert_ne!(kind.other_writers, "", "`by hand` is who writes it: no operation may write a person's guide");
    assert_eq!(
        reload::kind_of(Base::Root, agent_guide::PROJECT_PATH).map(|k| k.id),
        Some("guide"),
        "a changed `.dark_factory/AGENTS.md` is that row and no other"
    );
}

/// The editor's own verdicts, as `grimoire/state` answers.
fn reloads(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    let state = app
        .world_mut()
        .run_system_with(id, Some(json!({})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    state["reloads"].clone()
}

fn guide_verdict(app: &mut App) -> Value {
    reloads(app)["kinds"]
        .as_array()
        .and_then(|k| k.iter().find(|v| v["kind"] == "guide").cloned())
        .unwrap_or(Value::Null)
}

fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 8.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
    panic!("never saw: {what}; reloads {}", reloads(app));
}

/// **A guide that does not read is refused by name, and the fix is applied** — the two verdicts a person
/// reads on the Reload panel for any other kind.
#[test]
fn a_guide_that_does_not_read_is_refused_in_the_reloads_list_and_the_fix_is_applied() {
    let root = fixtures::Fixture::new(&format!("guide_kind_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(OpsBridgePlugin);
    for _ in 0..6 {
        app.update();
    }
    let project = app.world().resource::<Project>().root.clone();
    let path = project.join(agent_guide::PROJECT_PATH);
    std::fs::create_dir_all(path.parent().expect("the guide has a folder")).expect("the folder is made");

    // **A guide that is not UTF-8**, which `read_layer` refuses rather than decoding lossily.
    std::fs::write(&path, [0x23, 0x20, 0xff]).expect("a guide that is not text is written");
    until(&mut app, "the guide is refused", |app| guide_verdict(app)["outcome"] == "refused");
    let refused = guide_verdict(&mut app);
    assert_eq!(refused["file"], json!(agent_guide::PROJECT_PATH), "the verdict names the file a person edits: {refused}");
    assert!(
        refused["reason"].as_str().is_some_and(|r| r.contains("UTF-8")),
        "and says why it was left out: {refused}"
    );

    // **The same file, made good**, is applied: a person fixes the file and the list says so.
    std::thread::sleep(Duration::from_millis(30));
    std::fs::write(&path, "# Walls\n\nWalls are 2.40 m.\n").expect("a guide that reads is written");
    until(&mut app, "the fixed guide is applied", |app| guide_verdict(app)["outcome"] == "applied");
    assert_eq!(guide_verdict(&mut app)["file"], json!(agent_guide::PROJECT_PATH));

    let _ = std::fs::remove_dir_all(&root);
}
