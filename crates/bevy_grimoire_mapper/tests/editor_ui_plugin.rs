//! **The editor's UI is gooey, in every build, and one plugin owns it** (UI-01, #209).
//!
//! `bevy_gooey` was an optional feature, so a default build had no skin, no style plugin and no
//! `ui_skin` tests. It is now required: a headless editor on default features carries the running
//! skin, `family.ron` may list the crate without a problem, and `EditorUiPlugin` is the one plugin
//! that added gooey's plugins.

use bevy::prelude::*;
use bevy_gooey::GooeySkin;
use bevy_grimoire_mapper::ui::EditorUiPlugin;

use crate::fixtures;

#[test]
fn a_default_build_has_the_gooey_skin_and_one_plugin_that_owns_it() {
    let root = fixtures::Fixture::new(&format!("editor_ui_plugin_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));

    assert!(app.is_plugin_added::<EditorUiPlugin>(), "EditorUiPlugin is part of the editor");
    assert!(app.is_plugin_added::<bevy_gooey::GooeyStylePlugin>(), "gooey's style plugin came with it");
    assert!(app.world().get_resource::<GooeySkin>().is_some(), "the running skin exists with no cargo feature");
    assert!(bevy_grimoire_mapper::family_host::built().contains(&"bevy_gooey"), "the build says it has gooey");

    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_gooey\"])").unwrap();
    assert!(
        bevy_grimoire_mapper::family_host::problems_with(&root, &bevy_grimoire_mapper::family_host::built()).is_empty(),
        "a project may list bevy_gooey in a default build"
    );
}
