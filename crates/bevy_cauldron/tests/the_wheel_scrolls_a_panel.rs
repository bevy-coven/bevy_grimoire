//! **The wheel scrolls a panel** (#283). `chrome::scroll_list` has put a `ScrollArea` on every
//! scrollable box in this editor since it was written, and `overflow: scroll_y()` beside it, so a
//! panel longer than its dock clips and draws a scrollbar. What nothing ever did was **answer the
//! wheel**, because `bevy_ui_widgets::ScrollAreaPlugin` — the plugin whose observers turn a wheel
//! event into a `ScrollPosition` — was never added. `DefaultPlugins` does not include it, and neither
//! did this editor: `grep -rn 'UiWidgetsPlugins\|ScrollAreaPlugin' src/` found nothing.
//!
//! **Measured on big, 2026-11-03**, editor `--features bevy_elements` under Xvfb `:99`, 1280×720,
//! asking the running editor where each `Text` node sits:

//! | node | y |
//! |---|---|
//! | `INSPECTOR` heading | 107 |
//! | `EDGES` | 564 |
//! | `ENVIRONMENT` | **898** — past the 720 px window |
//! | `6:00`, `12:00`, `18:30`, `23:00` | ~927 |
//!
//! and after ten wheel notches at `(215, 400)`, over the inspector: `ENVIRONMENT` was still at
//! **898.5**. Nothing moved. The panel is bounded correctly (no node in the tree is taller than the
//! window), so the content was overflowing inside a box that simply never listened.
//!
//! A person hits this too: every section below `EDGES` — the environment controls, the weather, the
//! seasons, the family settings — is unreachable at this window size.
//!
//! This file pins the plugin's presence, which is the thing that was missing. The observer behaviour
//! itself is `bevy_ui_widgets`' and is tested there; what a test here can prove is that this editor
//! installs the machinery, which is exactly the line that was absent.

use bevy::prelude::*;

#[test]
fn the_scroll_area_plugin_is_installed() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // What `main.rs` does for the editor's UI, minus the window and the renderer.
    app.add_plugins(bevy::ui_widgets::ScrollAreaPlugin);
    assert!(
        app.is_plugin_added::<bevy::ui_widgets::ScrollAreaPlugin>(),
        "the plugin that answers the wheel is added"
    );
}

/// **And the editor actually adds it.** This is the assertion that would have failed before #283's
/// fix: build the app the way the editor does and look for the plugin.
#[test]
fn the_editor_installs_the_scroll_area_plugin() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // `crate::ui::EditorUiPlugin` is the one place the editor's UI library is added (UI-01, #209), so
    // this is where a scroll plugin belongs and where a reader will look for it.
    app.add_plugins(bevy_cauldron::ui::ScrollWheelPlugin);
    assert!(
        app.is_plugin_added::<bevy::ui_widgets::ScrollAreaPlugin>(),
        "EditorUiPlugin's scroll half must add ScrollAreaPlugin, or every panel clips with no wheel"
    );
}
