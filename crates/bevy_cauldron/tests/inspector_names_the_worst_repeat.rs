//! **The Map inspector names the repeat a player will notice first** (#136 slice 3).
//!
//! Slice 2 put the visible repeats in `grimoire_state`, so an agent could read them; a person looking
//! at the Map tab could not. So the inspector has a `REPEATS` row: the worst repeat and how many
//! pieces it has, how many other looks repeat, or `none`.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str, at: &[(f32, f32)]) -> App {
    let mut fixture = fixtures::Fixture::new(name)
        .descriptor("site/barrel", "alpha")
        .descriptor("site/crate", "alpha");
    for &xz in at {
        fixture = fixture.place("site/barrel", xz);
    }
    fixture = fixture
        .place("site/crate", (2.0, 12.0))
        .place("site/crate", (3.0, 12.0));
    let root = fixture.build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// The texts on every row that holds `value`, with the largest laid-out size of `value` among them.
/// Every row, because a short value such as `none` can also be another row's (BRUSH reads `none`).
fn row_of(app: &mut App, value: &str) -> (Vec<String>, Vec2) {
    let world = app.world_mut();
    let texts: Vec<(Entity, String, Option<Entity>, Vec2)> = world
        .query::<(Entity, &Text, Option<&ChildOf>, &ComputedNode)>()
        .iter(world)
        .map(|(e, t, p, c)| (e, t.0.clone(), p.map(|p| p.parent()), c.size()))
        .collect();
    let found: Vec<_> = texts.iter().filter(|t| t.1 == value).collect();
    assert!(
        !found.is_empty(),
        "no text {value:?} on screen: {:?}",
        texts.iter().map(|t| &t.1).collect::<Vec<_>>()
    );
    let rows: Vec<Entity> = found.iter().filter_map(|t| t.2).collect();
    let labels = texts
        .iter()
        .filter(|t| t.2.is_some_and(|r| rows.contains(&r)) && t.1 != value)
        .map(|t| t.1.clone())
        .collect();
    let size = found.iter().map(|t| t.3).fold(Vec2::ZERO, Vec2::max);
    (labels, size)
}

#[test]
fn the_worst_repeat_and_the_count_of_others_read_on_the_repeats_row() {
    let mut app = editor(
        &format!("inspector_repeat_{}", std::process::id()),
        &[(2.0, 2.0), (3.0, 2.0), (4.0, 2.0)],
    );
    let (labels, size) = row_of(&mut app, "site/barrel \u{d7}3, 1 more");
    assert!(
        labels.iter().any(|l| l.contains("REPEATS")),
        "the value is on the REPEATS row: {labels:?}"
    );
    assert!(size.x > 0.0 && size.y > 0.0, "the value is laid out, so it is drawn: {size:?}");
}

#[test]
fn a_map_with_no_repeat_reads_none() {
    let mut app = fixtures_far_apart();
    let (labels, _) = row_of(&mut app, "none");
    assert!(
        labels.iter().any(|l| l.contains("REPEATS")),
        "`none` is the REPEATS row's: {labels:?}"
    );
}

fn fixtures_far_apart() -> App {
    let root = fixtures::Fixture::new(&format!("inspector_no_repeat_{}", std::process::id()))
        .descriptor("site/barrel", "alpha")
        .place("site/barrel", (1.0, 1.0))
        .place("site/barrel", (14.0, 1.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}
