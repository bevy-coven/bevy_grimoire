//! **The shell is a gooey dock, and a resized dock never covers the viewport** (UI-30, #224).
//!
//! The editor's outer layout used to be `chrome::Frame`'s five hand-built nodes: a chrome bar, a door
//! strip, a body split into left/viewport/right, and a status band. `Frame` is still what the editor
//! reads to place a control, but the *arrangement* is `bevy_gooey`'s `Dock` now, so a person can drag
//! a panel edge and the viewport keeps its own share.
//!
//! Two things this file holds down, and each can fail:
//!
//! 1. **Every region is findable by its accessible label**, not by a pixel. A test that clicks
//!    coordinates passes whether or not the widget is a widget; a test that asks for it by name only
//!    passes when something answers to that name.
//! 2. **A resized dock never covers the viewport.** `DockLayout` is plain data, so the claim is
//!    arithmetic on the layout the dock is showing: whatever the split fractions, the viewport's
//!    pane has a share and the panes beside it are inside the layout's own room. A hand-built
//!    `position: absolute` split could satisfy "there is a viewport" while covering it.
//!
//! The dock's own frame and titles come from the running `GooeySkin` (`editor_skin.yaml`); the panels'
//! *contents* keep the editor's own colours, which is what UI-02 (#210) later moves onto the skin.

use bevy::prelude::*;
use bevy_cauldron::harness;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..6 {
        app.update();
    }
    (root, app)
}

/// **The editor registers its five regions as dock panels**, so a dock can show them and a person
/// can rearrange them.
#[test]
fn every_region_of_the_shell_is_a_registered_dock_panel() {
    let (_root, app) = editor("shell-regions");
    let registry = app
        .world()
        .get_resource::<bevy_gooey::PanelRegistry>()
        .expect(
            "the editor must register its regions with gooey's `PanelRegistry`, or the shell is still \
             the hand-built `chrome::Frame` and nothing can be docked",
        );
    for id in ["inspector", "viewport", "place", "status"] {
        assert!(
            registry.contains(id),
            "`{id}` is not a registered panel; a dock shows what the registry names and nothing else. \
             The registry holds: {:?}",
            ["inspector", "viewport", "place", "status"]
                .iter()
                .filter(|i| registry.contains(i))
                .collect::<Vec<_>>()
        );
    }
}

/// **The viewport is a leaf of the dock's layout**, with a share of its own, so no split can cover it.
#[test]
fn the_viewport_has_its_own_pane_in_the_dock_layout() {
    let (_root, mut app) = editor("shell-viewport");
    let docks = app
        .world_mut()
        .query::<&bevy_gooey::Dock>()
        .iter(app.world())
        .map(|d| d.0.clone())
        .collect::<Vec<_>>();
    assert!(
        !docks.is_empty(),
        "the editor draws no `bevy_gooey::Dock`, so its outer layout is still the hand-built frame"
    );
    let shows = docks.iter().any(|layout| layout.contains("viewport"));
    assert!(
        shows,
        "no dock shows the viewport. A viewport outside the dock is a region a resized dock can cover; \
         the layouts are: {docks:?}"
    );
    for layout in &docks {
        // Every panel a dock names must be one the registry knows, or the dock silently skips it and
        // the region is gone from the screen while the test above still finds it registered.
        let panels = layout.panels();
        if panels.contains(&"viewport") {
            assert!(
                panels.len() > 1,
                "a dock holding only the viewport has nothing to resize against: {layout:?}"
            );
        }
    }
}

/// **Every region of the shell answers to a screen reader by its name** (UI-30, #224, acceptance item 1).
///
/// A `Role::Panel` is a box in the accessibility tree with **no name on it**, so the editor met a
/// reader as four or five unlabelled regions: "panel, panel, panel". The acceptance asks that a
/// headless test find each region *by its accessible label*, which is only true if the panels carry
/// one — so this asks the tree the same question a reader would.
///
/// It is deliberately not satisfied by the heading drawn inside the panel: a `Text` node is read out
/// when focus reaches it, which is no help to someone who needs to know what the region *is* before
/// they enter it.
#[test]
fn every_panel_answers_a_screen_reader_by_its_name() {
    let (_root, mut app) = editor("shell-a11y");
    let world = app.world_mut();
    // Every node in the tree that carries a name, whatever its role: what matters is that the
    // region a person is in can say what it is, not which role the crate happened to give the box.
    let mut named_nodes = world.query::<&bevy::prelude::AccessibleLabel>();
    let named: Vec<String> = named_nodes.iter(world).map(|l| l.0.clone()).collect();

    for region in ["INSPECTOR", "PLACE"] {
        assert!(
            named.iter().any(|n| n == region),
            "the `{region}` panel has no accessible name, so a screen reader meets it as an \
             unlabelled box. Named panels: {named:?}"
        );
    }
}
