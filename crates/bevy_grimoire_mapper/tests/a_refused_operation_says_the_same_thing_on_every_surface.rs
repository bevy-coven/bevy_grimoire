//! **A refused operation says the same `what`, `where` and `fix` to the toast, the journal row and
//! the agent (UI-05, #213).**
//!
//! A refusal reached a person as a string a key handler formatted, an agent as a string the tool
//! answer carried and the journal as a third, so a part one of them named was missing from the next.
//! `bevy_grimoire_core::problem::Problem` is the facts once: the toast and the row are printed from
//! it and the agent's answer is its JSON.

use bevy::prelude::*;
use bevy_grimoire_core::live::KindVerdict;
use bevy_grimoire_mapper::chrome::Journal;
use bevy_grimoire_mapper::editor::EditorState;
use bevy_grimoire_mapper::harness;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_grimoire_mapper::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

#[test]
fn a_refused_removal_names_what_where_and_the_fix_in_the_answer_the_toast_and_the_journal() {
    let mut app = editor("one_problem");
    let reply = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_remove", "parameters": {"id": "ghost"}, "dry_run": false}),
    );
    assert_eq!(reply["outcome"], "refused", "{reply}");
    let answer = &reply["problem"];
    assert_eq!(answer["what"], reply["refusal"], "`refusal` is the problem's `what`: {reply}");
    assert_eq!(answer["where"]["entity"], "ghost", "the answer does not say where: {reply}");
    assert!(
        answer["fix"].as_str().is_some_and(|f| f.contains("name a placement that is on the map")),
        "the answer has no fix: {reply}"
    );

    // The toast shows the status line, and the row it was raised as keeps the problem it was printed from.
    for _ in 0..3 {
        app.update();
    }
    let state = app.world().resource::<EditorState>();
    let row = state.status.problems().last().expect("the refusal is on the tab's log");
    let kept = row.problem.as_ref().expect("the row keeps the structured problem");
    assert_eq!(&kept.to_json(), answer, "the row and the answer are not one problem");
    let toast = state.status.problem_text().to_owned();
    assert_eq!(toast, format!("agent: placement_remove: {}", kept.line()), "the toast is not printed from it");
    for part in ["no placement is called `ghost`", "`ghost`", "name a placement that is on the map"] {
        assert!(toast.contains(part), "the toast leaves out `{part}`: {toast}");
    }

    // And the session journal, which `Cmd+E` opens.
    let journal = app.world().resource::<Journal>();
    let entry = journal.entries().last().expect("the refusal reached the journal");
    assert_eq!(entry.text, toast, "the journal row reads like the toast");
    assert_eq!(entry.problem.as_ref().map(|p| p.to_json()).as_ref(), Some(answer), "the journal lost a part");
}

#[test]
fn a_preview_of_the_same_refusal_answers_with_the_same_problem() {
    let mut app = editor("one_problem_preview");
    let params = serde_json::json!({"operation": "placement_remove", "parameters": {"id": "ghost"}});
    let preview = call(&mut app, "grimoire/operation_preview", params);
    let applied = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_remove", "parameters": {"id": "ghost"}, "dry_run": false}),
    );
    assert_eq!(preview["problem"], applied["problem"], "a preview and an apply refuse differently");
}

#[test]
fn a_refusal_that_is_only_a_sentence_says_only_its_what() {
    let mut app = editor("one_problem_bare");
    let reply = call(
        &mut app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "placement_remove", "parameters": {}, "dry_run": false}),
    );
    assert_eq!(reply["outcome"], "refused", "{reply}");
    assert_eq!(reply["problem"], serde_json::json!({"what": reply["refusal"]}), "{reply}");
}

#[test]
fn a_refused_file_names_its_file_field_reason_and_what_the_editor_kept() {
    let verdict = KindVerdict::refused("rigs", "rigs.ron", Some("rigs[2]".to_owned()), "1:47: Expected float");
    let p = verdict.problem("the rigs already loaded stay");
    assert_eq!(
        p.to_json(),
        serde_json::json!({
            "what": "rigs.ron did not read, so the rigs already loaded stay",
            "where": {"file": "rigs.ron", "field": "rigs[2]"},
            "why": "1:47: Expected float",
            "fix": "correct rigs.ron and save it; the editor reads it again at once",
        })
    );
    // The journal's old sentence is still in the line, so anyone searching it for the field finds it.
    assert!(p.line().contains("rigs.ron (rigs[2]): 1:47: Expected float"), "{}", p.line());
}
