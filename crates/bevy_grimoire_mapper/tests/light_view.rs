//! **A placed torch flickers in the viewport preview** (#103): every placement that carries a
//! `bevy_lights` `Flicker` is drawn with a real light the crate's own `FlickerPlugin` drives, so the
//! author sees the intensity the game will draw, not a number.
#![cfg(feature = "bevy_lights")]

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_grimoire_mapper::project::OpenMap;
use bevy_lights::Flicker;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/torch", "alpha")
        .place("site/torch", (0.0, 0.0))
        .place("site/torch", (6.0, 0.0))
        .build("m");
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_lights\"])").unwrap();
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(40)));
    for _ in 0..4 {
        app.update();
    }
    app
}

fn set_family(app: &mut App, nth: usize, value: Option<&str>) -> String {
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    let p = &mut open.map.placements[nth];
    match value {
        Some(v) => drop(p.family.insert("bevy_lights".to_owned(), v.to_owned())),
        None => drop(p.family.remove("bevy_lights")),
    }
    p.id.clone()
}

/// The lights the preview drew, with the placement each hangs under.
fn lights(app: &mut App) -> Vec<(String, f32, Flicker)> {
    let world = app.world_mut();
    let mut q = world.query::<(&PointLight, &Flicker, &ChildOf)>();
    let hits: Vec<(f32, Flicker, Entity)> = q.iter(world).map(|(l, f, c)| (l.intensity, *f, c.parent())).collect();
    hits.into_iter()
        .filter_map(|(i, f, parent)| world.get::<bevy_grimoire_mapper::editor::Placement>(parent).map(|p| (p.0.clone(), i, f)))
        .collect()
}

#[test]
fn a_placement_carrying_a_flicker_holds_a_light_and_one_without_holds_none() {
    let mut app = editor("light_view_holds");
    assert!(lights(&mut app).is_empty(), "no placement carries a flicker yet");
    let id = set_family(&mut app, 0, Some("(base: 1200.0, phase: 0.4)"));
    for _ in 0..3 {
        app.update();
    }
    let drawn = lights(&mut app);
    assert_eq!(drawn.len(), 1, "one placement carries a flicker, so one light: {drawn:?}");
    assert_eq!(drawn[0].0, id);
    assert_eq!((drawn[0].2.base, drawn[0].2.phase), (1200.0, 0.4), "the light carries the placement's own Flicker");
}

#[test]
fn the_light_flickers_between_the_crates_own_bounds_as_frames_pass() {
    let mut app = editor("light_view_flickers");
    set_family(&mut app, 0, Some("(base: 1000.0, depth: 0.15)"));
    let mut seen = Vec::new();
    for _ in 0..120 {
        app.update();
        if let Some((_, i, _)) = lights(&mut app).first() {
            seen.push(*i);
        }
    }
    assert!(seen.len() >= 100, "the light existed for the frames");
    let (lo, hi) = seen.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &i| (lo.min(i), hi.max(i)));
    assert!(hi - lo > 20.0, "the intensity moves over frames: {lo}..{hi}");
    assert!(hi <= 1000.0 + 1.0 && lo >= 700.0 - 1.0, "base is the peak and depth 0.15 dips to 0.70 of it: {lo}..{hi}");
}

#[test]
fn editing_the_value_retunes_the_light_and_removing_it_takes_the_light_down() {
    let mut app = editor("light_view_edits");
    set_family(&mut app, 0, Some("(base: 1000.0)"));
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(lights(&mut app).len(), 1);
    set_family(&mut app, 0, Some("(base: 400.0, depth: 0.0)"));
    for _ in 0..3 {
        app.update();
    }
    let drawn = lights(&mut app);
    assert_eq!(drawn.len(), 1, "a retune is not a second light: {drawn:?}");
    assert_eq!(drawn[0].2.base, 400.0);
    assert!((drawn[0].1 - 400.0).abs() < 1.0, "with no depth the light holds its base: {}", drawn[0].1);
    set_family(&mut app, 0, None);
    for _ in 0..3 {
        app.update();
    }
    assert!(lights(&mut app).is_empty(), "removing the value removes the light");
}
