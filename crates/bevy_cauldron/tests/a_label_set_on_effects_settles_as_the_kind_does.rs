//! **An effect an agent writes is settled the way a kind write settles it.** Cycle 9 gap 6.
//!
//! `prop` implies `powered` in this vocabulary. Writing `kind` settled the effects a kind implies;
//! writing `effects` did not, so the registry answered with a list the library's layered read then
//! changed underneath. Now a derived effect that the piece's kind does not imply is refused, naming
//! the kind, and leaving an implied effect out puts it back. The axis stays stamped `agent`.

use bevy::prelude::*;
use bevy_grimoire_core::descriptor::{Axis, By};
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;
use bevy_cauldron::tiles::Mode;

use crate::fixtures;

fn editor_at(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .kind_implies("powered")
        .descriptor("floor", "alpha")
        .descriptor("bench", "alpha")
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

#[test]
fn an_effect_the_kind_does_not_imply_is_refused_and_the_piece_is_unchanged() {
    let mut app = editor_at("effects-refused");
    let cleared = label_set(&mut app, "bench", "kind", &[]);
    assert_eq!(cleared["outcome"], "applied", "{cleared}");
    let before = live_piece(&app, "bench").effects;
    let answer = label_set(&mut app, "bench", "effects", &["powered"]);
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let why = answer["refusal"].as_str().unwrap_or_default();
    assert!(why.contains("powered"), "the refusal names the effect: {why}");
    assert!(
        why.contains("does not imply"),
        "the refusal says the kind does not imply it: {why}"
    );
    assert_eq!(live_piece(&app, "bench").effects, before, "a refused write changes nothing");
}

#[test]
fn leaving_out_an_effect_the_kind_implies_puts_it_back_and_stays_agent() {
    let mut app = editor_at("effects-settled");
    let kind = label_set(&mut app, "bench", "kind", &["prop"]);
    assert_eq!(kind["outcome"], "applied", "{kind}");
    assert!(
        live_piece(&app, "bench").effects.iter().any(|e| e == "powered"),
        "a kind write settles the effect it implies"
    );

    let answer = label_set(&mut app, "bench", "effects", &[]);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let piece = live_piece(&app, "bench");
    assert!(
        piece.effects.iter().any(|e| e == "powered"),
        "an effects write settles the implied effect back in: {:?}",
        piece.effects
    );
    let by = piece.labels.and_then(|l| l.by.get(Axis::Effects));
    assert_eq!(by, Some(By::Agent), "the axis stays stamped agent");
}

/// **An `effects` write says what the vocabulary put back** — E-064.
///
/// `set_axis` settles `effects` after an effects write as well as after a `kind` write, so an author
/// who removes an implied effect has it restored. That is the right behaviour — the kind decides the
/// derived half, and the library must not keep a claim the kind contradicts — but the receipt said
/// only what the author typed:
///
///     set effects on `bench`  -powered
///
/// and `powered` was still there. The sentence named a removal that had not happened, which is worse
/// than saying nothing: a person reading it believes the effect is gone.
///
/// This pins the correction. The phrase is the *derived* one, reported separately from the axis the
/// author wrote, exactly as a `kind` write already does it (E-063).
#[test]
fn an_effects_write_says_which_effect_the_vocabulary_put_back() {
    let mut app = editor_at("effects-settled-receipt");
    let kind = label_set(&mut app, "bench", "kind", &["prop"]);
    assert_eq!(kind["outcome"], "applied", "{kind}");

    // The author asks for no effects at all. `prop` implies `powered`, so the vocabulary puts it back.
    let answer = label_set(&mut app, "bench", "effects", &[]);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let summary = answer["summary"].as_str().unwrap_or_default();
    let piece = live_piece(&app, "bench");
    assert!(
        piece.effects.iter().any(|e| e == "powered"),
        "the vocabulary must keep the implied effect: {:?}",
        piece.effects
    );
    // The author asked for no effects. `prop` implies `powered`, so the vocabulary kept it — and the
    // receipt said only `-inert`, which is the net change and not the thing the author needs to
    // know: their request was overruled.
    assert!(
        summary.contains("kept") && summary.contains("powered"),
        "the receipt must say the vocabulary kept an effect the author asked to remove: {summary:?}"
    );
}

/// **And a plain effects write that settles nothing keeps its single clause.**
///
/// The guard for the test above: an effects write on a piece whose kind implies nothing must not grow
/// a second clause about effects it never touched.
#[test]
fn an_effects_write_with_nothing_derived_keeps_its_one_clause() {
    let mut app = editor_at("effects-no-derivation");
    // No kind, so nothing is implied and the write settles nothing.
    let cleared = label_set(&mut app, "bench", "kind", &[]);
    assert_eq!(cleared["outcome"], "applied", "{cleared}");
    // With no kind, `[]` really does empty the axis — nothing is implied, so nothing is put back.
    let answer = label_set(&mut app, "bench", "effects", &[]);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let summary = answer["summary"].as_str().unwrap_or_default();
    assert!(
        summary.contains("-inert"),
        "the author's own removal is named: {summary:?}"
    );
    assert!(
        !summary.contains("kept"),
        "and no override is claimed where none happened: {summary:?}"
    );
}

/// **The clause is right on a row the file had not caught up with** — the gap E-064's review found.
///
/// `Project::open` settles the layered `library` but not `measured`, the copy this operation edits
/// (`project.rs:251-255`). So a piece whose stored `effects` predate its kind's `implies` rule shows
/// `[inert, powered]` on screen while the file still holds `[inert]`. On that row, an `effects: []`
/// write was reported as **`+powered -inert`** — an *addition* — when `powered` was the effect the
/// author asked to remove and the vocabulary put back.
///
/// **No `kind` write here**, deliberately: the test above does one first, which settles `measured`
/// and hides exactly this case.
#[test]
fn the_kept_clause_is_right_before_anything_has_settled_the_file() {
    let mut app = editor_at("effects-stale-row");
    // The fixture's `bench` is `kind: ["prop"]` with `effects: ["inert"]` on disk, and `prop`
    // implies `powered` — so the screen shows `[inert, powered]` and the file does not.
    let before = live_piece(&app, "bench");
    assert_eq!(
        before.effects,
        vec!["inert".to_owned()],
        "the fixture's stored effects must still be the un-settled ones, or this is not the case"
    );

    let answer = label_set(&mut app, "bench", "effects", &[]);
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let summary = answer["summary"].as_str().unwrap_or_default();
    assert!(
        summary.contains("kept") && summary.contains("powered"),
        "`powered` was kept, not added — the receipt must say so: {summary:?}"
    );
    assert!(
        !summary.contains("+powered"),
        "and must not report the vocabulary's own effect as the author's addition: {summary:?}"
    );
}
