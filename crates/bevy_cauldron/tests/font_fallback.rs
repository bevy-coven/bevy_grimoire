//! **A project does not have to carry the editor's font.**
//!
//! A game that only *uses* the editor (Sanctuary) has no `assets/fonts/FiraMono-Regular.ttf`, and
//! `install_font` used to refuse such a project outright, so the editor would not open on it. It now
//! falls back to the face compiled into the crate (a project's own file still wins).

use bevy::prelude::*;
use bevy::text::Font;
use bevy_cauldron::harness::install_font;

#[test]
fn a_project_without_the_font_still_gets_one() {
    let root = std::env::temp_dir().join(format!("bevy_grimoire_font_absent_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("assets")).unwrap_or_else(|e| panic!("{e}"));

    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), AssetPlugin::default())).init_asset::<Font>();
    install_font(&mut app, &root).unwrap_or_else(|e| panic!("a project with no font must boot: {e}"));
    assert!(
        app.world().resource::<Assets<Font>>().get(AssetId::default()).is_some(),
        "the default font id must hold the editor's face"
    );
    let _ = std::fs::remove_dir_all(&root);
}
