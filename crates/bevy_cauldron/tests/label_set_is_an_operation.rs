//! **An agent can set a piece's label axis, through the vocabulary gate, and the write is attributed
//! to the agent.**
//!
//! E-042: an agent could read a piece's labels but could not write one. `label_set` is the
//! registered operation. It refuses an unknown token with the nearest ones, refuses a duplicate,
//! refuses a piece the measured library does not hold, writes `library.ron` through the same path a
//! chip click uses, stamps the axis `Agent` (not `Model`, which is a VLM's word, and not `Human`,
//! which is a person's confirmation), and undoes to the exact prior record. These tests go through the
//! registered BRP methods, as an agent's request goes, and assert on the editor's own state and on
//! the file it wrote.

use bevy::prelude::*;
use bevy_grimoire_core::descriptor::{Axis, By};
use bevy_cauldron::harness;
use bevy_cauldron::ops;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;

use crate::fixtures;

fn editor_at(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .look_tokens(&["worn"])
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

fn label_set(app: &mut App, id: &str, axis: &str, tokens: &[&str]) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "label_set",
            "parameters": {"id": id, "axis": axis, "tokens": tokens},
            "dry_run": false,
        }),
    )
}

/// The piece as the editor holds it in memory — the live resource, not a copy.
fn live_piece(app: &App, id: &str) -> bevy_grimoire_core::descriptor::Descriptor {
    app.world()
        .resource::<Project>()
        .measured
        .descriptors
        .iter()
        .find(|d| d.id == id)
        .unwrap_or_else(|| panic!("`{id}` is not in the live measured library"))
        .clone()
}

fn look_of(app: &App, id: &str) -> Vec<String> {
    live_piece(app, id).look
}

fn look_by(app: &App, id: &str) -> Option<By> {
    live_piece(app, id).labels.and_then(|l| l.by.get(Axis::Look))
}

#[test]
fn the_registry_has_label_set() {
    let op = ops::find("label_set").expect("label_set is a registered operation");
    assert_eq!(op.undo(), ops::Undo::Inverse, "a label change is put back by its inverse");
}

#[test]
fn an_unknown_token_is_refused_with_the_nearest_tokens() {
    let mut app = editor_at("label-set-unknown");
    let answer = label_set(&mut app, "site/wall", "look", &["wrn"]);
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(why.contains("worn"), "the refusal names the close token: {why}");
    assert_eq!(look_of(&app, "site/wall"), vec!["plain".to_string()], "a refused label moves nothing");
}

#[test]
fn a_duplicate_token_is_refused() {
    let mut app = editor_at("label-set-duplicate");
    let answer = label_set(&mut app, "site/wall", "look", &["plain", "plain"]);
    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(look_of(&app, "site/wall"), vec!["plain".to_string()]);
}

#[test]
fn a_piece_the_measured_library_does_not_hold_is_refused() {
    let mut app = editor_at("label-set-unmeasured");
    let answer = label_set(&mut app, "site/nope", "look", &["plain"]);
    assert_eq!(answer["outcome"], "refused", "{answer}");
}

#[test]
fn a_change_is_sorted_stamped_agent_and_reaches_the_editor_and_the_file() {
    let mut app = editor_at("label-set-agent");
    // Given out of vocabulary order: the stored list follows the vocabulary's order.
    let answer = label_set(&mut app, "site/wall", "look", &["worn", "plain"]);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(
        look_of(&app, "site/wall"),
        vec!["plain".to_string(), "worn".to_string()],
        "the live library holds the write, so the next save cannot lose it"
    );
    assert_eq!(look_by(&app, "site/wall"), Some(By::Agent), "an agent's write is not a model's or a person's");
    let path = app.world().resource::<Project>().library_path.clone();
    let on_disk = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    assert!(on_disk.contains("worn"), "the file holds the new token: {on_disk}");
    assert!(on_disk.contains("Agent"), "the file holds the provenance: {on_disk}");
}

#[test]
fn undo_restores_the_prior_record_exactly() {
    let mut app = editor_at("label-set-undo");
    let before = live_piece(&app, "site/wall");
    let applied = label_set(&mut app, "site/wall", "look", &["plain", "worn"]);
    assert_eq!(applied["outcome"], "applied", "{applied}");
    let line = applied["operation_id"].as_str().unwrap_or_else(|| panic!("no operation id: {applied}"));
    let undone = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": line, "dry_run": false}),
    );
    assert_ne!(undone["outcome"], "refused", "{undone}");
    let after = live_piece(&app, "site/wall");
    assert_eq!(after.look, before.look, "the list comes back");
    assert_eq!(
        serde_json::to_value(&after.labels).unwrap(),
        serde_json::to_value(&before.labels).unwrap(),
        "the labels record comes back exactly, including the absence of one"
    );
}

#[test]
fn a_save_that_fails_refuses_and_changes_nothing() {
    let mut app = editor_at("label-set-save-fails");
    let path = app.world().resource::<Project>().library_path.clone();
    let dir = path.parent().expect("a library has a directory").to_path_buf();
    let mut locked = std::fs::metadata(&dir).expect("the kit directory exists").permissions();
    let open_perms = locked.clone();
    use std::os::unix::fs::PermissionsExt;
    locked.set_mode(0o555);
    std::fs::set_permissions(&dir, locked).expect("make the kit directory read-only");

    let answer = label_set(&mut app, "site/wall", "look", &["plain", "worn"]);
    std::fs::set_permissions(&dir, open_perms).expect("restore the kit directory");

    assert_eq!(answer["outcome"], "refused", "{answer}");
    assert_eq!(look_of(&app, "site/wall"), vec!["plain".to_string()], "the editor holds what the file holds");
    assert_eq!(look_by(&app, "site/wall"), None);
}

/// **The receipt names the transition, not just the axis** — E-048's second half.
///
/// `set kind on `torch_metal`` says which axis moved and leaves out what it became, which is the one
/// thing a person reading a receipt wants. Measured on the running editor, that sentence was the
/// whole visible evidence of an agent's label change: `0.116%` of the window.
#[test]
fn the_receipt_says_what_the_axis_changed_from_and_to() {
    let mut app = editor_at("label-set-receipt-transition");
    // `site/wall` starts `look: ["plain"]`; the fixture declares `worn` as a second look token.
    let answer = label_set(&mut app, "site/wall", "look", &["worn"]);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let summary = answer["summary"].as_str().unwrap_or_default();
    assert!(
        summary.contains("+worn"),
        "the receipt must name the token that arrived: {summary}"
    );
    assert!(
        summary.contains("-plain"),
        "and the token that left: {summary}"
    );

    // Clearing an axis says what left, so a deletion is not a receipt that reads `set look`.
    let answer = label_set(&mut app, "site/wall", "look", &[]);
    let summary = answer["summary"].as_str().unwrap_or_default();
    assert!(summary.contains("-worn"), "an emptied axis names what left: {summary}");
}

/// **A re-stated axis names what actually happened**, not "unchanged".
///
/// Setting an axis to the tokens it already holds is not a no-op: `stamp_by` moves the axis's
/// provenance to this writer, so a chip a person had confirmed turns `UNCHECKED` on screen. A
/// receipt reading `unchanged` would be contradicted by the panel beside it.
#[test]
fn a_receipt_for_a_restated_axis_says_the_claim_was_restated() {
    let mut app = editor_at("label-set-receipt-unchanged");
    // The same tokens the piece already has.
    let answer = label_set(&mut app, "site/wall", "look", &["plain"]);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let summary = answer["summary"].as_str().unwrap_or_default();
    assert!(
        summary.contains("claim restated"),
        "re-asserting an axis moves its provenance and repaints the chip, so the receipt must say \
         that rather than `unchanged`: {summary}"
    );
    assert!(
        !summary.contains("unchanged"),
        "a word the screen contradicts: {summary}"
    );
}

/// **The kit door's receipt carries the agent's change too** — E-048.
///
/// The person reads the receipt belonging to the door they are on: `EditorState::status` is the Map
/// door's, and the kit door shows `ImportState::status`. The agent path wrote only the Map one, so
/// with the kit door open an agent's label change reached the file, the live library and the chips,
/// and **said nothing in the row the author was looking at**. Measured on the running editor: the
/// labels row read `agent set kind`, and the words `+crop -tile` appeared nowhere on screen.
#[test]
fn the_kit_doors_receipt_names_the_agents_label_change() {
    let mut app = editor_at("label-set-kit-receipt");
    let answer = label_set(&mut app, "site/wall", "look", &["worn"]);
    assert_eq!(answer["outcome"], "applied", "{answer}");

    let receipt = app
        .world()
        .resource::<bevy_cauldron::tiles::ImportState>()
        .status
        .note_text()
        .to_owned();
    assert!(
        receipt.contains("agent") && receipt.contains("label_set"),
        "the kit door's receipt must say an agent changed a label, because that is the row the \\
         person is reading: {receipt:?}"
    );
    assert!(
        receipt.contains("+worn"),
        "and it must name what changed, not just the axis: {receipt:?}"
    );
}

/// **A refusal reaches the kit door's journal as a problem**, not the receipt slot.
#[test]
fn the_kit_doors_refusal_is_a_problem() {
    let mut app = editor_at("label-set-kit-refusal");
    let answer = label_set(&mut app, "site/wall", "look", &["not-a-token"]);
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let import = app.world().resource::<bevy_cauldron::tiles::ImportState>();
    assert!(
        import.status.has_problem(),
        "a refusal is an alarm, and the kit door must show it where alarms go"
    );
}

/// **A `kind` change names the effects the vocabulary re-settled** — E-063.
///
/// `set_axis` re-runs `settle_implied_effects` whenever `kind` or `effects` moves, because the
/// derived half of `effects` is the vocabulary's answer, not the writer's: a piece that stops being
/// a `light` stops emitting, and one that becomes a bed starts recharging stamina. The write is
/// correct — and the receipt said only `+decor -light`, so the author was never told that four other
/// tokens had changed under them.
///
/// This pins the sentence, not the model: the assertion is about what the receipt SAYS, because the
/// whole point is that the change was invisible.
#[test]
fn a_kind_change_names_the_effects_the_vocabulary_resettled() {
    // `lamp` implies `electricity`; `decor` implies nothing. Setting the kind from one to the other
    // must therefore drop the effect, and the receipt must say so.
    let root = fixtures::Fixture::new("e063-kind-settles")
        .descriptor("site/wall", "alpha")
        .kind_implies("electricity")
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }

    // The piece is a `prop`, which implies `electricity`, and its stored effects are `["inert"]` —
    // so the settled row the editor shows is `[inert, electricity]`. **Taking the kind away is the
    // derivation**: with no kind nothing implies the effect, so the vocabulary drops it, and the
    // author typed only the kind.
    let answer = label_set(&mut app, "site/wall", "kind", &[]);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let summary = answer["summary"].as_str().unwrap_or_default();

    let effects = live_piece(&app, "site/wall").effects;
    assert!(
        !effects.iter().any(|e| e == "electricity"),
        "with no kind the derived effect must go: {effects:?}"
    );
    assert!(
        summary.contains("electricity"),
        "the receipt must name the effect the vocabulary dropped, or the author is told only about \
         the axis they typed and nothing about the rest of the change: {summary:?}"
    );
}

/// **The derived phrase reads as a second sentence, not as a second axis.**
///
/// `effects:+electricity` without the separator would read as part of the axis name. This pins the
/// shape the test above cannot: a `kind` write that moves effects says so after a `;`, and a write
/// that leaves them alone adds nothing at all.
#[test]
fn the_derived_effects_phrase_is_separated_and_only_appears_when_they_moved() {
    let root = fixtures::Fixture::new("e063-separator")
        .descriptor("site/wall", "alpha")
        .kind_implies("electricity")
        .build("m");
    let mut app =
        harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }

    // Taking the kind away drops the derived effect, so the receipt must name it, after a
    // semicolon.
    let first = label_set(&mut app, "site/wall", "kind", &[]);
    let said = first["summary"].as_str().unwrap_or_default();
    assert!(
        said.contains("; effects  -electricity"),
        "the derived effects follow a semicolon so they cannot read as part of the axis: {said:?}"
    );

    // A `look` write touches nothing derived, so it must not mention effects at all.
    let quiet = label_set(&mut app, "site/wall", "look", &["plain"]);
    let said = quiet["summary"].as_str().unwrap_or_default();
    assert!(
        !said.contains("effects"),
        "a write on an axis the vocabulary does not settle must not talk about effects: {said:?}"
    );
}
