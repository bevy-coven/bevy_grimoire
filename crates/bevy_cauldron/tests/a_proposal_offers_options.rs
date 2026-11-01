//! **"Show me three": `layout_propose` offers distinct layouts, each with its seed** (#153, lens #10).
//!
//! A proposal was one layout per seed, so an agent asked for options guessed seeds and could not
//! tell two identical layouts apart. `options: N` tries seeds from `seed` upward, keeps the first N
//! whose layouts differ, and names each one's seed so the agent proposes the one the person picks.
//!
//! # What is being proved
//!
//! 1. **The options differ pairwise**, and each says by how many pieces it differs from its nearest.
//! 2. **The first option is the layout `seed` alone lays**, so asking for options changes nothing
//!    about which layout stands.
//! 3. **A count outside 1..=8 is refused** rather than clamped.
//! 4. **The commit stands the first option** and answers the list.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
        .side(16.0)
        .authored_lattice("wall", "alpha", "wall", 2)
        .place("wall", (2.0, 2.0))
        .place("wall", (4.0, 2.0))
        .place("wall", (2.0, 4.0))
        .place("wall", (4.0, 4.0))
        .bounded_composition("pair", (1.0, 1.0, 1.0), &[("a", "wall", (0.0, 0.0))])
        .bounded_composition("single", (1.0, 1.0, 1.0), &[("only", "wall", (0.0, 0.0))])
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) =
        app.world().resource::<RemoteMethods>().get(method).cloned()
    else {
        panic!("no method `{method}` is registered");
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
        serde_json::json!({
            "operation": "layout_propose",
            "parameters": parameters,
            "dry_run": dry_run,
        }),
    )
}

#[test]
fn three_options_differ_pairwise_and_name_their_seeds() {
    let mut app = editor("options_three");
    let answer = apply(
        &mut app,
        serde_json::json!({"source": "learned", "seed": 1, "options": 3}),
        true,
    );
    assert_eq!(answer["outcome"], "previewed", "{answer}");
    let options = answer["would_change"]["options"]
        .as_array()
        .unwrap_or_else(|| panic!("no options in the answer: {answer}"));
    assert_eq!(options.len(), 3, "{answer}");
    let seeds: Vec<u64> = options
        .iter()
        .map(|o| o["seed"].as_u64().unwrap_or_else(|| panic!("an option without a seed: {o}")))
        .collect();
    assert_eq!(seeds[0], 1, "the first option is the asked seed's: {answer}");
    let mut sorted = seeds.clone();
    sorted.dedup();
    assert_eq!(sorted.len(), 3, "three seeds, not one twice: {seeds:?}");
    for o in options {
        let differs = o["differs"].as_u64().unwrap_or_else(|| panic!("no differs: {o}"));
        assert!(differs > 0, "an option identical to another was offered: {answer}");
    }
    // Each option, proposed alone by its seed, lays what the option counted.
    for o in options {
        let alone = apply(
            &mut app,
            serde_json::json!({"source": "learned", "seed": o["seed"]}),
            true,
        );
        assert_eq!(
            alone["would_change"]["placements"], o["placements"],
            "option {o} is not what its seed lays: {alone}"
        );
    }
}

#[test]
fn a_count_outside_one_to_eight_is_refused() {
    let mut app = editor("options_bounds");
    for n in [0, 9] {
        let answer = apply(
            &mut app,
            serde_json::json!({"source": "learned", "options": n}),
            true,
        );
        assert_eq!(answer["outcome"], "refused", "options {n}: {answer}");
    }
}

#[test]
fn the_commit_stands_the_first_option() {
    let mut app = editor("options_commit");
    let alone = apply(&mut app, serde_json::json!({"source": "learned", "seed": 5}), true);
    let answer = apply(
        &mut app,
        serde_json::json!({"source": "learned", "seed": 5, "options": 2}),
        false,
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let after = &answer["log_entry"]["after"];
    assert_eq!(after["seed"], 5, "{answer}");
    assert_eq!(
        after["placements"], alone["would_change"]["placements"],
        "the standing proposal is the first option's: {answer}"
    );
    assert_eq!(after["options"][0]["seed"], 5, "{answer}");
}
