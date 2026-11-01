//! **The carved ground wears the cover the environment file names** (#101): `volume_cover`, the
//! crate's own `SurfaceCover` in `family/bevy_elements.ron`, is put on the carved chunks' weathered
//! material, follows an edit to the file without a remesh, and the chunks go back to the plain
//! material when the file goes. Needs `bevy_elements` and `isomesh`:
//! `cargo test -p bevy_cauldron --features volume_snow --test editor volume_wears_its_cover::`.
#![cfg(all(feature = "bevy_elements", feature = "isomesh"))]

use bevy::prelude::*;
use bevy_elements::{CoverBand, SurfaceCover, WeatheredExtension, WeatheredMaterial};
use bevy_grimoire_core::heightfield::Heightfield;
use bevy_cauldron::environment::ElementsSettings;
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::terrain_view::TerrainView;
use serde_json::json;

use crate::fixtures;

/// Steps the app until `done` holds, for at most 5 s: the terrain view and the settings poll their files.
fn until(app: &mut App, done: impl Fn(&mut App) -> bool) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < 5.0 {
        app.update();
        if done(app) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    false
}

/// How each drawn chunk of the carved ground is dressed: `Some(extension)` when weathered, `None` when plain.
fn chunks(app: &mut App) -> Vec<Option<String>> {
    let world = app.world_mut();
    let mut q = world.query::<(&TerrainView, Option<&MeshMaterial3d<WeatheredMaterial>>, Option<&MeshMaterial3d<StandardMaterial>>)>();
    let drawn: Vec<_> = q
        .iter(world)
        .filter(|(v, ..)| **v == TerrainView::Volume)
        .map(|(_, w, s)| {
            assert!(w.is_some() != s.is_some(), "a chunk has exactly one material");
            w.map(|w| w.0.clone())
        })
        .collect();
    let materials = world.resource::<Assets<WeatheredMaterial>>();
    drawn.into_iter().map(|h| h.map(|h| format!("{:?}", materials.get(&h).expect("the chunk's material exists").extension))).collect()
}

/// The extension a material wearing `cover` has (`None`: the scene's weather).
fn wearing(cover: Option<SurfaceCover>) -> String {
    let mut e = WeatheredExtension::default();
    e.set_cover(cover);
    format!("{e:?}")
}

fn done(r: Result<Applied, ops::Refusal>) {
    match r.unwrap() {
        Applied::Done { .. } => {}
        Applied::Refused(e) => panic!("refused: {}", e.0),
    }
}

#[test]
fn the_carved_ground_wears_the_files_cover_and_follows_an_edit() {
    let root = fixtures::Fixture::new("volume_cover").descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_elements\"])").unwrap();
    let file = root.join("family/bevy_elements.ron");
    let wet = SurfaceCover { wetness: CoverBand::wet(1.0), ..default() };
    std::fs::write(&file, ElementsSettings { volume_cover: Some(wet), ..ElementsSettings::defaults() }.to_ron()).unwrap();
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // What `bevy_elements`' plugins give the editor's app; the headless harness renders nothing.
    app.init_asset::<WeatheredMaterial>();
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    std::fs::create_dir_all(project.grimoire_dir.join("terrain")).unwrap();
    std::fs::write(project.grimoire_dir.join("terrain/m.ron"), ron::to_string(&Heightfield::flat((0.0, 0.0), 1.0, 33, 33, 0.3)).unwrap()).unwrap();
    let mut ctx = Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() };
    let carve = json!({"map": "m", "edit": {"shape": {"Sphere": {"center": [16.0, 0.3, 16.0], "radius_m": 5.0}}, "mode": "Carve"}});
    done(ops::dispatch(&mut ctx, "volume_edit", &carve));

    let all_wear = |want: String| move |app: &mut App| {
        let c = chunks(app);
        !c.is_empty() && c.iter().all(|e| e.as_deref() == Some(want.as_str()))
    };
    assert!(until(&mut app, all_wear(wearing(Some(wet)))), "every carved chunk wears the file's wet cover: {:?}", chunks(&mut app));

    // An edit to the file, through the operation a person and an agent both use, reaches the drawn
    // chunks without a remesh: the same entities, wearing the new cover.
    let ids = |app: &mut App| -> Vec<Entity> {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &TerrainView)>();
        let mut v: Vec<Entity> = q.iter(world).filter(|(_, t)| **t == TerrainView::Volume).map(|(e, _)| e).collect();
        v.sort();
        v
    };
    let before = ids(&mut app);
    let snow = SurfaceCover { snow: CoverBand::snow(1.0), ..default() };
    let set = |ctx: &mut Ctx, value: String| done(ops::dispatch(ctx, "family_set", &json!({"crate": "bevy_elements", "field": "volume_cover", "value": value})));
    set(&mut ctx, ron::to_string(&Some(snow)).unwrap());
    assert!(until(&mut app, all_wear(wearing(Some(snow)))), "the snow cover replaces the wet one: {:?}", chunks(&mut app));
    assert_eq!(ids(&mut app), before, "a cover edit remeshed the carved ground");
    set(&mut ctx, "None".to_owned());
    assert!(until(&mut app, all_wear(wearing(None))), "no cover: the scene's weather decides: {:?}", chunks(&mut app));
    assert_eq!(ids(&mut app), before, "a cover edit remeshed the carved ground");

    // Without the environment file the editor's own plain material is back.
    std::fs::remove_file(&file).unwrap();
    assert!(
        until(&mut app, |app| {
            let c = chunks(app);
            !c.is_empty() && c.iter().all(Option::is_none)
        }),
        "the chunks go back to the plain material: {:?}",
        chunks(&mut app)
    );
}
