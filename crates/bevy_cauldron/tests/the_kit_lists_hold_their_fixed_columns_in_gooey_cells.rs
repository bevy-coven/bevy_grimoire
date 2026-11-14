//! **The Kit lists' fixed columns are gooey `cell`s, not hand-sized `Node`s** (UI-59, #245).
//!
//! The pack heading's fold chevron, the candidate rows' severity mark and the gap inside a detail line were each a text or an empty box
//! with a `Node { width: .., flex_shrink: 0 }` of their own. They are `chrome::text_cell` and `chrome::gap_cell` now, which are gooey's
//! `cell` (a width in body text sizes), so they grow with the person's text size, and the raw-`Node` ratchet
//! (`the_kit_panels_are_built_from_gooey_widgets.rs`) has four fewer to count.
//!
//! This file reads what a cell does on screen, on the one of the four a bare project draws: the **gap between the two label/value pairs
//! of a selected piece's detail line** is an empty gooey cell as wide as a group gap. (The chevrons and the severity mark are drawn only for
//! candidate meshes a scan has found, which needs files outside the fixture's library; the ratchet counts them.)

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::chrome::GAP_GROUP;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::{ImportState, Mode};
use bevy_gooey::Role;

use crate::fixtures;

/// Frames the editor is given to draw the detail pane: it waits on the thumbnail bake, which is paced by the clock.
const SETTLE_FRAMES: usize = 80;

/// How far a cell's laid-out width may stand from the column it stands for, logical px: layout is physical and rounds.
const WIDTH_SLACK_PX: f32 = 2.0;

/// **A selected piece's detail line spaces its two pairs with an empty gooey cell the width of a group gap.**
#[test]
fn the_detail_lines_gap_is_an_empty_gooey_cell_as_wide_as_a_group_gap() {
    let root = fixtures::Fixture::new(&format!("kit_cells_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<ImportState>().selected_library_id = Some("site/wall".to_owned());
    for _ in 0..SETTLE_FRAMES {
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update();
    }
    let world = app.world_mut();
    let mut cells = world.query::<(&Role, &ComputedNode, Option<&Children>)>();
    let empty: Vec<f32> = cells
        .iter(world)
        .filter(|(role, _, kids)| **role == Role::Cell && kids.is_none_or(|k| k.is_empty()))
        .map(|(_, node, _)| node.size().x * node.inverse_scale_factor())
        .collect();
    assert!(
        empty.iter().any(|w| (w - GAP_GROUP).abs() <= WIDTH_SLACK_PX),
        "no empty gooey cell as wide as a group gap ({GAP_GROUP} px) is drawn in the detail line: the empty cells are {empty:?} px wide"
    );
}
