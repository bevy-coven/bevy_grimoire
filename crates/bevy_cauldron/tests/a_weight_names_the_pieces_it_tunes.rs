//! **A weight that tunes no piece says so (E-082, corrected).**
//!
//! `BlockRules::weights` multiplies the learned weight of every piece **whose id starts with a
//! prefix** (`bevy_grimoire_core::rules`:352). So `weights: [(prefix: "town/house_", times: 100.0)]`
//! is a sentence *about the library*, and a prefix no descriptor starts with is a tune that cannot
//! do anything at all.
//!
//! # The item this settles was misdiagnosed, and the measurement is the point
//!
//! E-082 said `rules_set` "cannot check that a rule names descriptors the library has, so it can
//! write a project that will not open" — `border: Some("town/no_such_tile")` applied, then refused
//! at the next open. **That is not reachable.** `rules_set::Params` states `cell`, `seed`,
//! `attempts`, `no_empty` and `weights` and carries `deny_unknown_fields`, so a `border` is refused
//! by serde at parse time before any check runs; and `surgery` rewrites only the fields the
//! parameters named, so no write of this operation's can introduce an id it could not state.
//!
//! What *is* reachable is the one id-shaped field it does own: `weights[].prefix`. Nothing validated
//! it — not `validate_numbers` (numbers only), and not `validate_against` either, which checks
//! `border`, `regions` and each footprint's nine parts and never looks at `weights`. So an agent
//! asked to make houses commoner could write `prefix: "town/hous_"`, be told `set the rule for
//! 'town_block'`, and the next generation would be identical to the one before it. That is the
//! silent no-op [`rules_set::Params`]' own doc refuses for every other field: *"a caller that asks
//! for a field nothing wrote is refused rather than told 'applied'."*
//!
//! # Why this reports rather than refuses, which is the whole design decision
//!
//! Refusing a dead prefix was the first draft and it is wrong, because **the undo path re-dispatches
//! this operation with the old weight list as its parameters** (`Undo::Inverse`, `take_back`). A
//! rules file may legitimately hold a weight written by hand before this check existed; taking back
//! an agent's weights change would then ask to *restore* that dead prefix, and a refusal would leave
//! the agent's write standing while telling the person their Cmd+Z was refused. Reporting keeps the
//! undo exact and still makes the no-op impossible to miss: the preview carries it before the change
//! and the receipt carries it after.
//!
//! # What is reported, and the three things that follow from `prefix` being a prefix
//!
//! 1. **A prefix, not an id.** `al` tunes `alpha`: `starts_with` is the generator's own rule, so a
//!    check that wanted an exact descriptor id would report every family weight as broken.
//! 2. **Only what the request introduces.** A prefix already in the block is the file's standing
//!    state, not this change's doing — which is exactly the case an undo lands in.
//! 3. **The sentence names real pieces**, so an author who meant one reads what to type instead of
//!    being told only that their guess was wrong.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::project::Project;

use crate::fixtures;

/// A rules file whose one weight prefix names the fixture's only piece, `alpha`.
const RULES: &str = "(\n\
    version: 1,\n\
    blocks: [\n\
        (\n\
            example: \"m\",\n\
            regions: [],\n\
            cell: 2.0,\n\
            size: (4, 4),\n\
            border: None,\n\
            no_empty: true,\n\
            weights: [(prefix: \"alpha\", times: 2.0)],\n\
            footprints: [],\n\
            attempts: 8,\n\
            seed: 7,\n\
        ),\n\
    ],\n)\n";

fn boot(name: &str, rules: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .side(8.0)
        .descriptor("alpha", "alpha")
        .rules(rules)
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn editor(name: &str) -> App {
    boot(name, RULES)
}

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

fn apply(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "rules_set", "parameters": parameters, "dry_run": false}),
    )
}

fn preview(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({"operation": "rules_set", "parameters": parameters, "dry_run": true}),
    )
}

fn undo(app: &mut App) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_undo",
        serde_json::json!({"operation_id": null, "dry_run": false}),
    )
}

/// The file, parsed — so a rewrite that kept the right characters but broke the file cannot pass.
fn on_disk(app: &App) -> bevy_grimoire_core::rules::Rules {
    let path = app.world().resource::<Project>().project_dir.join("rules.ron");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    bevy_grimoire_core::rules::Rules::parse(&text)
        .unwrap_or_else(|e| panic!("rules.ron no longer parses: {e}\n{text}"))
}

/// **The preview says a prefix tunes nothing, before anything is written.**
///
/// This is the check that failed before: the preview answered the block it would write and said
/// nothing about the weight reaching no piece.
#[test]
fn the_preview_names_a_prefix_that_tunes_nothing() {
    let mut app = editor("weight_dead_preview");
    let reply = preview(
        &mut app,
        serde_json::json!({"example": "m", "weights": [{"prefix": "beta", "times": 50.0}]}),
    );
    let dead = &reply["would_change"]["tunes_nothing"];
    assert!(
        dead.is_array(),
        "the preview did not say the weight tunes nothing: {reply}"
    );
    let said = dead.to_string();
    assert!(
        said.contains("beta"),
        "the report does not name the prefix that tunes nothing: {said}"
    );
    assert!(
        said.contains("alpha"),
        "the report does not name a piece the library has, so the author cannot tell what the \
         prefix should have been: {said}"
    );
    // **A preview changes nothing**, which is what makes it safe to ask.
    assert_eq!(
        on_disk(&app).blocks[0].weights[0].prefix,
        "alpha",
        "the preview wrote the file"
    );
}

/// **The receipt says it after the write**, in the same words, so the answer an agent reads is not
/// "set the rule" whether the weight did something or nothing.
///
/// The change still applies. That is deliberate: refusing it would break the undo that restores a
/// file's own hand-written weight, and a report is what an agent needs to correct itself.
#[test]
fn the_receipt_says_the_weight_tuned_nothing_and_the_change_still_applies() {
    let mut app = editor("weight_dead_receipt");
    let reply = apply(
        &mut app,
        serde_json::json!({"example": "m", "weights": [{"prefix": "beta", "times": 50.0}]}),
    );
    assert!(
        !reply.to_string().contains("refused"),
        "a dead prefix is reported, not refused, because refusing it would block the undo that \
         restores one: {reply}"
    );
    assert_eq!(
        on_disk(&app).blocks[0].weights[0].prefix,
        "beta",
        "the weight the author asked for was not written: {reply}"
    );
    let receipt = reply["summary"].as_str().unwrap_or_default().to_owned();
    assert!(
        receipt.contains("beta") && receipt.contains("tunes nothing"),
        "the receipt reads the same whether the weight reached a piece or not: {reply}"
    );
}

/// **Nothing is reported when the weight does tune a piece**, so the field is absent rather than an
/// empty list — a field that is always there is a field a reader learns to skip.
#[test]
fn a_weight_that_tunes_a_piece_reports_nothing() {
    let mut app = editor("weight_live_prefix");
    let reply = preview(
        &mut app,
        serde_json::json!({"example": "m", "weights": [{"prefix": "alpha", "times": 50.0}]}),
    );
    assert!(
        reply["would_change"].get("tunes_nothing").is_none(),
        "a live weight was reported as tuning nothing: {reply}"
    );
}

/// **`prefix` stays a prefix.** `al` tunes `alpha` by the generator's own rule, so it must not be
/// reported — a check that wanted an exact id would report every family weight as broken.
#[test]
fn a_prefix_that_starts_a_pieces_id_is_not_reported() {
    let mut app = editor("weight_prefix_family");
    let reply = preview(
        &mut app,
        serde_json::json!({"example": "m", "weights": [{"prefix": "al", "times": 50.0}]}),
    );
    assert!(
        reply["would_change"].get("tunes_nothing").is_none(),
        "a family prefix that tunes `alpha` was reported as tuning nothing: {reply}"
    );
}

/// **One good prefix and one dead one reports only the dead one**, because a sentence that blames
/// both sends the author to fix a weight that was already right.
#[test]
fn a_list_with_one_dead_prefix_names_only_that_one() {
    let mut app = editor("weight_mixed_prefixes");
    let reply = preview(
        &mut app,
        serde_json::json!({"example": "m", "weights": [
            {"prefix": "alpha", "times": 5.0},
            {"prefix": "beta", "times": 50.0},
        ]}),
    );
    let said = reply["would_change"]["tunes_nothing"].to_string();
    assert!(said.contains("beta"), "the dead prefix is not named: {said}");
    assert!(
        !said.contains("`alpha`"),
        "the report blames the live prefix too: {said}"
    );
    assert_eq!(
        reply["would_change"]["tunes_nothing"].as_array().map(Vec::len),
        Some(1),
        "more than the one dead prefix was reported: {reply}"
    );
}

/// **A file that already held a dead prefix can still be changed, and the change undone** — the
/// reason the report is about what a request *introduces* rather than about the list it lands.
///
/// A person can hand-write a weight before this check existed. Their `seed` change must apply
/// without a word about it, their weights change must apply, and the undo must put the old list back
/// even though that old list tunes nothing — otherwise the agent's write is left standing and the
/// person's Cmd+Z is refused.
#[test]
fn a_file_that_already_held_a_dead_prefix_can_still_be_changed_and_undone() {
    const STALE: &str = "(\n\
        version: 1,\n\
        blocks: [\n\
            (\n\
                example: \"m\",\n\
                regions: [],\n\
                cell: 2.0,\n\
                size: (4, 4),\n\
                border: None,\n\
                no_empty: true,\n\
                weights: [(prefix: \"gone/\", times: 3.0)],\n\
                footprints: [],\n\
                attempts: 8,\n\
                seed: 7,\n\
            ),\n\
        ],\n\
    )\n";
    let mut app = boot("weight_stale_prefix", STALE);
    assert_eq!(
        on_disk(&app).blocks[0].weights[0].prefix,
        "gone/",
        "the fixture must carry the stale prefix, or this test proves nothing"
    );

    // **A change that does not touch weights says nothing about them**, because the standing prefix
    // is not this request's doing.
    let seeded = preview(&mut app, serde_json::json!({"example": "m", "seed": 99}));
    assert!(
        seeded["would_change"].get("tunes_nothing").is_none(),
        "a seed change was blamed for a weight it never touched: {seeded}"
    );
    apply(&mut app, serde_json::json!({"example": "m", "seed": 99}));

    let weighted = apply(
        &mut app,
        serde_json::json!({"example": "m", "weights": [{"prefix": "alpha", "times": 5.0}]}),
    );
    assert!(!weighted.to_string().contains("refused"), "refused: {weighted}");
    assert_eq!(on_disk(&app).blocks[0].weights[0].prefix, "alpha");

    // **The undo restores the file's own dead prefix and is not refused**, which is the whole reason
    // this is a report and not a refusal.
    let taken_back = undo(&mut app);
    assert!(
        !taken_back.to_string().contains("refused"),
        "the undo was refused, which leaves the agent's write standing: {taken_back}"
    );
    assert_eq!(
        on_disk(&app).blocks[0].weights[0].prefix,
        "gone/",
        "the undo did not put the file's own weight list back: {taken_back}"
    );
}
