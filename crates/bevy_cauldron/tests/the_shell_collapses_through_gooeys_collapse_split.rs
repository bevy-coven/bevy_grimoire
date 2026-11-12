//! **The shell asks gooey's split to collapse, and synthesises no key to do it** (UI-R06, #336).
//!
//! Cmd+B (View ▸ Show left panel) used to collapse the shell's gooey split by sending Enter to its divider, because gooey
//! offered no call that collapses a split from code. gooey's `CollapseSplit` (G-77, bevy-coven/bevy_gooey#88) is that call,
//! and it is the same path as the divider's own Enter, South button and double press. This file holds what moved:
//!
//! - **the shell's source asks with `CollapseSplit`** and holds no keyboard event of its own to send, so the shell is not
//!   reaching into the divider's input handling and a change there cannot silently stop Cmd+B;
//! - **the call does what the key did**: a `CollapseSplit` sent to the shell's split by anything slides its first pane shut
//!   and keeps the fraction a restore gives back.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::ComputedNode;
use bevy::window::PrimaryWindow;
use bevy_cauldron::chrome::ShellSplit;
use bevy_gooey::{CollapseSplit, MotionSettings, Role, SplitMin, SplitSaved};

use crate::fixtures;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;
/// Frames that settle a glide on the clock above (the spring takes a quarter of a second).
const SETTLE_FRAMES: usize = 90;
/// How close to the floor a collapsed pane stands, logical px: layout is physical and rounds.
const SLACK_PX: f32 = 2.0;

/// **What the shell's source may not contain**: a keyboard event or a focused input of its own to send.
const SYNTHESISED: [&str; 3] = ["KeyboardInput", "FocusedInput", "press_enter_on"];

#[test]
fn the_shell_asks_for_the_collapse_and_sends_no_key() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shell_view.rs");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    // The module's own header says what it no longer does, in words: only the code is read here.
    let code: String = text.lines().filter(|line| !line.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    assert!(code.contains("CollapseSplit"), "shell_view.rs collapses the split with gooey's `CollapseSplit`");
    for what in SYNTHESISED {
        assert!(!code.contains(what), "shell_view.rs sends no key to the divider: `{what}` is a second way into gooey's input handling");
    }
}

#[test]
fn a_collapse_split_slides_the_shells_first_pane_shut_and_open() {
    let root = fixtures::Fixture::new(&format!("shell_collapse_split_{}", std::process::id())).descriptor("fence", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    app.insert_resource(MotionSettings::default());
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    let split = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<ShellSplit>>().single(world).expect("one shell split")
    };
    let pane = app
        .world()
        .get::<Children>(split)
        .expect("children")
        .iter()
        .find(|&kid| app.world().get::<Role>(kid) == Some(&Role::SplitPane))
        .expect("a first pane");
    let width = |app: &App| {
        let node = app.world().get::<ComputedNode>(pane).expect("laid out");
        node.size().x * node.inverse_scale_factor()
    };
    let wide = width(&app);
    // **A collapse goes to the split's own minimum**: the shell puts that floor to nothing for its own collapse (see
    // `collapse_the_left_pane`), and a call that does not goes to the floor, which is what gooey's collapse is.
    let floor = app.world().get::<SplitMin>(split).expect("a minimum").first;
    assert!(wide > floor + SLACK_PX, "the first pane is open to begin with, wider than its floor: {wide} against {floor}");

    app.world_mut().commands().trigger(CollapseSplit { entity: split, collapsed: true });
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert!((width(&app) - floor).abs() <= SLACK_PX, "the pane slid to its floor: {} against {floor}", width(&app));
    assert!(app.world().get::<SplitSaved>(split).is_some(), "and the split remembers what a restore gives back");

    app.world_mut().commands().trigger(CollapseSplit { entity: split, collapsed: false });
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert!((width(&app) - wide).abs() <= SLACK_PX, "the pane is the width it was: {} against {wide}", width(&app));
    assert!(app.world().get::<SplitSaved>(split).is_none(), "and the split saves nothing while it is open");
}
