//! **The toolbar's tool light slides from the tool that was armed to the one that is** (UI-R06, bevy-coven/bevy_grimoire#336).
//!
//! The tool modes are one gooey `toggle_group`, and gooey's G-61 springs one accent node (its `SelectionIndicator`) from the old
//! option's rect to the new one's instead of swapping two fills. The headless editor runs with motion off so every other test reads
//! a result in the frame it is made; this puts the motion back and steps a clock the test sets, so a frame is a known length.
//! What it holds: nothing shows at rest, a change starts a travel that is between the two options on the way and lands on the
//! new one within half a pixel, and a change in the middle turns round from where the light is drawn, with no jump.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use bevy_cauldron::editor::{EditorState, Tool};
use bevy_cauldron::toolbar::ToolModes;
use bevy_gooey::{IndicatorPuck, MotionSettings, Role, SelectionIndicator, ToggleGroupValue};

use crate::fixtures;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;
/// How far the light may land from the option's own rect, logical px (gooey's own bound).
const LANDS_WITHIN_PX: f32 = 0.5;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    // The motion back on, as a person has it.
    app.insert_resource(MotionSettings::default());
    for _ in 0..40 {
        app.update();
    }
    app
}

fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn group(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<ToolModes>>().single(world).expect("one tool group")
}

fn options(app: &App, group: Entity) -> Vec<Entity> {
    app.world().get::<Children>(group).expect("children").iter().filter(|&kid| matches!(app.world().get::<Role>(kid), Some(Role::GroupOption))).collect()
}

/// Where `entity` is laid out, logical px.
fn rect(app: &App, entity: Entity) -> Rect {
    let node = app.world().get::<ComputedNode>(entity).expect("laid out");
    let at = app.world().get::<UiGlobalTransform>(entity).expect("placed").translation;
    let scale = node.inverse_scale_factor();
    Rect::from_center_size(at * scale, node.size() * scale)
}

fn indicator_of(app: &mut App, group: Entity) -> Entity {
    app.world_mut()
        .query::<(Entity, &SelectionIndicator, &ChildOf)>()
        .iter(app.world())
        .find(|(_, _, parent)| parent.parent() == group)
        .map(|(entity, ..)| entity)
        .expect("the group's indicator")
}

/// The light's drawn left edge relative to the strip, and whether the indicator shows at all.
fn drawn(app: &App, indicator: Entity) -> (f32, bool) {
    let puck = app
        .world()
        .get::<Children>(indicator)
        .expect("the indicator's puck")
        .iter()
        .find(|&kid| app.world().get::<IndicatorPuck>(kid).is_some())
        .expect("a puck");
    let left = match app.world().get::<Node>(puck).expect("a node").left {
        Val::Px(px) => px,
        _ => 0.0,
    };
    let shown = app.world().get::<Node>(indicator).expect("a node").display == Display::Flex;
    (left, shown)
}

fn relative_left(app: &App, option: Entity, group: Entity) -> f32 {
    rect(app, option).min.x - rect(app, group).min.x
}

fn arm(app: &mut App, tool: Tool) {
    app.world_mut().resource_mut::<EditorState>().tool = tool;
}

fn lit(app: &mut App, group: Entity) -> usize {
    app.world().get::<ToggleGroupValue>(group).expect("a value").0
}

#[test]
fn arming_another_tool_slides_the_light_to_it_and_it_is_hidden_at_rest() {
    let mut app = editor("toolbar_slide");
    let group = group(&mut app);
    let options = options(&app, group);
    let indicator = indicator_of(&mut app, group);
    let from = lit(&mut app, group);
    assert!(!drawn(&app, indicator).1, "nothing is drawn at rest: the options' own fills show");

    arm(&mut app, Tool::Remove);
    frames(&mut app, 1);
    let to = lit(&mut app, group);
    assert_ne!(from, to, "the other tool is a different option");
    let (start, end) = (relative_left(&app, options[from], group), relative_left(&app, options[to], group));
    assert!((end - start).abs() > 1.0, "the two options stand apart: {start} to {end}");

    frames(&mut app, 3);
    let (left, shown) = drawn(&app, indicator);
    assert!(shown, "a travel is under way");
    let (low, high) = (start.min(end), start.max(end));
    assert!(left > low + 0.01 && left < high - 0.01, "on the way, between the options: {left} not inside {low}..{high}");

    frames(&mut app, 90);
    assert!(!drawn(&app, indicator).1, "at rest again, hidden");
    assert!((relative_left(&app, options[to], group) - end).abs() < LANDS_WITHIN_PX, "and the option it landed on has not moved");
}

#[test]
fn the_light_lands_on_the_new_option_and_a_change_in_the_middle_turns_round_without_a_jump() {
    let mut app = editor("toolbar_turn");
    let group = group(&mut app);
    let options = options(&app, group);
    let indicator = indicator_of(&mut app, group);
    let first = lit(&mut app, group);

    arm(&mut app, Tool::Remove);
    frames(&mut app, 5);
    let second = lit(&mut app, group);
    let before = drawn(&app, indicator).0;

    arm(&mut app, Tool::Place);
    frames(&mut app, 1);
    let after = drawn(&app, indicator).0;
    assert_eq!(lit(&mut app, group), first, "back to where it began");
    let span = (relative_left(&app, options[second], group) - relative_left(&app, options[first], group)).abs();
    assert!((after - before).abs() < span / 2.0, "turning round moves it on from where it is drawn ({before} to {after}), not to an end");

    frames(&mut app, 120);
    assert!(!drawn(&app, indicator).1, "at rest");
    assert_eq!(lit(&mut app, group), first, "and the first tool is the one lit");
}
