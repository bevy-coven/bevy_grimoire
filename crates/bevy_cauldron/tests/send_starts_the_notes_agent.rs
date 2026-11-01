//! **Send starts the project's notes agent, once, and says when it cannot** (#94).

use bevy::prelude::*;
use bevy_cauldron::editor::{EditorState, send_notes};
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::json;
use std::path::{Path, PathBuf};

use crate::fixtures;

struct Rig {
    app: App,
    root: PathBuf,
}

fn rig(tag: &str) -> Rig {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    Rig { app, root }
}

impl Rig {
    /// A person leaves a draft, as the note box does.
    fn draft(&mut self, text: &str) {
        let mut ctx = Ctx {
            project: Some(self.app.world().resource::<Project>().clone()),
            open: Some(self.app.world().resource::<OpenMap>().clone()),
            session: "test".to_owned(),
            ..Ctx::default()
        };
        match ops::dispatch(&mut ctx, "note_add", &json!({"cells": [0, 0, 1, 1], "text": text})).unwrap() {
            Applied::Done { .. } => {}
            Applied::Refused(e) => panic!("refused: {}", e.0),
        }
        *self.app.world_mut().resource_mut::<OpenMap>() = ctx.open.take().unwrap();
    }

    /// Presses Send, and returns the status line's problem text.
    fn send(&mut self) -> String {
        let mut project = self.app.world().resource::<Project>().clone();
        let mut open = self.app.world().resource::<OpenMap>().clone();
        let mut state = self.app.world_mut().remove_resource::<EditorState>().unwrap();
        send_notes(Some(&mut project), &mut open, &mut state);
        let problem = state.status.problem_text().to_owned();
        self.app.world_mut().insert_resource(state);
        *self.app.world_mut().resource_mut::<OpenMap>() = open;
        problem
    }
}

/// Writes a script that appends one line per start, and `agent.ron` naming it.
fn agent(root: &Path, script_body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let script = root.join("fake-agent.sh");
    let log = root.join("starts.log");
    std::fs::write(&script, format!("#!/bin/sh\necho \"$@\" >> '{}'\n{script_body}\n", log.display())).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        root.join("agent.ron"),
        format!("(version: 1, command: Some(\"{}\"), args: [\"--model\", \"{{model}}\"], model: \"test-model\")", script.display()),
    )
    .unwrap();
    log
}

fn wait_for(log: &Path) -> String {
    for _ in 0..100 {
        if let Ok(t) = std::fs::read_to_string(log)
            && !t.is_empty()
        {
            std::thread::sleep(std::time::Duration::from_millis(200));
            return std::fs::read_to_string(log).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    String::new()
}

#[test]
fn with_no_agent_set_send_starts_nothing() {
    let mut r = rig("agent_unset");
    let log = r.root.join("starts.log");
    r.draft("one");
    assert_eq!(r.send(), "", "no agent.ron is not a problem");
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!log.exists());

    std::fs::write(r.root.join("agent.ron"), "(version: 1)").unwrap();
    r.draft("two");
    assert_eq!(r.send(), "", "a file without a command is the same");
}

#[test]
fn with_an_agent_set_each_send_starts_it_exactly_once() {
    let mut r = rig("agent_set");
    let log = agent(&r.root, "");
    r.draft("one");
    r.draft("two");
    assert_eq!(r.send(), "");
    assert_eq!(wait_for(&log), "--model test-model\n", "one start for two drafts, with the model filled in");

    r.draft("three");
    r.send();
    for _ in 0..40 {
        if std::fs::read_to_string(&log).unwrap().lines().count() == 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(std::fs::read_to_string(&log).unwrap().lines().count(), 2, "the second Send starts it once more");
}

#[test]
fn a_send_with_nothing_to_send_starts_nothing() {
    let mut r = rig("agent_nothing");
    let log = agent(&r.root, "");
    r.send();
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!log.exists(), "a refused send is not a launch");
}

#[test]
fn an_agent_that_cannot_start_is_a_problem_and_the_notes_stay_sent() {
    let mut r = rig("agent_broken");
    std::fs::write(r.root.join("agent.ron"), "(version: 1, command: Some(\"/nonexistent/agent\"))").unwrap();
    r.draft("one");
    let problem = r.send();
    assert!(problem.contains("could not start the notes agent") && problem.contains("/nonexistent/agent"), "{problem}");
    let notes = bevy_grimoire_core::notes::Notes::load(&bevy_grimoire_core::notes::path_for_map(&r.app.world().resource::<OpenMap>().map_path)).unwrap();
    assert!(notes.notes.iter().all(|n| !n.is_draft()));

    std::fs::write(r.root.join("agent.ron"), "(version: 9)").unwrap();
    r.draft("two");
    assert!(r.send().contains("version 9"), "an unreadable setting is reported too");
}

/// **The MCP docs' notes guide runs.** Its fenced `json` example is read out of `docs/mcp-tools.md`
/// and every line is dispatched against a headless editor, the agent's as the agent, so the guide
/// cannot describe operations that no longer work.
#[test]
fn the_guides_worked_example_runs_against_a_headless_editor() {
    let docs = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/mcp-tools.md")).unwrap();
    let guide = docs.split("## Working a note").nth(1).expect("the docs carry the notes guide");
    let block = guide.split("```json\n").nth(1).and_then(|b| b.split("```").next()).expect("the guide has a json example");
    let mut r = rig("agent_guide");
    let mut ctx = Ctx {
        project: Some(r.app.world().resource::<Project>().clone()),
        open: Some(r.app.world().resource::<OpenMap>().clone()),
        session: "guide".to_owned(),
        ..Ctx::default()
    };
    let lines: Vec<serde_json::Value> = block.lines().map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}"))).collect();
    assert!(lines.len() >= 5);
    for line in &lines {
        ctx.source = if line["parameters"]["agent"].is_string() || line["agent"].is_string() {
            ops::Source::Agent
        } else {
            ops::Source::Human
        };
        let op = line["operation"].as_str().unwrap();
        match ops::dispatch(&mut ctx, op, &line["parameters"]).unwrap_or_else(|e| panic!("{op}: {}", e.0)) {
            Applied::Done { .. } => {}
            Applied::Refused(e) => panic!("{op} refused: {}", e.0),
        }
    }
    let notes = bevy_grimoire_core::notes::Notes::load(&bevy_grimoire_core::notes::path_for_map(&ctx.open.as_ref().unwrap().map_path)).unwrap();
    assert!(matches!(notes.notes[0].state, bevy_grimoire_core::notes::State::Resolved { .. }), "{:?}", notes.notes[0].state);
    let _ = &mut r;
}
