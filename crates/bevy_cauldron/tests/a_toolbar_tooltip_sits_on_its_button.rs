//! **A toolbar tooltip opens on its own button** (#290): centred under the control it describes, not a
//! hand's width to one side of it.
//!
//! The audit of 2026-11-04 found the tooltip drawn about 150 px left of the hovered icon, so a person
//! reading "Fill with the brush" could not tell which of three icons it belonged to. gooey places a
//! tooltip with Bevy's `Popover` below a widget whose parent lays out in a row, centred on it; this
//! reads the box and the button back from the laid-out UI and holds the centres within a few pixels.

use bevy::input_focus::{InputFocus, InputFocusVisible};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_cauldron::harness;

use crate::fixtures;

/// How far the box's centre may sit from the button's, in logical px.
const CENTRE_TOLERANCE_PX: f32 = 8.0;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// The centre of `entity` in logical px: layout is physical, and `inverse_scale_factor` undoes both
/// the window's scale and `UiScale`.
fn centre_x(app: &App, entity: Entity) -> f32 {
    let node = app.world().get::<ComputedNode>(entity).expect("laid out");
    let at = app.world().get::<UiGlobalTransform>(entity).expect("placed");
    at.translation.x * node.inverse_scale_factor
}

/// `entity`'s laid-out rectangle in logical px.
fn rect_of(app: &App, entity: Entity) -> Rect {
    let node = app.world().get::<ComputedNode>(entity).expect("laid out");
    let at = app.world().get::<UiGlobalTransform>(entity).expect("placed");
    Rect::from_center_size(at.translation * node.inverse_scale_factor, node.size() * node.inverse_scale_factor)
}

/// The UI's width in logical px: the editor's root node spans the whole window.
fn window_width(app: &App) -> f32 {
    let root = app.world().resource::<bevy_cauldron::chrome::Frame>().root;
    let node = app.world().get::<ComputedNode>(root).expect("the root is laid out");
    node.size().x * node.inverse_scale_factor
}

fn tooltip_of(app: &App, widget: Entity) -> Entity {
    app.world()
        .get::<Children>(widget)
        .and_then(|kids| {
            kids.iter().find(|k| app.world().get::<bevy_gooey::TooltipBox>(*k).is_some())
        })
        .expect("a button with a Tooltip owns a TooltipBox")
}

/// Opens `widget`'s tooltip and returns a complaint, or `None` when the box is on its button.
///
/// **Centred when it can be.** gooey centres the box under the control and, when a centred box would
/// leave the window (the first buttons of a bar, with a long chord), slides it along the control's edge
/// instead; so a box that could not be centred must still be inside the window and still stand over its
/// button's centre. Anything else — the audit's box a hand's width to one side — is a fault.
fn tooltip_complaint(app: &mut App, widget: Entity, what: &str) -> Option<String> {
    app.insert_resource(InputFocusVisible(true));
    app.world_mut().resource_mut::<InputFocus>().set(widget, bevy::input_focus::FocusCause::Navigated);
    for _ in 0..4 {
        app.update();
    }
    let tip = tooltip_of(app, widget);
    let shown = app.world().get::<Node>(tip).expect("a node").display != Display::None;
    if !shown {
        return Some(format!("{what}: focus on the button must open its tooltip"));
    }
    let (button_rect, tip_rect) = (rect_of(app, widget), rect_of(app, tip));
    // **Below the button, never beside it**: a box beside a toolbar button covers the buttons next to it.
    if tip_rect.min.y < button_rect.max.y - 1.0 {
        return Some(format!(
            "{what}: the tooltip {tip_rect:?} is not below its button {button_rect:?}, so it covers the neighbouring buttons"
        ));
    }
    let width = tip_rect.width();
    let window_width = window_width(app);
    let (button, tooltip) = (centre_x(app, widget), centre_x(app, tip));
    // gooey keeps a box one skin gap clear of the window's edge, so that is the room a centred box needs.
    let margin = app.world().resource::<bevy_gooey::GooeySkin>().gap;
    let centred_fits = button - width / 2.0 >= margin && button + width / 2.0 <= window_width - margin;
    let off = (button - tooltip).abs();
    if centred_fits {
        (off > CENTRE_TOLERANCE_PX).then(|| {
            format!(
                "{what}: the tooltip's centre is at x={tooltip:.1} and the button's at x={button:.1} (logical px), \
                 {off:.1} px apart; a centred box ({width:.0} px wide) fits the {window_width:.0} px window, so it \
                 should be centred within {CENTRE_TOLERANCE_PX}"
            )
        })
    } else {
        let (left, right) = (tooltip - width / 2.0, tooltip + width / 2.0);
        (left < -0.5 || right > window_width + 0.5 || button < left || button > right).then(|| {
            format!(
                "{what}: a centred box could not fit, but the slid one spans x={left:.0}..{right:.0} in a \
                 {window_width:.0} px window and the button's centre is at x={button:.0}"
            )
        })
    }
}

/// **Fill's one-shot button.**
#[test]
fn the_fill_tooltip_is_centred_on_the_fill_button() {
    let mut app = editor("tooltip-fill");
    harness::resize_surface(&mut app, 1280, 720).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let fill = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &bevy_cauldron::toolbar::ToolbarCommand)>();
        q.iter(world).find(|(_, c)| c.0 == "map.fill").map(|(e, _)| e).expect("Fill is on the bar")
    };
    if let Some(complaint) = tooltip_complaint(&mut app, fill, "Fill") {
        panic!("{complaint}");
    }
}

/// **Every toolbar button and every mode option, on the editor's real window shape** (1280x720
/// physical at UiScale 1.2, where the audit measured it). Every complaint is reported, not the first.
#[test]
fn every_toolbar_tooltip_is_on_its_control_at_1280x720() {
    let mut app = editor("tooltip-every");
    harness::resize_surface(&mut app, 1280, 720).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let widgets: Vec<Entity> = {
        let world = app.world_mut();
        let bar = {
            let mut b = world.query_filtered::<Entity, With<bevy_cauldron::toolbar::Toolbar>>();
            b.single(world).expect("one toolbar")
        };
        let mut q = world.query_filtered::<Entity, With<bevy_gooey::Tooltip>>();
        q.iter(world)
            .filter(|e| {
                let mut at = *e;
                while let Some(parent) = world.get::<ChildOf>(at) {
                    at = parent.parent();
                    if at == bar {
                        return true;
                    }
                }
                false
            })
            .collect()
    };
    assert!(widgets.len() >= 8, "the bar carries {} tooltips; expected every button and mode", widgets.len());
    let mut complaints = Vec::new();
    for widget in widgets {
        let what = app
            .world()
            .get::<bevy_gooey::Tooltip>(widget)
            .map_or_else(|| "?".to_owned(), |t| t.text.to_string());
        complaints.extend(tooltip_complaint(&mut app, widget, &what));
    }
    assert!(complaints.is_empty(), "{}", complaints.join("\n"));
}
