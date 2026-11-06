//! **The What's new tab switches the page it names** — `feedback-1793927950267`, relation: *idempotence of
//! the shown page against the chosen tab*.
//!
//! The note, from the Projects home at 2560x1604: *"The "What's new" button here is not working?"* — pointing
//! at the sidebar's third tab, which is drawn with its count (`What's new (7)`) and nothing happens when it
//! is pressed.
//!
//! # The relation, and why this one
//!
//! A picture has no expected output, so the oracle is a relation between two things that both exist in one
//! run: **the tab the person chose, and the page on screen**. Segura, Fraser, Sánchez & Ruiz-Cortés, *A
//! Survey on Metamorphic Testing* (`10.1109/TSE.2016.2532875`), find the effective relations are those
//! "strongly inspired by the semantics of the program under test" — here, choosing a tab shows that tab's
//! page, which is the whole of what a tab means.
//!
//! The survey's other finding is why the *press* is what this test drives rather than the resource:
//! `capture_feedback_1793905912313` already sets `Launcher::tab` directly and passes, so a test that writes
//! the resource **passes on the defect**. Only going through `Activate` — the same bus a click, `Enter` and
//! a pad's south button all land on — can see a broken handler.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::launcher::{HomePage, HomeTab, Launcher, LauncherPlugin, NavTab};
use bevy_cauldron::screen::Screen;

use crate::fixtures;

/// The Projects home, stood up the way the editor stands it up.
fn home(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **The menu is its own screen**, and the capture is on it.
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    harness::stand_on_the_project_page(&mut app, &root);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..10 {
        app.update();
    }
    app
}

/// The `NavTab` entity whose word is `label`, with the words a person reads on it.
fn nav_tab(app: &mut App, label: &str) -> Option<Entity> {
    let world = app.world_mut();
    let mut tabs = world.query::<(Entity, &NavTab)>();
    let found: Vec<Entity> = tabs.iter(world).map(|(entity, _)| entity).collect();
    for entity in found {
        // The words live on the button's own `Text` child, badge included (`What's new (7)`).
        let words = world
            .get::<Children>(entity)
            .and_then(|kids| kids.iter().find_map(|k| world.get::<Text>(k).map(|t| t.0.clone())))
            .unwrap_or_default();
        if words.starts_with(label) {
            return Some(entity);
        }
    }
    None
}

/// Is the page for `tab` the visible one?
fn page_shown(app: &mut App, tab: HomeTab) -> bool {
    let world = app.world_mut();
    let mut pages = world.query::<(&HomePage, &Node)>();
    pages
        .iter(world)
        .find(|(page, _)| page.0 == tab)
        .is_some_and(|(_, node)| node.display != Display::None)
}

/// **Pressing the What's new tab shows the What's new page.**
///
/// The press goes through `Activate`, which is the bus a click, `Enter` and a gamepad's south button all
/// fire — so the handler under test is the one a person actually reaches. Then the page must be the visible
/// one, and the other pages must not be.
#[test]
fn pressing_the_whats_new_tab_shows_the_whats_new_page() {
    let mut app = home("capture_1793927950267");

    // The home opens on Projects, so What's new is not the page on screen to begin with.
    assert!(
        page_shown(&mut app, HomeTab::Projects),
        "the Projects home opens on its first tab; if it does not, this test's premise is wrong"
    );

    let tab = nav_tab(&mut app, "What's new").expect(
        "the sidebar draws a What's new tab — the capture shows it with its count, `What's new (7)`",
    );
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: tab });
    for _ in 0..4 {
        app.update();
    }

    let chosen = app.world().resource::<Launcher>().tab;
    assert_eq!(
        chosen,
        HomeTab::WhatsNew,
        "pressing the What's new tab did not even set it as the chosen tab — the handler is not running"
    );
    assert!(
        page_shown(&mut app, HomeTab::WhatsNew),
        "the What's new tab is chosen, but its page is not on screen: the press sets the tab and nothing \
         shows it. This is what \"The 'What's new' button here is not working?\" reports — a tab that looks \
         live, counts its entries, and changes nothing."
    );
    assert!(
        !page_shown(&mut app, HomeTab::Projects),
        "both pages are visible at once, so the switch is a paint rather than a page change"
    );
}
