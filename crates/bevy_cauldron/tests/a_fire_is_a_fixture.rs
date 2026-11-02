//! **A placement can carry a whole `bevy_lights` `Fire`, not only a `Flicker`** (#103): the value is the
//! crate's own type, a `Fire` is told from a `Flicker` by the fields only it has, the preview holds the
//! `Fire` itself (the crate's `FirePlugin` makes its flame and its pooled light in the running editor),
//! and the light map reads the same lumens the pool would be handed.
#![cfg(feature = "bevy_lights")]

use bevy::prelude::*;
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_lights::{Fire, FireKind, Flicker};
use serde_json::json;

use crate::fixtures;

fn editor(name: &str) -> (bevy::prelude::App, Ctx) {
    let root = fixtures::Fixture::new(name).descriptor("site/torch", "alpha").place("site/torch", (0.0, 0.0)).place("site/torch", (6.0, 6.0)).build("m");
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_lights\"])").unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    (app, Ctx { project: Some(project), open: Some(open), session: name.to_owned(), ..Ctx::default() })
}

fn set(ctx: &mut Ctx, nth: usize, value: &str) -> Result<Applied, ops::Refusal> {
    let id = ctx.open.as_ref().unwrap().map.placements[nth].id.clone();
    ops::dispatch(ctx, "placement_set_family", &json!({"id": id, "crate": "bevy_lights", "value": value}))
}

#[test]
fn a_fire_is_read_as_a_fire_and_a_misspelt_kind_is_refused_not_read_as_a_default_flicker() {
    let (_app, mut ctx) = editor("fire_fixture_refuse");
    assert!(matches!(set(&mut ctx, 0, "(kind: Campfire)"), Ok(Applied::Done { .. })));
    let saved = ctx.open.as_ref().unwrap().map.placements[0].family.get("bevy_lights").cloned();
    assert_eq!(saved.as_deref(), Some("(kind: Campfire)"));

    let refused = set(&mut ctx, 1, "(kind: Campfir)");
    let said = match refused {
        Ok(Applied::Refused(e)) | Err(e) => e.0,
        other => panic!("a kind the crate does not have must be refused, got {other:?}"),
    };
    assert!(said.contains("Fire") && said.contains("Campfir"), "the refusal names the Fire type and the value: {said}");
    assert!(ctx.open.as_ref().unwrap().map.placements[1].family.get("bevy_lights").is_none(), "nothing was set");
}

#[test]
fn a_placement_carrying_a_fire_holds_that_fire_and_no_bare_light() {
    let (mut app, _) = editor("fire_fixture_view");
    {
        let mut open = app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>();
        open.map.placements[0].family.insert("bevy_lights".to_owned(), "(kind: Brazier, lumens: 4000.0)".to_owned());
    }
    for _ in 0..3 {
        app.update();
    }
    let world = app.world_mut();
    let mut fires = world.query::<(&Fire, &ChildOf)>();
    let found: Vec<(Fire, Entity)> = fires.iter(world).map(|(f, c)| (*f, c.parent())).collect();
    assert_eq!(found.len(), 1, "one placement carries a fire, so one fire: {found:?}");
    assert_eq!(found[0].0.kind, FireKind::Brazier);
    assert_eq!(found[0].0.lumens, Some(4000.0));
    assert!(world.get::<bevy_cauldron::editor::Placement>(found[0].1).is_some(), "the fire hangs under the placement");
    let mut bare = world.query_filtered::<(), (With<Flicker>, With<PointLight>)>();
    assert_eq!(bare.iter(world).count(), 0, "a fire is not also drawn as a bare flicker light");

    // Taking the value off takes the fire down.
    app.world_mut().resource_mut::<bevy_cauldron::project::OpenMap>().map.placements[0].family.remove("bevy_lights");
    for _ in 0..3 {
        app.update();
    }
    let world = app.world_mut();
    assert_eq!(world.query::<&Fire>().iter(world).count(), 0);
}

#[cfg(feature = "bevy_light_grid")]
mod light_map {
    use super::*;

    fn lux(ctx: &mut Ctx, at: (f32, f32)) -> f64 {
        match ops::dispatch(ctx, "light_map", &json!({"at": [[at.0, at.1]]})).unwrap() {
            Applied::Done { outcome, .. } => outcome.after.unwrap()["samples"][0]["lux"].as_f64().unwrap(),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_fires_own_lumens_light_the_map_like_a_flicker_of_that_base() {
        let (_app, mut ctx) = editor("fire_fixture_map");
        set(&mut ctx, 0, "(kind: Campfire, lumens: 5000.0)").unwrap();
        set(&mut ctx, 1, "(base: 5000.0)").unwrap();
        let fire = lux(&mut ctx, (0.0, 0.0));
        let flicker = lux(&mut ctx, (6.0, 6.0));
        assert!((fire - flicker).abs() < 1e-6, "a fire of 5000 lumens and a flicker of base 5000 read alike: {fire} vs {flicker}");
        assert!((fire - 1.0).abs() > 1e-6, "not the default 1000 a misread flicker would give: {fire}");
    }

    #[test]
    fn a_fire_with_no_lumens_is_sized_by_its_kind() {
        let (_app, mut ctx) = editor("fire_fixture_kind");
        set(&mut ctx, 0, "(kind: Bonfire)").unwrap();
        set(&mut ctx, 1, "(kind: Candle)").unwrap();
        let (bonfire, candle) = (lux(&mut ctx, (0.0, 0.0)), lux(&mut ctx, (6.0, 6.0)));
        assert!(bonfire > candle, "a bonfire lights more than a candle: {bonfire} vs {candle}");
    }
}
