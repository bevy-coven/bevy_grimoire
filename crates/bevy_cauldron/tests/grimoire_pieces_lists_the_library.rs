//! **`grimoire/pieces` lists the library's pieces, a page at a time.**
//!
//! `grimoire/state` names the first page of library ids and says where the next page starts, but an
//! agent that wants the pieces themselves (the mesh each one names, its kinds, effects and looks,
//! and who labelled each of them) has to read them one by one. `grimoire/pieces` is that list: the
//! same paging as the state's `descriptor_ids`, each row carrying the fields a piece is chosen by
//! and its label provenance, and filters that narrow the list without reading every row.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

/// The most rows one page of `grimoire/pieces` names. Matches the cap written into the answer.
const PIECE_CAP: usize = 50;
const PIECES: usize = 60;

fn editor(name: &str) -> App {
    let mut fixture = fixtures::Fixture::new(name);
    for i in 0..PIECES {
        fixture = fixture.descriptor(&format!("site/p{i:02}"), "alpha");
    }
    let root = fixture.build("m");
    started(&root)
}

/// A library of four pieces with four different provenances:
///
/// - `site/bare` has no label record at all (a manifest-generated descriptor);
/// - `site/guessed` was labelled entirely by the model, at low confidence;
/// - `site/mixed` has its kind set by a person and its look set by the model, at medium confidence;
/// - `site/judged` was labelled entirely by a person, with no model and no confidence.
fn labelled_project(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/bare", "alpha")
        .labelled_descriptor("site/guessed", "alpha", "gemma-x", "Low")
        .labelled_by(
            "site/mixed",
            "alpha",
            "(kind: Some(Human), effects: None, look: Some(Model), surfaces: None, mount: None, note: None)",
            r#"model: Some("gemma-x"), at: "2026-10-21", confidence: Some(Medium)"#,
        )
        .labelled_by(
            "site/judged",
            "alpha",
            "(kind: Some(Human), effects: Some(Human), look: Some(Human), surfaces: None, mount: None, note: None)",
            r#"model: None, at: "2026-10-22", confidence: None"#,
        )
        .build("m");
    started(&root)
}

fn started(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Through the registered method**, as an agent's request goes. A refusal panics.
fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    try_call(app, method, params).unwrap_or_else(|e| panic!("{method}: {e}"))
}

/// The same request, with a refusal returned instead of panicking.
fn try_call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{e:?}"))?
        .map_err(|e| format!("{e:?}"))
}

/// **With no project open there is no library, and the answer says so.** An empty page would read
/// as an empty library: an agent that asked which pieces need labelling would conclude none do.
#[test]
fn with_no_project_open_the_list_is_refused_not_empty() {
    let mut app = editor("pieces-no-project");
    app.world_mut()
        .remove_resource::<bevy_cauldron::project::Project>();
    let refused = try_call(&mut app, "grimoire/pieces", serde_json::json!({}));
    let reason = refused.expect_err("a list with no project open must be refused, not answered empty");
    assert!(
        reason.contains("no project is open"),
        "the refusal must say why there is nothing to list; got {reason}"
    );
}

fn ids(answer: &serde_json::Value) -> Vec<String> {
    let mut ids: Vec<String> = answer["pieces"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|p| p["id"].as_str().map(str::to_owned))
        .collect();
    ids.sort();
    ids
}

fn row<'a>(answer: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    answer["pieces"]
        .as_array()
        .and_then(|rows| rows.iter().find(|p| p["id"] == id))
        .unwrap_or_else(|| panic!("no row for {id}: {answer}"))
}

#[test]
fn grimoire_pieces_names_the_first_page_and_says_where_the_next_starts() {
    let mut app = editor("pieces_first_page");
    let answer = call(&mut app, "grimoire/pieces", serde_json::json!({}));

    assert_eq!(
        answer["total"],
        serde_json::json!(PIECES),
        "the true count is reported: {answer}"
    );
    let pieces = answer["pieces"].as_array().cloned().unwrap_or_default();
    assert_eq!(pieces.len(), PIECE_CAP, "one page names the cap, not the library: {answer}");
    assert_eq!(pieces[0]["id"], "site/p00", "the page starts at the start: {answer}");
    assert!(
        pieces[0]["mesh"].as_str().is_some_and(|m| m.ends_with(".glb")),
        "each row names the mesh its piece is drawn with: {answer}"
    );
    assert_eq!(
        answer["pieces_next"],
        serde_json::json!(PIECE_CAP),
        "the next page starts where this one stopped: {answer}"
    );
    assert_eq!(
        answer["how_to_narrow"],
        serde_json::Value::Null,
        "an unfiltered page names no filter to narrow it by: {answer}"
    );
}

#[test]
fn grimoire_pieces_pages_past_the_cap() {
    let mut app = editor("pieces_pages");
    let answer = call(&mut app, "grimoire/pieces", serde_json::json!({"pieces_from": PIECE_CAP}));
    let pieces = answer["pieces"].as_array().cloned().unwrap_or_default();
    assert_eq!(pieces.len(), PIECES - PIECE_CAP, "the second page holds the rest: {answer}");
    assert_eq!(
        pieces.last().and_then(|p| p["id"].as_str()),
        Some("site/p59"),
        "the last piece is on the last page: {answer}"
    );
    assert_eq!(answer["pieces_next"], serde_json::Value::Null, "nothing follows: {answer}");
}

#[test]
fn with_mesh_leaves_out_the_flat_pieces_that_name_no_mesh() {
    let root = fixtures::Fixture::new("pieces_with_mesh")
        .descriptor("site/house", "alpha")
        .descriptor("site/barn", "alpha")
        .descriptor("site/tree", "alpha")
        .flat_descriptor("site/road", 1.0, 1.0)
        .flat_descriptor("site/field", 1.0, 1.0)
        .build("m");
    let mut app = started(&root);

    let all = call(&mut app, "grimoire/pieces", serde_json::json!({}));
    assert_eq!(all["total"], serde_json::json!(5), "every piece is counted: {all}");

    let models = call(&mut app, "grimoire/pieces", serde_json::json!({"with_mesh": true}));
    assert_eq!(models["total"], serde_json::json!(3), "only the three meshed pieces: {models}");
    let pieces = models["pieces"].as_array().cloned().unwrap_or_default();
    assert!(
        pieces.iter().all(|p| p["mesh"].is_string()),
        "no row with no mesh survives with_mesh: {models}"
    );
    assert!(
        pieces.iter().all(|p| p["id"] != "site/road" && p["id"] != "site/field"),
        "the flat pieces are gone: {models}"
    );
}

#[test]
fn the_old_mesh_filter_name_is_refused_not_kept_as_an_alias() {
    let mut app = editor("pieces_old_name");
    let refused = try_call(&mut app, "grimoire/pieces", serde_json::json!({"model_only": true}));
    assert!(
        refused.is_err(),
        "model_only was renamed to with_mesh, and an unknown key is refused: {refused:?}"
    );
}

#[test]
fn each_row_says_who_labelled_each_axis_and_when() {
    let mut app = labelled_project("pieces_provenance");
    let answer = call(&mut app, "grimoire/pieces", serde_json::json!({}));

    let guessed = row(&answer, "site/guessed");
    assert_eq!(guessed["unlabelled"], serde_json::json!(false), "{guessed}");
    assert_eq!(guessed["labels"]["model"], "gemma-x", "{guessed}");
    assert_eq!(guessed["labels"]["at"], "2026-10-20", "{guessed}");
    assert_eq!(guessed["labels"]["confidence"], "low", "{guessed}");
    assert_eq!(
        guessed["labels"]["axes"]["kind"],
        serde_json::json!({"value": ["prop"], "by": "model"}),
        "each labelled axis names its value and who wrote it: {guessed}"
    );

    let mixed = row(&answer, "site/mixed");
    assert_eq!(mixed["labels"]["axes"]["kind"]["by"], "human", "{mixed}");
    assert_eq!(mixed["labels"]["axes"]["look"]["by"], "model", "{mixed}");

    let judged = row(&answer, "site/judged");
    assert_eq!(judged["labels"]["model"], serde_json::Value::Null, "{judged}");
    assert_eq!(judged["labels"]["confidence"], serde_json::Value::Null, "{judged}");
    assert_eq!(judged["labels"]["at"], "2026-10-22", "{judged}");
    assert_eq!(judged["unlabelled"], serde_json::json!(false), "{judged}");

    let bare = row(&answer, "site/bare");
    assert_eq!(bare["unlabelled"], serde_json::json!(true), "no label record: {bare}");
    assert_eq!(bare["labels"]["axes"], serde_json::json!({}), "{bare}");
}

#[test]
fn unlabelled_keeps_only_the_pieces_with_no_labelled_axis() {
    let mut app = labelled_project("pieces_unlabelled");
    let answer = call(&mut app, "grimoire/pieces", serde_json::json!({"unlabelled": true}));
    assert_eq!(ids(&answer), vec!["site/bare"], "{answer}");
    assert_eq!(answer["total"], serde_json::json!(1), "{answer}");
}

#[test]
fn by_model_keeps_the_pieces_with_an_axis_no_person_has_confirmed() {
    let mut app = labelled_project("pieces_by_model");
    let answer = call(&mut app, "grimoire/pieces", serde_json::json!({"by_model": true}));
    assert_eq!(
        ids(&answer),
        vec!["site/guessed", "site/mixed"],
        "a piece with any model-written axis is in; a fully judged one is out: {answer}"
    );
}

#[test]
fn confidence_keeps_only_the_word_it_names() {
    let mut app = labelled_project("pieces_confidence");
    let low = call(&mut app, "grimoire/pieces", serde_json::json!({"confidence": "low"}));
    assert_eq!(ids(&low), vec!["site/guessed"], "{low}");
    let medium = call(&mut app, "grimoire/pieces", serde_json::json!({"confidence": "medium"}));
    assert_eq!(ids(&medium), vec!["site/mixed"], "{medium}");
    let high = call(&mut app, "grimoire/pieces", serde_json::json!({"confidence": "high"}));
    assert!(ids(&high).is_empty(), "no piece is labelled high: {high}");
}

#[test]
fn confidence_refuses_a_word_it_does_not_know() {
    let mut app = labelled_project("pieces_confidence_refused");
    let refused = try_call(&mut app, "grimoire/pieces", serde_json::json!({"confidence": "very"}));
    let message = refused.err().unwrap_or_default();
    assert!(
        message.contains("confidence") && message.contains("high"),
        "the refusal names the words that are accepted: {message}"
    );
}

#[test]
fn id_contains_keeps_the_pieces_whose_id_has_the_text() {
    let mut app = labelled_project("pieces_id_contains");
    let answer = call(&mut app, "grimoire/pieces", serde_json::json!({"id_contains": "mix"}));
    assert_eq!(ids(&answer), vec!["site/mixed"], "{answer}");
}

#[test]
fn filters_combine_and_each_one_narrows_the_list() {
    let mut app = labelled_project("pieces_combined");
    let answer = call(
        &mut app,
        "grimoire/pieces",
        serde_json::json!({"by_model": true, "confidence": "low"}),
    );
    assert_eq!(ids(&answer), vec!["site/guessed"], "{answer}");
}

#[test]
fn a_capped_filtered_page_names_the_filters_that_narrow_it() {
    let mut app = editor("pieces_capped_filter");
    let answer = call(&mut app, "grimoire/pieces", serde_json::json!({"id_contains": "site/p"}));
    assert_eq!(answer["total"], serde_json::json!(PIECES), "{answer}");
    assert_eq!(answer["pieces_next"], serde_json::json!(PIECE_CAP), "{answer}");
    let narrow = answer["how_to_narrow"].as_str().unwrap_or_default();
    for filter in ["with_mesh", "unlabelled", "by_model", "confidence", "id_contains", "pieces_from"] {
        assert!(narrow.contains(filter), "how_to_narrow names {filter}: {answer}");
    }
}
