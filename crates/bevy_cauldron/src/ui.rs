//! **The one plugin that owns the editor's UI library** (UI-01, #209).
//!
//! The editor's UI is `bevy_gooey`, in every build. [`EditorUiPlugin`] is the only place that adds
//! gooey's plugins, so a later step of the UI series (a theme, a command palette, a menu bar) adds
//! its gooey plugin here and nowhere else.

use bevy::prelude::*;
use bevy_gooey::{GooeyNavPlugin, GooeySkin, GooeyStylePlugin};

/// The editor's own skin, as data: `src/editor_skin.ron`.
const EDITOR_SKIN: &str = include_str!("editor_skin.ron");

/// **The editor's look as `bevy_gooey`'s own type** (UI-02, #210): the colours and measures
/// `chrome.rs` draws with, the elevation ladder in `surfaces` and the sizes a compact tool wants.
/// `GooeySkin::problems()` is the check that the ladder is a ladder and the ink is readable.
pub fn editor_skin() -> GooeySkin {
    crate::ui_skin::from_ron(EDITOR_SKIN).unwrap_or_else(|e| panic!("src/editor_skin.ron does not read: {e}"))
}

/// Adds gooey's style plugin and its navigation plugin, puts the editor's skin in as the running one and adds the system that applies the project's skin file ([`crate::ui_skin::apply_ui_skin`]).
pub struct EditorUiPlugin;

impl Plugin for EditorUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<GooeyStylePlugin>() {
            app.add_plugins(GooeyStylePlugin);
        }
        // **Menus need it** (UI-36, #230): gooey registers the context-menu click, the popovers' anchoring
        // and their closing in the navigation plugin. The editor's arrow keys stay its own because gooey's
        // only act on widgets that opt in (`AutoDirectionalNavigation`), and the viewport is not one.
        if !app.is_plugin_added::<GooeyNavPlugin>() {
            app.add_plugins(GooeyNavPlugin);
        }
        // **The settings screen** (UI-71, #249): Cmd/Ctrl+, over any screen.
        app.add_plugins((crate::settings_screen::SettingsScreenPlugin, crate::project_config::ProjectConfigPlugin, crate::keymap_screen::KeymapScreenPlugin));
        app.insert_resource(editor_skin());
        // **The shell's regions as gooey panels** (UI-30, #224). A `Dock` shows only what a
        // `PanelRegistry` names, so the regions the editor draws are registered here — the one place
        // that owns the UI library — and the dock that arranges them is spawned in
        // `chrome::frame`, where the outer layout already lives.
        //
        // The contents keep the editor's own colours for now; UI-02 (#210) moves those onto the skin.
        // What changes here is the *arrangement*: a region can be dragged and resized by gooey rather
        // than by a hand-built flex row nothing can move.
        app.add_systems(PreStartup, register_the_shell_panels);
        // **`K` is the reveal, and gooey owns the rule** (UI-41, #235; bevy-coven/bevy_gooey#36).
        //
        // `ShortcutBadges` is ExposeHK's overlay, and until G-32 it could only be shown by a
        // *modifier* held exactly — `Modifiers::held(keys) == hold`. This editor's overlay key is
        // `K`, a plain key (`keys::Action::Shortcuts`, and `a_key_still_fires_while_k_is_held` pins
        // that `K`+verb still fires), so its own `keys::pressed` poll was the only way to reveal it.
        // `Reveal::Key` says the same thing in the crate, and `BadgesShown` reports it back.
        //
        // **Zero delay**: `K` is not a chord prefix but the overlay's own key, and the 0.3 s default
        // exists to stop a modifier flashing the screen on the way to `Ctrl+S`. Nothing types `K`
        // to reach a chord, so the delay would only make the overlay feel slow.
        app.insert_resource(bevy_gooey::ShortcutBadges {
            hold: bevy_gooey::Reveal::Key(bevy::input::keyboard::KeyCode::KeyK),
            delay_seconds: 0.0,
        });
        // **Refusals stand at the top of the viewport** (UI-39, #233), where the card they replace stood.
        app.insert_resource(bevy_gooey::ToastSettings { anchor: bevy_gooey::ToastAnchor::TopCenter, ..default() });
        app.add_systems(
            Update,
            crate::ui_skin::apply_ui_skin
                .after(crate::family_host::Reloaded)
                .before(bevy_gooey::GooeySystems::Style),
        );
    }
}

/// **The regions of the shell, as `bevy_gooey` panels** (UI-30, #224).
///
/// Each is registered with the id a dock layout names. A `Dock` shows what the registry holds when it
/// is built and skips anything it cannot find (`DockPanelSkipped`), so this runs before the frame.
///
/// **Registered once, at `PreStartup`.** The spawn closure fills the panel; a region whose contents
/// the editor draws elsewhere — the viewport is a texture — gets the node that holds them, so the
/// dock's own arrangement is what a person drags.
fn register_the_shell_panels(mut registry: ResMut<bevy_gooey::PanelRegistry>) {
    for (id, title) in [
        ("inspector", "INSPECTOR"),
        ("viewport", "VIEWPORT"),
        ("place", "PLACE"),
        ("status", "STATUS"),
    ] {
        registry.register(id, title, move |c| {
            c.insert(bevy_gooey::DockPanel(id.to_owned()));
        });
    }
}

/// **The shell's arrangement**, as a dock layout a divider can move (UI-30, #224).
///
/// A left dock, the viewport in the middle with the most room, and a right dock — the shape the
/// hand-built frame has had since 2026-08-17, now data that gooey keeps and a person edits.
pub fn shell_layout() -> bevy_gooey::DockLayout {
    use bevy_gooey::{DockLayout, SplitAxis};
    DockLayout::split(
        SplitAxis::Horizontal,
        0.22,
        DockLayout::group(["inspector"]),
        DockLayout::split(
            SplitAxis::Horizontal,
            0.78,
            DockLayout::group(["viewport"]),
            DockLayout::group(["place"]),
        ),
    )
}
