//! **A 16-bit height map imports as the map's terrain** (#99): the heights round-trip and one undo puts the ground back.

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::Project;
use serde_json::json;

use crate::fixtures;

#[test]
fn a_16_bit_png_round_trips_and_undo_removes_it() {
    let root = fixtures::Fixture::new("height_map_import").descriptor("site/wall", "alpha").place("site/wall", (5.0, 5.0)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    let samples: Vec<u16> = vec![0, 16384, 32768, 65535, 100, 200];
    let img = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_raw(3, 2, samples.clone()).unwrap();
    let file = project.root.join("hills.png");
    img.save(&file).unwrap();
    let mut ctx = Ctx { project: Some(project.clone()), open: Some(open), session: "test".to_owned(), ..Ctx::default() };
    let p = json!({"map": "m", "file": "hills.png", "cell_m": 2.0, "range_m": [0.0, 100.0], "origin": [1.0, 1.0]});
    let Applied::Done { operation_id, .. } = ops::dispatch(&mut ctx, "terrain_import", &p).unwrap() else { panic!("refused") };
    let text = std::fs::read_to_string(project.grimoire_dir.join("terrain").join("m.ron")).unwrap();
    let field: bevy_grimoire_core::heightfield::Heightfield = ron::from_str(&text).unwrap();
    assert_eq!((field.width, field.depth, field.cell_m, field.origin), (3, 2, 2.0, (1.0, 1.0)));
    assert_eq!(field.to_u16((0.0, 100.0)).unwrap(), samples, "the saved field reads back as the same 16-bit samples");
    ops::undo(&mut ctx, Some(&operation_id)).unwrap();
    assert!(!project.grimoire_dir.join("terrain").join("m.ron").exists(), "undo of a first import leaves no terrain");
}

#[test]
fn an_8_bit_image_is_refused_untouched() {
    let root = fixtures::Fixture::new("height_map_8bit").descriptor("site/wall", "alpha").place("site/wall", (5.0, 5.0)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<Project>().clone();
    image::GrayImage::from_raw(2, 2, vec![0, 1, 2, 3]).unwrap().save(project.root.join("flat8.png")).unwrap();
    let mut ctx = Ctx { project: Some(project.clone()), open: None, session: "test".to_owned(), ..Ctx::default() };
    let p = json!({"map": "m", "file": "flat8.png", "cell_m": 1.0, "range_m": [0.0, 10.0]});
    assert!(ops::dispatch(&mut ctx, "terrain_import", &p).is_err());
    assert!(!project.grimoire_dir.join("terrain").join("m.ron").exists());
}
