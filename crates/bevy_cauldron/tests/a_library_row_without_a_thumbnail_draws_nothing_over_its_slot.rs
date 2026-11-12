//! **A library row with no thumbnail yet draws nothing over its slot** (UI-59, #245, review of the slot's gooey `image_box`).
//!
//! The slot's fill (`slot_bg`) is what a row shows until its portrait is baked. gooey's `image_box` carries an `ImageNode`,
//! and Bevy's default image handle is a real 1x1 white image, not nothing: a slot handed `Handle::default()` painted white over
//! its fill on every unbaked row. The slot is given `TRANSPARENT_IMAGE_HANDLE`, which the UI renderer skips, until a portrait
//! exists, and the baked image itself after.

use bevy::image::TRANSPARENT_IMAGE_HANDLE;
use bevy::prelude::*;
use bevy_cauldron::thumbs::Thumbnails;
use bevy_cauldron::tiles::{ImportState, LibraryRow, Mode};
use bevy_gooey::Role;

use crate::fixtures;

#[test]
fn an_unbaked_slot_holds_the_transparent_image_not_the_default_white_one() {
    let root = fixtures::Fixture::new(&format!("library_slot_transparent_{}", std::process::id()))
        .descriptor("lamp_tall", "alpha")
        .descriptor("bed", "alpha")
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    // **A library piece selected puts the Meshes tab on its library shelf**, where each piece is a row with a slot.
    app.world_mut().resource_mut::<ImportState>().selected_library_id = Some("lamp_tall".to_owned());
    for _ in 0..12 {
        app.update();
    }
    let world = app.world_mut();
    let rows: Vec<(Entity, String)> = world.query::<(Entity, &LibraryRow)>().iter(world).map(|(entity, row)| (entity, row.0.clone())).collect();
    assert!(!rows.is_empty(), "the Meshes shelf of the library has rows for the fixture's pieces");
    let mut slots = 0;
    for (row, id) in rows {
        // **The baked portrait when there is one, and nothing at all when there is not**: the slot's image is what the
        // thumbnails hold for the piece, or the transparent image the renderer skips; never the default white pixel.
        let want = world.get_resource::<Thumbnails>().and_then(|thumbs| thumbs.image(&id)).unwrap_or(TRANSPARENT_IMAGE_HANDLE);
        let Some(kids) = world.get::<Children>(row) else { continue };
        for kid in kids.iter() {
            if world.get::<Role>(kid) == Some(&Role::ImageBox) {
                slots += 1;
                let image = world.get::<ImageNode>(kid).expect("an image box carries an ImageNode");
                assert_eq!(image.image, want, "the slot of `{id}` holds its portrait, or the transparent image until it is baked");
                assert_ne!(image.image, Handle::<Image>::default(), "and never the default handle, a white pixel over the fill");
            }
        }
    }
    assert!(slots > 0, "every library row has its thumbnail slot");
}
