//! **A sidebar tab that is no longer the chosen one stops wearing the selection rail** —
//! `feedback-1793927950267`, oracle `other:rail-follows-selection` (an invariant on one run: the rail count
//! on the sidebar equals one, on the tab `Launcher::tab` names).
//!
//! The note — *"The "What's new" button here is not working?"* — came with a frame that has two railed
//! tabs (`Projects` and `Learn`) beside the `Projects` page. Reproduced on the installed 0.3.0 on mac_air:
//! the click does switch the page, but the tab left behind keeps its rail, so the sidebar reads as two
//! chosen tabs and the one that is chosen looks like it did nothing. `paint_marked_buttons` only visits
//! buttons that still carry `Selected`, so the branch that removes a rail from a button that lost it never
//! ran.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::launcher::{HomeTab, Launcher, NavTab};
use bevy_cauldron::screen::Screen;

use crate::fixtures;

fn home(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    harness::stand_on_the_project_page(&mut app, &root);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..10 {
        app.update();
    }
    app
}

/// The tabs that wear a rail: a child pinned absolutely to the button's leading edge.
fn railed_tabs(app: &mut App) -> Vec<HomeTab> {
    let world = app.world_mut();
    let mut tabs = world.query::<(&NavTab, &Children)>();
    let mut out = Vec::new();
    for (tab, kids) in tabs.iter(world) {
        let rails = kids
            .iter()
            .filter(|k| world.get::<Node>(*k).is_some_and(|n| n.position_type == PositionType::Absolute))
            .count();
        assert!(rails <= 1, "a tab wears at most one rail");
        if rails == 1 {
            out.push(tab.0);
        }
    }
    out
}

#[test]
fn only_the_chosen_tab_wears_the_rail_after_the_choice_moves() {
    let mut app = home("capture_1793927950267_rail");
    assert_eq!(railed_tabs(&mut app), vec![HomeTab::Projects], "the home opens with Projects railed");

    for chosen in [HomeTab::WhatsNew, HomeTab::Learn, HomeTab::Projects] {
        app.world_mut().resource_mut::<Launcher>().tab = chosen;
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(
            railed_tabs(&mut app),
            vec![chosen],
            "after choosing {chosen:?}, exactly that tab wears the rail; a tab left behind keeps its own, so two read as chosen"
        );
    }
}
