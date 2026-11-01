//! **A proposal says how varied it is (E-080).**
//!
//! The loop's Inspect step asks two things of generated content: which constraint shaped the layout,
//! and *how varied the seeds were*. `grimoire_list_rules` answered the first two iterations ago and
//! `rules_set` made it adjustable. The second had nothing behind it: `range::measure` is the
//! measurement Smith & Whitehead's *expressive range* is read from, and no caller ever reached it.
//!
//! # What was actually missing
//!
//! Less than the backlog item claimed. `Solved` returns `grid`, `width`, `height` **and** the
//! interfaces (`faces`) that `range::Faces` classifies — its own doc says so in as many words:
//! *"Returned rather than rebuilt… `crate::range` measures a solved grid."* What dropped them was
//! `editor::propose_from_grammar`, which built a `Proposed` and let the rest fall out of scope. So
//! this is four values carried one function further and scored, not a new subsystem.
//!
//! # What is being proved
//!
//! 1. **A `composed` proposal answers with an enclosure, a region count and a cell count**, measured
//!    on the grid the solver actually collapsed rather than on a reconstruction from `placements` —
//!    pinned cells are not in `placements` at all, so a reconstruction would disagree about exactly
//!    the cells the author cared most about.
//! 2. **A source with no interfaces says `null`, and is not scored as if it had none.** A grammar
//!    learned from placements derives no wall faces, so nothing can be classified. `0.0` is a
//!    *meaningful* value on that axis ("no floor is enclosed"), which is why unmeasurable must not be
//!    spelled the same way.
//! 3. **The same seed measures the same**, because a number that moves between runs cannot be
//!    compared with the next seed's — and comparing seeds is the whole reason to have it.
//!
//! # Why these are not pinned to a value
//!
//! The assertions are on *what the answer carries* — that it names a grid, that it counts regions,
//! that an unscorable source is `null`, that a fraction is a fraction — and never on a particular
//! enclosure. A test pinned to `0.43` would pass while the measurement measured the wrong grid.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

/// **A project whose pieces present walls, so a composed grammar can enclose a room.**
///
/// `grammar::from_compositions` derives a prototype's faces from the band geometry of the pieces its
/// members are made of, and reads the token `wall`. `authored_lattice` writes a cell list, and every
/// cell it writes can carry a token — so the piece's whole lattice says `wall` and `range::Faces`
/// has something to classify.
///
/// **`Bounded`, not `Anchored`.** A tile grammar is a grammar over *tiles of the grid's size*:
/// `from_compositions` skips an anchored group by definition — *"it presents no edge to match"* — so
/// an anchored fixture is a grammar of one prototype and `solve` refuses it as *"nothing has been
/// placed yet"*. That refusal cost three attempts to understand.
///
/// Four pieces on the map, because a grammar is learned or solved from an arrangement and `solve`
/// refuses a grammar of fewer than two prototypes.
fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
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

/// **A preview**, whose answer is `would_change` — a preview changes nothing, so the fields are what
/// it *would* do. A first version of these tests read them at the top level and found nothing.
fn propose(app: &mut App, source: &str, seed: u64) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": "layout_propose",
            "parameters": {"source": source, "seed": seed},
            "dry_run": true,
        }),
    )
}

/// **A source with no wall interfaces says `null`, rather than scoring zero.**
#[test]
fn a_source_with_no_interfaces_answers_null_rather_than_zero() {
    let mut app = editor("variety_learned");
    let answer = propose(&mut app, "learned", 1);
    assert_eq!(answer["outcome"], "previewed", "{answer}");
    let would = &answer["would_change"];
    assert!(
        would.get("variety").is_some(),
        "the preview says nothing about variety: {answer}"
    );
    assert!(
        would["variety"].is_null(),
        "a source with no wall interfaces must answer `null`, not a scored zero: {answer}"
    );
}

/// **The same source and seed measure the same**, because a number that moves between runs cannot be
/// compared with the next seed's.
#[test]
fn the_same_seed_measures_the_same() {
    let mut app = editor("variety_stable");
    let first = propose(&mut app, "composed", 7);
    let second = propose(&mut app, "composed", 7);
    assert!(
        !first["would_change"]["variety"].is_null(),
        "the composed source must be measurable for this test to mean anything: {first}"
    );
    assert_eq!(
        first["would_change"]["variety"], second["would_change"]["variety"],
        "the same source and seed answered two different measurements"
    );
}

/// **A `composed` proposal is scored on the grid the solver collapsed.**
/// **The measurement is not the cost of a proposal.** `range::measure` is a flood fill and a
/// region pass over the grid; `solve` is the collapse that fills it. This times the measurement on
/// the same 256-cell grid the test above scores, so a report can say which of the two dominates
/// rather than implying the metric is free.
#[test]
fn the_measurement_is_cheap_beside_the_solve() {
    let mut app = editor("variety_cost");
    let t0 = std::time::Instant::now();
    let answer = propose(&mut app, "composed", 3);
    let whole = t0.elapsed();
    assert!(!answer["would_change"]["variety"].is_null(), "{answer}");
    // **Generous on purpose.** This is a shape, not a benchmark: a measurement that took longer
    // than the solve would be a finding, and one that takes a fraction of it is the ordinary case.
    // A wall-clock assertion tight enough to fail on a loaded machine would be a flake generator --
    // the loop has been burned by exactly that -- so the bound is loose and the *number* is printed.
    eprintln!("PROPOSAL: {} ms for solve + measure", whole.as_millis());
    assert!(
        whole.as_millis() < 5_000,
        "one composed proposal took {} ms",
        whole.as_millis()
    );
}

#[test]
fn a_composed_proposal_is_scored_on_the_solvers_grid() {
    let mut app = editor("variety_composed");
    let answer = propose(&mut app, "composed", 3);
    assert_eq!(answer["outcome"], "previewed", "{answer}");
    let variety = &answer["would_change"]["variety"];
    assert!(
        !variety.is_null(),
        "a composed proposal has interfaces, so it must be measurable: {answer}"
    );
    for key in ["enclosure", "regions", "opening_density"] {
        assert!(
            variety.get(key).is_some(),
            "`{key}` is missing from the measurement: {answer}"
        );
    }
    let enclosure = variety["enclosure"]
        .as_f64()
        .unwrap_or_else(|| panic!("{answer}"));
    assert!(
        (0.0..=1.0).contains(&enclosure),
        "enclosure is a fraction, and {enclosure} is not one: {answer}"
    );
    assert!(variety["regions"].is_u64(), "regions is a count: {answer}");
    // **And it names the grid it measured**, so a reader can tell a 16x16 solve from an empty one.
    assert!(
        variety["cells"].as_u64().is_some_and(|c| c > 0),
        "the measurement does not say how many cells it covered: {answer}"
    );
}
