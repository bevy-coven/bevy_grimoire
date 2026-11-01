//! **The one plugin that owns the editor's UI library** (UI-01, #209).
//!
//! The editor's UI is `bevy_gooey`, in every build. [`EditorUiPlugin`] is the only place that adds
//! gooey's plugins, so a later step of the UI series (a theme, a command palette, a menu bar) adds
//! its gooey plugin here and nowhere else.

use bevy::prelude::*;
use bevy_gooey::{GooeySkin, GooeyStylePlugin};

/// The editor's own skin, as data: `src/editor_skin.ron`.
const EDITOR_SKIN: &str = include_str!("editor_skin.ron");

/// **The editor's look as `bevy_gooey`'s own type** (UI-02, #210): the colours and measures
/// `chrome.rs` draws with, the elevation ladder in `surfaces` and the sizes a compact tool wants.
/// `GooeySkin::problems()` is the check that the ladder is a ladder and the ink is readable.
pub fn editor_skin() -> GooeySkin {
    crate::ui_skin::from_ron(EDITOR_SKIN).unwrap_or_else(|e| panic!("src/editor_skin.ron does not read: {e}"))
}

/// Adds gooey's style plugin (not its navigation plugin: the editor owns the arrow keys), puts the
/// editor's skin in as the running one and adds the system that applies the project's skin file ([`crate::ui_skin::apply_ui_skin`]).
pub struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<GooeyStylePlugin>() {
            app.add_plugins(GooeyStylePlugin);
        }
        app.insert_resource(editor_skin());
        app.add_systems(
            Update,
            crate::ui_skin::apply_ui_skin
                .after(crate::family_host::Reloaded)
                .before(bevy_gooey::GooeySystems::Style),
        );
    }
}
