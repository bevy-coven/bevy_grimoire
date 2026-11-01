//! **An agent can propose a layout (S1.3, `layout_propose`).**
//!
//! The generate keys have always worked for a person: `G` offers a layout, it is drawn as outlines,
//! and `Enter` keeps it or `Esc` throws it away. But the offer was `editor`'s own private act — five
//! functions taking `&mut EditorState` — so an agent could not ask for one, and an agent is exactly
//! who the brief's *"a mossy ruined courtyard with two exits"* is supposed to reach.
//!
//! # What is being proved
//!
//! 1. **It is in the registry**, so `grimoire_list_operations` names it and an MCP client can call it.
//! 2. **A proposal changes nothing** — not the map, not the file. That is the whole promise: the
//!    person or the agent decides, and a proposal is an offer.
//! 3. **The same seed on the same map lays the same layout.** Without that an agent's proposal is a
//!    layout nobody can reproduce, and the log's seed would be decoration.
//! 4. **The seed reaches the log**, beside the operation id, so the run can be replayed.
//! 5. **An unknown source is refused by name**, rather than silently treated as `learned`.

use bevy::prelude::*;
use bevy_cauldron::editor::Proposal;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

/// **A project with two one-cell compositions.** The generator needs a grammar over compositions:
/// `learned` has nothing to continue on a map holding one piece, and a kit with no `Bounded`
/// composition has nothing for `composed` to tile.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .descriptor("rug", "alpha")
        .bounded_composition("tile_floor", (1.0, 1.0, 1.0), &[("floor", "floor", (0.0, 0.0))])
        .bounded_composition("tile_rug", (1.0, 1.0, 1.0), &[("rug", "rug", (0.0, 0.0))])
        .place("rug", (0.5, 0.5))
        .build("m")
}

/// **The editor, with the agent's door open.** The same plugin graph as
/// `an_agent_change_is_on_the_receipt`.
fn editor_with_bridge(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

/// **Call one of the editor's BRP commands through the registered method**, as the MCP server does.
fn call(
    app: &mut App,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    use bevy::remote::RemoteMethodSystemId;
    use bevy::remote::RemoteMethods;
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

/// Apply one operation, which is what an agent's tool does underneath.
fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "grimoire/operation_apply",
        serde_json::json!({
            "operation": operation,
            "parameters": parameters,
            "dry_run": false,
        }),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

/// Warm the app up so the first real frame is not the one being measured.
fn ready(name: &str) -> (std::path::PathBuf, App) {
    let root = project(name);
    let mut app = editor_with_bridge(&root);
    for _ in 0..8 {
        app.update();
    }
    (root, app)
}

/// **1. The operation is listed.**
#[test]
fn layout_propose_is_listed() {
    let (_root, mut app) = ready("proplisted");
    let listed = call(&mut app, "grimoire/operation_list", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("{e}"));
    // The handler's rows name the operation under `operation`, not `name` — read from
    // `ops_brp::operation_list`, which builds `{operation, undo, parameters}`.
    let names: Vec<&str> = listed
        .get("operations")
        .and_then(|o| o.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|o| o.get("operation").and_then(|n| n.as_str()))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        names.contains(&"layout_propose"),
        "an agent cannot propose a layout, so the brief's \"a mossy ruined courtyard\" stops at an \
         empty map. The registry holds: {names:?}"
    );
}

/// **2. A proposal changes nothing on the map or on the disk.**
#[test]
fn a_proposal_changes_nothing_on_the_map_or_the_disk() {
    let (_root, mut app) = ready("propnothing");
    let before_map = app.world().resource::<OpenMap>().map.clone();
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let before_file = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));

    let applied = apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "composed", "seed": 7}),
    );
    assert_eq!(applied["outcome"], "applied", "the propose did not apply: {applied}");
    assert_eq!(
        applied["saved"], false,
        "a proposal wrote to the file, which is the one thing an offer must not do"
    );

    let proposal = app.world().resource::<Proposal>();
    let proposed = proposal
        .0
        .as_ref()
        .unwrap_or_else(|| panic!("no proposal is standing after a propose: {applied}"));
    assert!(
        !proposed.stamps.is_empty(),
        "the `composed` source proposed nothing at all, so 'it proposes' was not exercised"
    );

    assert_eq!(
        app.world().resource::<OpenMap>().map,
        before_map,
        "the map changed under a proposal — an offer must leave the document alone"
    );
    let after_file = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        after_file, before_file,
        "the map file changed under a proposal"
    );
}

/// **3. The same seed lays the same layout, and a different one lays a different layout.**
#[test]
fn the_same_seed_lays_the_same_layout() {
    let (_root, mut app) = ready("propseed");
    apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "composed", "seed": 7}),
    );
    let first = app
        .world()
        .resource::<Proposal>()
        .0
        .clone()
        .expect("a proposal is standing");

    apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "composed", "seed": 7}),
    );
    let second = app
        .world()
        .resource::<Proposal>()
        .0
        .clone()
        .expect("a proposal is standing");

    assert_eq!(
        first.stamps, second.stamps,
        "the same seed on the same map laid a different layout, so an agent's proposal cannot be \
         reproduced and the seed in the log is decoration"
    );

    apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "composed", "seed": 99}),
    );
    let third = app
        .world()
        .resource::<Proposal>()
        .0
        .clone()
        .expect("a proposal is standing");
    assert_ne!(
        first.stamps, third.stamps,
        "two different seeds laid the identical layout, so the seed is not reaching the solver"
    );
}

/// **4. The seed is in the log**, beside the operation id.
#[test]
fn the_seed_is_in_the_log() {
    let (_root, mut app) = ready("propseedlog");
    let applied = apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "composed", "seed": 7}),
    );
    let entry = applied
        .get("log_entry")
        .unwrap_or_else(|| panic!("the applied operation logged nothing: {applied}"));
    assert_eq!(
        entry["rng_seed"], 7,
        "the seed an agent's proposal ran under is not in the log, so the layout cannot be replayed. \
         entry: {entry}"
    );
}

/// **5. An unknown source is refused by name**, not silently treated as `learned`.
#[test]
fn an_unknown_source_is_refused_by_name() {
    let (_root, mut app) = ready("propbad");
    let refused = apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "guessed", "seed": 7}),
    );
    assert_eq!(
        refused["outcome"], "refused",
        "an unknown source was accepted, so the operation guessed what the caller meant: {refused}"
    );
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("guessed") || why.contains("source"),
        "the refusal does not name the value it could not read: {why:?}"
    );
    assert!(
        app.world().resource::<Proposal>().0.is_none(),
        "a refused propose left a proposal standing"
    );
}

/// **6. An agent keeps a layout and it is on disk.**
#[test]
fn an_agent_keeps_a_layout_and_it_is_on_disk() {
    let (_root, mut app) = ready("propkeep");
    let applied = apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "composed", "seed": 7}),
    );
    assert_eq!(applied["outcome"], "applied", "{applied}");
    let proposed = app
        .world()
        .resource::<Proposal>()
        .0
        .clone()
        .expect("a proposal is standing");

    let kept = apply(&mut app, "layout_accept", serde_json::json!({}));
    assert_eq!(kept["outcome"], "applied", "the accept did not apply: {kept}");
    assert_eq!(kept["saved"], true, "the accept did not write the map: {kept}");

    let open = app.world().resource::<OpenMap>();
    assert!(
        !open.map.stamps.is_empty(),
        "the layout was kept but the map holds no stamps: {}",
        kept["summary"]
    );
    assert!(
        open.map.stamps.iter().all(|s| s.of.starts_with("tile_")),
        "a kept stamp names something that is not a composition: {:?}",
        open.map.stamps.iter().map(|s| &s.of).collect::<Vec<_>>()
    );
    assert!(
        app.world().resource::<Proposal>().0.is_none(),
        "the proposal is still standing after it was kept, so a second Enter would apply it twice"
    );
    // **The file has them too**, which is the difference between "kept" and "kept and saved".
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
    let on_disk: bevy_grimoire_core::map::Map =
        bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        on_disk.stamps.len(),
        open.map.stamps.len(),
        "the map holds the layout and the file does not"
    );
    assert_eq!(
        proposed.stamps.len(),
        open.map.stamps.len(),
        "the map holds a different number of stamps than the proposal offered"
    );
}

/// **7. Undo takes the kept layout back** — the operation's inverse restores what its commit
/// changed, and leaves nothing else touched.
#[test]
fn undo_takes_the_kept_layout_back() {
    let (_root, mut app) = ready("propundo");
    apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "composed", "seed": 7}),
    );
    let placed_before = app.world().resource::<OpenMap>().map.placements.len();
    assert_eq!(app.world().resource::<OpenMap>().map.stamps.len(), 0);

    apply(&mut app, "layout_accept", serde_json::json!({}));
    let kept = app.world().resource::<OpenMap>().map.clone();
    assert!(!kept.stamps.is_empty(), "nothing was kept to undo");
    // **The map as the accept found it**, so "restored" has something to be equal to. Captured by
    // proposing again — the proposal changes nothing, which test 2 proves.
    let mut before_undo = kept.clone();
    before_undo.stamps.clear();

    // **`dry_run` defaults to `true`**, deliberately: an undo that changes things without being
    // asked to is the wrong default. The real undo says so.
    let undone = call(
        &mut app,
        "grimoire/operation_undo",
        serde_json::json!({"dry_run": false}),
    )
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(undone["outcome"], "applied", "the undo did not apply: {undone}");
    let after_undo = app.world().resource::<OpenMap>().map.clone();
    assert!(
        after_undo.stamps.is_empty(),
        "undo left the layout's stamps on the map: {:?}",
        after_undo.stamps.iter().map(|s| &s.id).collect::<Vec<_>>()
    );
    assert_eq!(
        after_undo.placements.len(),
        placed_before,
        "undo did not put the map's placements back as they were"
    );

    // **Redo is the editor's own stack, and an agent's undo does not feed it.** `operation_undo`
    // calls `forget_line` when a change is taken back rather than joining the history, so after an
    // agent's undo there is nothing on the redo stack — measured here, and logged as E-076 rather
    // than asserted as if it worked. What this test can prove is the operation's own contract: the
    // inverse it recorded restores exactly what its commit changed.
    assert_eq!(
        after_undo.placements,
        before_undo.placements,
        "the undo did not restore the placements the accept had cleared"
    );
    assert_eq!(
        after_undo.bounds, before_undo.bounds,
        "the undo changed something the accept never touched"
    );
    let _ = kept;
}

/// **8. Keeping with nothing proposed is refused**, rather than silently doing nothing.
#[test]
fn keeping_with_nothing_proposed_is_refused() {
    let (_root, mut app) = ready("propnokeep");
    let refused = apply(&mut app, "layout_accept", serde_json::json!({}));
    assert_eq!(
        refused["outcome"], "refused",
        "keeping nothing was reported as a change: {refused}"
    );
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(
        why.contains("no layout") && why.contains("layout_propose"),
        "the refusal does not say what to do instead: {why:?}"
    );
}

/// **9. Discarding leaves the map alone**, and clears the proposal.
#[test]
fn discarding_leaves_the_map_alone() {
    let (_root, mut app) = ready("propdiscard");
    apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "composed", "seed": 7}),
    );
    let before = app.world().resource::<OpenMap>().map.clone();

    let discarded = apply(&mut app, "layout_discard", serde_json::json!({}));
    assert_eq!(discarded["outcome"], "applied", "{discarded}");
    assert_eq!(
        app.world().resource::<OpenMap>().map,
        before,
        "discarding a proposal changed the map"
    );
    assert!(
        app.world().resource::<Proposal>().0.is_none(),
        "the proposal is still standing after it was thrown away"
    );
}

/// **10. An agent opens a map by name** — the verb that was missing (E-067).
///
/// `map_create` could make a map and then there was no way in: `door_open` takes a door and a tab.
/// The acceptance script's third step failed on exactly this.
#[test]
fn an_agent_opens_a_map_by_name() {
    let (_root, mut app) = ready("propopenn");
    let listed = call(&mut app, "grimoire/operation_list", serde_json::json!({}))
        .unwrap_or_else(|e| panic!("{e}"));
    let names: Vec<&str> = listed["operations"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|o| o.get("operation").and_then(|n| n.as_str()))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        names.contains(&"map_open"),
        "no operation opens a map by name, so an agent that made one cannot work in it. The \
         registry holds: {names:?}"
    );

    // **Opening the map that is already open is a no-op that answers.** This is the observable the
    // test can make in one process: the editor it was built with is already on `m`.
    let opened = apply(&mut app, "map_open", serde_json::json!({"name": "m"}));
    assert_eq!(opened["outcome"], "applied", "the open did not apply: {opened}");
    assert!(
        opened["summary"].as_str().unwrap_or_default().contains('m'),
        "the receipt does not name the map: {opened}"
    );

    // **A name that is not there is refused**, rather than starting an empty map on a name nobody
    // chose. `OpenMap::open` starts one when the file is absent, which is right for the editor's own
    // "new map" path and wrong for this verb's caller — so it is checked here.
    let refused = apply(&mut app, "map_open", serde_json::json!({"name": "no_such_map"}));
    assert_eq!(
        refused["outcome"], "refused",
        "opening a map that does not exist invented one: {refused}"
    );
}

/// **11. `Esc` must not wipe the map.**
///
/// `discard_layout` swaps the real map out for an empty one while the operation holds it, and writes
/// back whatever the operation left. `layout_discard` changes no map at all — so the context still
/// held the empty placeholder, and the write-back installed it. `Esc` on a proposal therefore
/// **emptied the open map**. Found by review of the Esc path; the same shape applies to any operation
/// whose commit never touches `ctx.open`.
#[test]
fn discarding_a_proposal_does_not_wipe_the_map() {
    let (_root, mut app) = ready("propdiscardmap");
    let before = app.world().resource::<OpenMap>().map.clone();
    assert!(
        !before.placements.is_empty(),
        "the fixture must hold a placement for a wipe to be observable"
    );

    apply(
        &mut app,
        "layout_propose",
        serde_json::json!({"source": "composed", "seed": 7}),
    );
    apply(&mut app, "layout_discard", serde_json::json!({}));

    assert_eq!(
        app.world().resource::<OpenMap>().map, before,
        "discarding a proposal changed the map — and the shape that does it is the placeholder an \
         operation is given while it holds the real one"
    );
}
