//! **A mesh the render world took does not crash the triangle count** (#193).
//!
//! `bevy_elements` draws its stars, its moon and its lightning from meshes whose `asset_usage` is
//! `RENDER_WORLD` only, so once the first frame is extracted the main world's copy has no vertex or
//! index data, and `Mesh::indices()` panics on it. `refresh_triangle_total` (the `N tris drawn` readout)
//! asked every visible mesh for its indices, so the editor aborted on `big` the moment a project's
//! `family/bevy_elements.yaml` dressed the viewport. The readout counts what the main world can still
//! read and skips what it cannot.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::ViewVisibility;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

use crate::fixtures;

fn triangle(usage: RenderAssetUsages) -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, usage)
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2]))
}

/// The number the `N tris drawn` readout says.
fn readout(app: &mut App) -> usize {
    let mut q = app.world_mut().query_filtered::<&Text, With<bevy_cauldron::editor::TriangleTotal>>();
    let said = q.single(app.world()).expect("one triangle readout").0.clone();
    said.split(" tris").next().unwrap().replace(',', "").parse().unwrap_or_else(|_| panic!("`{said}` is not a count"))
}

#[test]
fn a_visible_mesh_the_render_world_took_is_skipped_and_the_rest_still_counts() {
    let root = fixtures::Fixture::new("tris_extracted").descriptor("floor", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The count is behind View -> Show statistics (#302); this asks for it.
    let file = std::env::temp_dir().join(format!("grimoire_stats_tris_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("a scratch folder");
    std::fs::write(&file, "{version: 1, show_statistics: true}").expect("a settings file");
    app.insert_resource(bevy_cauldron::settings::UserSettingsFile(file));
    for _ in 0..80 {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(15));
        if app.world().resource::<bevy_cauldron::settings::EditorSettings>().show_statistics {
            break;
        }
    }
    for _ in 0..3 {
        app.update();
    }

    let before = readout(&mut app);
    let mut taken = triangle(RenderAssetUsages::RENDER_WORLD);
    taken.take_gpu_data().expect("the first extraction succeeds");
    assert!(taken.try_indices_option().is_err(), "the fixture mesh is one the main world can no longer read");
    let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
    let taken = meshes.add(taken);
    let readable = meshes.add(triangle(RenderAssetUsages::default()));
    app.world_mut().spawn((Mesh3d(taken), ViewVisibility::VISIBLE));
    app.world_mut().spawn((Mesh3d(readable), ViewVisibility::VISIBLE));

    app.update();
    assert_eq!(readout(&mut app), before + 1, "the readable triangle counts and the taken mesh adds nothing");
}
