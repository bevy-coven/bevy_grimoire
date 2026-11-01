//! **A layout proposed in an area leaves the rest of the map where it was** (#192, research lens #6:
//! graybox first).
//!
//! `layout_propose` used to solve the whole map, so keeping the layout threw away every unpinned piece
//! outside the part the person meant. With an `area` (a rectangle in map metres, `[x0, z0, x1, z1]`)
//! the solve lays only the cells in it, and `layout_accept` replaces only the unpinned rows in it.

use bevy::prelude::*;
use bevy_cauldron::editor::Proposal;
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;

use crate::fixtures;

/// A 16 m map with a 4 x 4 block of pieces near the middle and one sketch piece in the far corner.
fn editor(name: &str) -> App {
    let mut fixture = fixtures::Fixture::new(name).side(16.0).descriptor("wall", "alpha");
    for i in 0..4 {
        for j in 0..4 {
            fixture = fixture.place("wall", (0.5 + i as f32, 0.5 + j as f32));
        }
    }
    let root = fixture.place("wall", (-5.5, -5.5)).build("m");
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
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("no method `{method}` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    call(app, "grimoire/operation_apply", serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false}))
}

fn rows(app: &App) -> Vec<(String, (f32, f32))> {
    app.world().resource::<OpenMap>().map.placements.iter().map(|p| (p.id.clone(), p.at)).collect()
}

fn inside(at: (f32, f32)) -> bool {
    (0.0..=6.0).contains(&at.0) && (0.0..=6.0).contains(&at.1)
}

#[test]
fn keeping_a_layout_proposed_in_an_area_replaces_only_the_rows_inside_it() {
    let mut app = editor("layout_in_area");
    let before = rows(&app);
    let outside: Vec<_> = before.iter().filter(|(_, at)| !inside(*at)).cloned().collect();
    assert_eq!(outside.len(), 1, "the fixture has one sketch piece outside the area: {before:?}");

    // The first seed that lays something: a learned grammar of a mostly empty map can lay an empty area.
    let laid = (1..=24).find(|seed| {
        let proposed = apply(&mut app, "layout_propose", serde_json::json!({"source": "learned", "seed": seed, "area": [0.0, 0.0, 6.0, 6.0]}));
        assert_eq!(proposed["outcome"], "applied", "{proposed}");
        app.world().resource::<Proposal>().0.as_ref().is_some_and(|p| !p.placements.is_empty())
    });
    assert!(laid.is_some(), "no seed of 24 laid a piece in the area");
    let kept = apply(&mut app, "layout_accept", serde_json::json!({}));
    assert_eq!(kept["outcome"], "applied", "{kept}");

    let after = rows(&app);
    for row in &outside {
        assert!(after.contains(row), "{row:?} stood outside the area and must stay where it was: {after:?}");
    }
    let new: Vec<_> = after.iter().filter(|r| !before.contains(r)).collect();
    assert!(!new.is_empty(), "the area was laid: {after:?}");
    for (id, at) in &new {
        assert!(inside(*at), "{id} was laid at {at:?}, outside the area: {after:?}");
    }
}

#[test]
fn an_area_that_holds_no_cell_of_the_map_is_refused_naming_it() {
    let mut app = editor("layout_in_nowhere");
    let answer = apply(&mut app, "layout_propose", serde_json::json!({"source": "learned", "seed": 3, "area": [100.0, 100.0, 110.0, 110.0]}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let text = answer.to_string();
    assert!(text.contains("100") && text.contains("110"), "the refusal names the area: {answer}");
}

#[test]
fn an_area_that_is_not_four_finite_numbers_is_refused() {
    let mut app = editor("layout_in_bad_area");
    let answer = apply(&mut app, "layout_propose", serde_json::json!({"source": "learned", "area": [0.0, 0.0, 6.0]}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
    let answer = apply(&mut app, "layout_propose", serde_json::json!({"source": "learned", "area": [0.0, 0.0, 6.0, 1.0e39]}));
    assert_eq!(answer["outcome"], "refused", "{answer}");
}
