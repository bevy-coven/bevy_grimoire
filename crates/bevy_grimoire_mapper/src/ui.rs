//! **The one plugin that owns the editor's UI library** (UI-01, #209).
//!
//! The editor's UI is `bevy_gooey`, in every build. [`EditorUiPlugin`] is the only place that adds
//! gooey's plugins, so a later step of the UI series (a theme, a command palette, a menu bar) adds
//! its gooey plugin here and nowhere else.

use bevy::prelude::*;
use bevy_gooey::GooeyStylePlugin;

/// Adds gooey's style plugin (not its navigation plugin: the editor owns the arrow keys) and the
/// system that applies the project's skin file ([`crate::ui_skin::apply_ui_skin`]).
pub struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<GooeyStylePlugin>() {
            app.add_plugins(GooeyStylePlugin);
        }
        app.add_systems(
            Update,
            crate::ui_skin::apply_ui_skin
                .after(crate::family_host::Reloaded)
                .before(bevy_gooey::GooeySystems::Style),
        );
    }
}
