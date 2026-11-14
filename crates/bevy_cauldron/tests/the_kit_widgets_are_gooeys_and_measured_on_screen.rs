//! **The Kit panels' last raw `Node`s are gooey's widgets, and each one is measured on screen** (UI-59, #245).
//!
//! `the_kit_panels_are_built_from_gooey_widgets.rs` counts the literals; this file reads what the swap did to the
//! laid-out panel, because a count cannot see a **white square** (a transparent handle for an empty image slot, caught in
//! review of an earlier slice) or a hit target that shrank. Each test names the widget a hand-built node became and then
//! measures it in a real headless layout:
//!
//! - the lattice's header and cell buttons are `glyph_button`s whose box is at least what the hand-built ones were
//!   (20 x [`MIN_FIELD_H`] px) and at least gooey's target on each side, and a picked cell is lit;
//! - a pack heading is a `band`, as wide as its list, whose tally stays inside it, and the heading the cursor stands on is lit;
//! - the workspace's tabs are `edge_tab`s, the active panel's one `Selected`;
//! - the Compose pane's lines still wrap inside their own column (the pair the 2026-08-17 measurement chose).

use bevy::prelude::*;
use bevy::ui::{ComputedNode, Selected};
use bevy_cauldron::chrome::MIN_FIELD_H;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::{CellButton, CellEdit, CellGlyph, CellLayer, FillHeader, ImportState, Mode, PackHeader, Tab, TabLabel};
use bevy_gooey::{GooeySkin, PrimaryButton, Role};

use crate::fixtures;

/// Frames the editor is given to draw a detail pane: it waits on the thumbnail bake, which is paced by the clock.
const SETTLE_FRAMES: usize = 80;

/// How far a laid-out size may stand under the size it stands for, logical px: layout is physical and rounds.
const SLACK_PX: f32 = 1.0;

/// The narrowest a hand-built lattice button was, logical px (`min_width` of the `Node` it replaces).
const OLD_CELL_MIN_W_PX: f32 = 20.0;

/// The fewest logical px a wrapped line of prose may be wide: under it every row lays out one glyph per line.
const LEAST_LINE_W_PX: f32 = 50.0;

/// A node's laid-out size in logical px: the window's scale and `UiScale` taken out.
fn size_of(world: &World, node: &ComputedNode) -> Vec2 {
    let ui = world.get_resource::<UiScale>().map_or(1.0, |u| u.0);
    node.size() * node.inverse_scale_factor() / ui
}

/// Where a node's right edge is, in the window's physical px: its centre and half its width, which are in one space.
fn right_edge(node: &ComputedNode, at: &UiGlobalTransform) -> f32 {
    at.translation.x + node.size().x * 0.5
}

/// A piece with a hand-authored lattice, selected, so the Meshes pane draws its subgrid.
fn lattice(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("kit_widgets_lattice_{tag}_{}", std::process::id()))
        .authored_lattice("site/wall", "alpha", "wall", 3)
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<ImportState>().selected_library_id = Some("site/wall".to_owned());
    for _ in 0..SETTLE_FRAMES {
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update();
    }
    app
}

/// Two packs of candidate meshes, so the Meshes pane draws a heading band for each.
fn packs(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("kit_widgets_packs_{tag}_{}", std::process::id()))
        .descriptor("wall", "alpha")
        .pack("aaa", &["a1", "a2", "a3"])
        .pack("bbb", &["b1", "b2", "b3"])
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..10 {
        app.update();
    }
    app
}

/// **The lattice's header and cell buttons are gooey glyph buttons, no smaller than what they replace.**
#[test]
fn the_lattice_buttons_are_glyph_buttons_no_smaller_than_the_hand_built_ones() {
    let mut app = lattice("glyphs");
    let world = app.world_mut();
    // **The skin's own target**, which is what a glyph button's box is on each side whatever its glyph.
    // It is held in screen px (WCAG 2.2 SC 2.5.8), so the box is compared after the editor's `UiScale`, which the skin's own px do not carry.
    let target = world.get_resource::<GooeySkin>().map_or(OLD_CELL_MIN_W_PX, GooeySkin::target);
    let ui_scale = world.get_resource::<UiScale>().map_or(1.0, |u| u.0);
    let mut q = world.query::<(Option<&Role>, &ComputedNode, Has<CellButton>, Has<FillHeader>)>();
    let found: Vec<(Option<Role>, Vec2, bool)> = q
        .iter(world)
        .filter(|(_, _, cell, header)| *cell || *header)
        .map(|(role, node, cell, _)| (role.copied(), size_of(world, node), cell))
        .collect();
    assert!(found.iter().any(|f| f.2), "the lattice drew no cell buttons: {found:?}");
    assert!(found.iter().any(|f| !f.2), "the lattice drew no header buttons: {found:?}");
    for (role, size, cell) in &found {
        let what = if *cell { "cell button" } else { "header button" };
        assert_eq!(*role, Some(Role::GlyphButton), "a {what} is gooey's `glyph_button`, not a `Button` and a `Node` of the editor's own");
        assert!(
            size.x + SLACK_PX >= OLD_CELL_MIN_W_PX && size.y + SLACK_PX >= MIN_FIELD_H,
            "a {what} is {size:?} px on screen, smaller than the {OLD_CELL_MIN_W_PX} x {MIN_FIELD_H} px box it replaces"
        );
        assert!(
            size.x * ui_scale + SLACK_PX >= target && size.y * ui_scale + SLACK_PX >= target,
            "a {what} is {size:?} px (x {ui_scale} UI scale) on screen, under the skin's {target} px target on a side"
        );
    }
}

/// **Every cell's glyph is its button's own words**, found by the pair of components the repaint reads them by, and drawn.
#[test]
fn every_cell_button_holds_one_glyph_the_repaint_can_find() {
    let mut app = lattice("words");
    let world = app.world_mut();
    let cells = world.query::<&CellButton>().iter(world).count();
    let mut glyphs = world.query::<(&CellGlyph, &CellLayer, &Text, &ComputedNode)>();
    let drawn: Vec<(String, f32)> = glyphs.iter(world).map(|(_, _, t, n)| (t.0.clone(), n.size().x)).collect();
    assert!(cells > 0, "no cell buttons are drawn");
    assert_eq!(drawn.len(), cells, "one glyph per cell button: {drawn:?}");
    assert!(
        drawn.iter().all(|(text, width)| !text.is_empty() && *width > 0.0),
        "a cell's glyph is empty or has no width on screen (the white-square class of regression): {drawn:?}"
    );
}

/// **The picked cell is lit, and picking another moves the light.**
#[test]
fn the_picked_cell_is_lit_and_picking_another_moves_the_light() {
    let mut app = lattice("pick");
    let cells: Vec<(Entity, u32, u32, u32)> = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &CellButton, &CellLayer)>();
        let mut cells: Vec<_> = q.iter(world).map(|(e, b, l)| (e, b.0, l.0, b.1)).collect();
        cells.sort_by_key(|c| (c.2, c.3, c.1));
        cells
    };
    assert!(cells.len() >= 2, "the lattice needs two cells to move a pick between: {}", cells.len());
    let lit = |app: &mut App| -> Vec<Entity> {
        let world = app.world_mut();
        world.query_filtered::<Entity, (With<CellButton>, With<PrimaryButton>)>().iter(world).collect()
    };
    for (pick, name) in [(cells[0], "first"), (cells[1], "second")] {
        app.world_mut().commands().trigger(bevy_gooey::Activate { entity: pick.0 });
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(
            app.world().resource::<CellEdit>().at,
            Some((pick.1, pick.2, pick.3)),
            "a click on the {name} cell picks it"
        );
        assert_eq!(lit(&mut app), vec![pick.0], "exactly the {name} cell, the one picked, is lit");
    }
}

/// **A pack heading is a gooey band, as wide as its list, and its tally stays inside it.**
#[test]
fn a_pack_heading_is_a_band_as_wide_as_its_list_and_its_tally_stays_inside() {
    let mut app = packs("band");
    let world = app.world_mut();
    let mut headings = world.query::<(Entity, &PackHeader, Option<&Role>, &ComputedNode, &UiGlobalTransform, &ChildOf)>();
    let found: Vec<_> = headings
        .iter(world)
        .map(|(e, h, role, n, t, parent)| (e, h.pack().to_owned(), role.copied(), size_of(world, n), right_edge(n, t), parent.parent()))
        .collect();
    assert!(found.len() >= 2, "a heading per pack, aaa and bbb: {found:?}");
    for (band, pack, role, size, band_right, parent) in found {
        assert_eq!(role, Some(Role::Band), "the `{pack}` heading is gooey's `band`");
        // The list's own content box: a band is as wide as the room its host gives it, which is the host less its padding and border.
        let list = world
            .get::<ComputedNode>(parent)
            .map(|n| {
                let (padding, border) = (n.padding(), n.border());
                let inset = padding.min_inset.x + padding.max_inset.x + border.min_inset.x + border.max_inset.x;
                size_of(world, n) - Vec2::new(inset * n.inverse_scale_factor(), 0.0)
            })
            .unwrap_or_default();
        assert!(size.y > 0.0, "the `{pack}` band has no height on screen");
        assert!(
            size.x + SLACK_PX >= list.x,
            "the `{pack}` band is {} px wide, narrower than its list's content box ({} px): a band is as wide as its host",
            size.x,
            list.x
        );
        let mut parts = world.query::<(&Role, &ComputedNode, &UiGlobalTransform, &ChildOf)>();
        let tally = parts
            .iter(world)
            .find(|(role, _, _, parent)| **role == Role::CountText && parent.parent() == band)
            .map(|(_, n, t, _)| right_edge(n, t))
            .unwrap_or_else(|| panic!("the `{pack}` band holds no gooey `count_text`"));
        assert!(
            parts.iter(world).any(|(role, _, _, parent)| *role == Role::FillText && parent.parent() == band),
            "the `{pack}` band holds no gooey `fill_text` for its name"
        );
        let tally_right = tally;
        assert!(
            tally_right <= band_right + SLACK_PX,
            "the `{pack}` band's tally ends at {tally_right} px, outside the band (ends {band_right}): a count never widens its row"
        );
    }
}

/// **The heading the cursor stands on is lit, and only that one.**
#[test]
fn the_heading_the_cursor_stands_on_is_lit() {
    let mut app = packs("armed");
    app.world_mut().resource_mut::<ImportState>().focused_pack = Some("bbb".to_owned());
    for _ in 0..4 {
        app.update();
    }
    let world = app.world_mut();
    let mut q = world.query::<(&PackHeader, Has<PrimaryButton>)>();
    let state: Vec<(String, bool)> = q.iter(world).map(|(h, lit)| (h.pack().to_owned(), lit)).collect();
    assert!(state.contains(&("bbb".to_owned(), true)), "the focused heading is lit: {state:?}");
    assert!(state.contains(&("aaa".to_owned(), false)), "and the other is not: {state:?}");
}

/// **The workspace's tabs are gooey edge tabs, the active panel's one `Selected`**, and moving the panel moves it.
#[test]
fn the_tabs_are_edge_tabs_and_the_active_panel_is_the_selected_one() {
    let mut app = packs("tabs");
    for mode in [Mode::Meshes, Mode::Tiles] {
        *app.world_mut().resource_mut::<Mode>() = mode;
        for _ in 0..4 {
            app.update();
        }
        let world = app.world_mut();
        let mut q = world.query::<(&Tab, Option<&Role>, &ComputedNode, Has<Selected>)>();
        let tabs: Vec<(Mode, Option<Role>, Vec2, bool)> = q.iter(world).map(|(t, r, n, s)| (t.0, r.copied(), size_of(world, n), s)).collect();
        assert!(tabs.len() >= 2, "the Kit door's strip has its panels: {}", tabs.len());
        for (tab, role, size, selected) in &tabs {
            assert_eq!(*role, Some(Role::EdgeTab), "the `{}` tab is gooey's `edge_tab`", tab.label());
            assert!(size.x > 0.0 && size.y > 0.0, "the `{}` tab has no size on screen: {size:?}", tab.label());
            assert_eq!(*selected, *tab == mode, "`{}` is `Selected` exactly while it is the panel shown ({})", tab.label(), mode.label());
        }
        let mut labels = world.query::<(&TabLabel, &Text, &ComputedNode)>();
        let drawn = labels.iter(world).filter(|(_, t, n)| !t.0.is_empty() && n.size().x > 0.0).count();
        assert_eq!(drawn, tabs.len(), "each tab's label is drawn");
    }
}

/// **The Compose pane's lines wrap inside their own column.** The flex pair a raw `Node` carried is the editor's own
/// `fill_text` now; without it the text's flex base resolved to zero width and every row laid out one glyph per line.
#[test]
fn the_compose_panes_lines_wrap_inside_their_own_column() {
    let root = fixtures::Fixture::new(&format!("kit_widgets_compose_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .composition("town/house_cottage_plaster", &[("site/wall", "site/wall", (0.0, 0.0))])
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Compose).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..10 {
        app.update();
    }
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &ComputedNode)>();
    let long: Vec<(String, Vec2)> = q
        .iter(world)
        .filter(|(t, n)| t.0.chars().count() > 12 && n.size().x > 0.0)
        .map(|(t, n)| (t.0.clone(), size_of(world, n)))
        .collect();
    assert!(!long.is_empty(), "the Compose pane drew no line of prose");
    for (text, size) in long {
        assert!(
            size.x >= LEAST_LINE_W_PX,
            "`{text}` is {} px wide: a line laid out one glyph per line has lost its flex measure",
            size.x
        );
    }
}
