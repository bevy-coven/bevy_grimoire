//! **Every registry command appears in exactly one menu** (UI-31, #225).
//!
//! `docs/ui/vision.md` §5: "The menu bar is generated from the categories … Every command appears in
//! exactly one menu." The bar is [`menu_bar::MENUS`], one gooey `menu_title` per category and one
//! `menu_item` per command, so the two things that can go wrong are a category with no menu (a command
//! nobody can find) and a command listed twice.

use bevy::prelude::*;
use bevy_cauldron::menu_bar::{EditorMenuBar, MenuCommand, MENUS};
use bevy_cauldron::commands;
use bevy_gooey::testing::find_by_label;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **Every category the registry declares has a menu**, so no command is homeless.
#[test]
fn every_category_has_a_menu() {
    let mut missing: Vec<&str> = commands::all()
        .iter()
        .map(|c| c.category())
        .filter(|category| !MENUS.iter().any(|(id, _)| id == category))
        .collect();
    missing.sort_unstable();
    missing.dedup();
    assert!(
        missing.is_empty(),
        "these command categories have no menu, so their commands can only be found through the \
         palette: {missing:?} (the bar is menu_bar::MENUS)"
    );
}

/// **And every command is in a menu exactly once.** The bar builds each row from the registry, so this
/// is the assertion that the *pair* is right: one row per command, no command twice.
#[test]
fn every_command_is_in_exactly_one_menu() {
    let mut app = editor("menu_bar_every_command");
    let mut rows: Vec<MenuCommand> = {
        let world = app.world_mut();
        let mut q = world.query::<&MenuCommand>();
        q.iter(world).copied().collect()
    };
    let built = rows.len();
    rows.sort_unstable_by_key(|r| r.0 as usize);
    let all = commands::all();
    assert_eq!(
        built,
        all.len(),
        "the bar has {built} rows and the registry {} commands: a command the bar leaves out is one \
         only the palette can reach",
        all.len()
    );
    for window in rows.windows(2) {
        assert_ne!(window[0], window[1], "{:?} is in the bar twice", window[0]);
    }
}

/// **The bar is a gooey menu bar, and it says the menial names.** Found by its words, so a test reads
/// it the way a person does.
#[test]
fn the_bar_is_drawn_with_its_menus() {
    let mut app = editor("menu_bar_drawn");
    let bars = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<EditorMenuBar>>();
        q.iter(world).collect::<Vec<_>>()
    };
    assert_eq!(bars.len(), 1, "the editor has one menu bar");
    for (_, title) in MENUS {
        assert!(
            find_by_label(&mut app, title).is_ok(),
            "the bar shows a {title:?} menu"
        );
    }
}

/// **A row the registry refuses is greyed with the registry's own reason.**
///
/// A fresh editor is idle on the Map tab with nothing in hand, so the commands that need a piece in
/// hand or a proposal are refused there. The assertion is the **pair**: every row whose command the
/// registry refuses carries that exact reason, and no other row is greyed. A greyed row with no words
/// is the dead end `Disabled`'s tooltip exists to prevent.
#[test]
fn a_disabled_row_carries_its_reason() {
    let mut app = editor("menu_bar_disabled");
    for _ in 0..4 {
        app.update();
    }
    let live = *app.world().resource::<bevy_cauldron::keys::Live>();
    let rows: Vec<(MenuCommand, Option<String>)> = {
        let world = app.world_mut();
        let mut q = world.query::<(&MenuCommand, Option<&bevy_gooey::Disabled>)>();
        q.iter(world).map(|(r, d)| (*r, d.map(|d| d.0.clone()))).collect()
    };
    assert!(!rows.is_empty(), "the bar has rows");
    let mut refused = 0;
    for (row, reason) in &rows {
        let command = commands::by_id(commands::identity(row.0).0).expect("the registry has it");
        let want = command.enabled(live).err();
        assert_eq!(
            reason.as_deref(),
            want,
            "{row:?} says {reason:?} and the registry says {want:?}: the row must carry the \
             registry's own reason, greyed exactly when it has one"
        );
        if want.is_some() {
            refused += 1;
            assert!(
                !reason.as_deref().unwrap_or_default().is_empty(),
                "{row:?} is greyed with nothing a person can read"
            );
        }
    }
    assert!(
        refused > 0,
        "with nothing in hand the registry refuses some commands (a piece-in-hand verb), so at \
         least one row must be greyed: none is, so the bar is not reading the registry's judge"
    );
}
