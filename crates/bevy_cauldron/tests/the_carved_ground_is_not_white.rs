//! **The carved ground is not drawn white** (#101). A carved chunk has no vertex colours, so its base
//! colour is all it wears; left at `StandardMaterial`'s white, `volume_cover`'s snow was invisible on
//! it (measured on a live editor: a carved hill's mean colour was the same bare, snowed and wet, and
//! a snowed flat went from 188,89,77 to 186,184,182 once the base was red). The ground's base must sit
//! well below snow's own white so a cover reads against bare ground.
//! `cargo test -p bevy_cauldron --features volume_snow --test editor the_carved_ground_is_not_white::`.

use bevy::prelude::*;

/// Relative luminance of an sRGB colour, 0 (black) to 1 (white).
fn luminance(color: Color) -> f32 {
    let c = color.to_linear();
    0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue
}

#[test]
fn the_carved_grounds_base_is_well_below_snow_white() {
    let base = luminance(bevy_cauldron::terrain_view::volume_base().base_color);
    let snow = luminance(Color::WHITE);
    assert!(base < 0.5 * snow, "the carved ground's base luminance is {base}; snow on it could not be told from bare ground");
}

#[cfg(all(feature = "bevy_elements", feature = "isomesh"))]
mod drawn {
    /// What a carved chunk wears: its weathered material, or its plain one.
    type Wearing = (Option<Handle<WeatheredMaterial>>, Option<Handle<StandardMaterial>>);

    use super::luminance;
    use bevy::prelude::*;
    use bevy_cauldron::ops::{self, Applied, Ctx};
    use bevy_cauldron::terrain_view::TerrainView;
    use bevy_elements::WeatheredMaterial;
    use bevy_grimoire_core::heightfield::Heightfield;
    use serde_json::json;

    use crate::fixtures;

    /// The base colours the carved chunks wear, plain or weathered, after a carve.
    fn bases(root: &std::path::Path, with_environment_file: bool) -> Vec<f32> {
        if with_environment_file {
            std::fs::create_dir_all(root.join("family")).unwrap();
            std::fs::write(root.join("family.yaml"), "(version: 1, crates: [\"bevy_elements\"])").unwrap();
            std::fs::write(root.join("family/bevy_elements.ron"), bevy_cauldron::environment::ElementsSettings::defaults().to_yaml()).unwrap();
        }
        let mut app = bevy_cauldron::harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        app.init_asset::<WeatheredMaterial>();
        for _ in 0..4 {
            app.update();
        }
        let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
        std::fs::create_dir_all(project.grimoire_dir.join("terrain")).unwrap();
        std::fs::write(project.grimoire_dir.join("terrain/m.ron"), bevy_grimoire_core::yaml::to_string(&Heightfield::flat((0.0, 0.0), 1.0, 33, 33, 0.3)).unwrap()).unwrap();
        let mut ctx = Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() };
        let carve = json!({"map": "m", "edit": {"shape": {"Sphere": {"center": [16.0, 0.3, 16.0], "radius_m": 5.0}}, "mode": "Carve"}});
        match ops::dispatch(&mut ctx, "volume_edit", &carve).unwrap() {
            Applied::Done { .. } => {}
            Applied::Refused(e) => panic!("refused: {}", e.0),
        }
        let start = std::time::Instant::now();
        loop {
            app.update();
            let world = app.world_mut();
            let mut q = world.query::<(&TerrainView, Option<&MeshMaterial3d<WeatheredMaterial>>, Option<&MeshMaterial3d<StandardMaterial>>)>();
            let drawn: Vec<Wearing> =
                q.iter(world).filter(|(v, ..)| **v == TerrainView::Volume).map(|(_, w, s)| (w.map(|w| w.0.clone()), s.map(|s| s.0.clone()))).collect();
            if !drawn.is_empty() || start.elapsed().as_secs_f64() > 5.0 {
                let plain = world.resource::<Assets<StandardMaterial>>();
                let weathered = world.resource::<Assets<WeatheredMaterial>>();
                return drawn
                    .into_iter()
                    .map(|(w, s)| match (w, s) {
                        (Some(h), _) => luminance(weathered.get(&h).expect("material").base.base_color),
                        (_, Some(h)) => luminance(plain.get(&h).expect("material").base_color),
                        _ => unreachable!("a chunk wears a material"),
                    })
                    .collect();
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    #[test]
    fn carved_chunks_are_drawn_with_a_ground_base_plain_or_weathered() {
        for (tag, env) in [("carved_plain", false), ("carved_weathered", true)] {
            let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
            let lum = bases(&root, env);
            assert!(!lum.is_empty(), "{tag}: no carved chunk was drawn");
            assert!(lum.iter().all(|l| *l < 0.5), "{tag}: a carved chunk is drawn white-ish: {lum:?}");
        }
    }
}
