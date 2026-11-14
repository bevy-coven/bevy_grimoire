//! **The home's pages, its spacers and the project page's body are gooey's** (#260, UI-99, round 6: bevy_gooey G-85).
//!
//! A hidden page was `Node { display: Display::None }` that a system flipped; it is `Shown(false)` now, which the skin writes as `Display::None` *and*
//! `Visibility::Hidden` (so nothing in a hidden page is a tab stop). A spacer was an empty `Node { flex_grow: 1 }`; it is `spacer()`. This file asks the home
//! and the project page what they are on screen: which page shows after a tab is chosen, what a hidden page's node and visibility are, and the gaps.

use bevy::prelude::*;
use bevy_cauldron::harness;
use bevy_cauldron::launcher::{HomePage, HomeTab, Launcher, LauncherPlugin};
use bevy_cauldron::screen::Screen;
use bevy_gooey::{Role, Shown};

use crate::fixtures;

fn on_the_home(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("pages_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    app
}

fn page(app: &mut App, tab: HomeTab) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &HomePage)>();
    q.iter(world).find(|(_, p)| p.0 == tab).map(|(e, _)| e).unwrap_or_else(|| panic!("the home has its {tab:?} page"))
}

/// What a page is on screen: its `Shown`, its `Display` and its `Visibility`.
fn state(app: &App, page: Entity) -> (bool, Display, Visibility) {
    (
        app.world().get::<Shown>(page).expect("a page carries `Shown`").0,
        app.world().get::<Node>(page).expect("a page has a Node").display,
        *app.world().get::<Visibility>(page).expect("a page has a Visibility"),
    )
}

/// **Only the chosen page shows, and a hidden one is hidden from layout and from the eye**, through gooey's `Shown`.
#[test]
fn the_home_shows_the_chosen_page_and_hides_the_rest_through_shown() {
    let mut app = on_the_home("tabs");
    let (projects, learn, news) = (page(&mut app, HomeTab::Projects), page(&mut app, HomeTab::Learn), page(&mut app, HomeTab::WhatsNew));
    assert!(state(&app, projects).0, "the Projects page shows on opening");
    assert_eq!(state(&app, projects).1, Display::Flex);
    for (hidden, name) in [(learn, "Learn"), (news, "What's new")] {
        let (shown, display, visibility) = state(&app, hidden);
        assert!(!shown, "the {name} page is not shown");
        assert_eq!(display, Display::None, "the {name} page takes no room");
        assert_eq!(visibility, Visibility::Hidden, "and nothing in the {name} page is visible, so none of it is a tab stop");
    }
    app.world_mut().resource_mut::<Launcher>().tab = HomeTab::Learn;
    for _ in 0..3 {
        app.update();
    }
    assert!(state(&app, learn).0 && state(&app, learn).1 == Display::Flex, "choosing Learn shows the Learn page");
    assert!(!state(&app, projects).0 && state(&app, projects).1 == Display::None, "and takes the Projects page away");
    assert_eq!(state(&app, projects).2, Visibility::Hidden, "so its widgets are not on screen");
}

/// **The pages are gooey regions with the gaps they had**: the Projects and What's-new pages grow and keep a group / row gap, the Learn page a group gap.
#[test]
fn the_homes_pages_are_gooey_regions_with_their_gaps() {
    let mut app = on_the_home("gaps");
    for (tab, role, gap) in [(HomeTab::Projects, Role::FillRegion, 16.0), (HomeTab::Learn, Role::Column, 16.0), (HomeTab::WhatsNew, Role::FillRegion, 5.0)] {
        let page = page(&mut app, tab);
        assert_eq!(app.world().get::<Role>(page), Some(&role), "the {tab:?} page is a gooey {role:?}, not a raw `Node`");
        let node = app.world().get::<Node>(page).expect("a page has a Node");
        assert_eq!(node.row_gap, Val::Px(gap), "the {tab:?} page's rows stand {gap} px apart");
    }
}

/// **The sidebar's and the Projects header's spacers are gooey's `spacer()`**: they take the room their row leaves.
#[test]
fn the_home_spacers_are_gooey_spacers_that_take_the_room_left() {
    let mut app = on_the_home("spacers");
    let spacers: Vec<f32> = {
        let world = app.world_mut();
        let mut q = world.query::<(&Role, &Node)>();
        q.iter(world).filter(|(role, _)| **role == Role::Spacer).map(|(_, node)| node.flex_grow).collect()
    };
    assert!(spacers.len() >= 2, "the sidebar and the Projects header each have a spacer; found {}", spacers.len());
    assert!(spacers.iter().all(|grow| *grow == 1.0), "a spacer takes the room its row leaves: {spacers:?}");
}

/// **The project page's body is a gooey fill region with the group gap between its sections.**
#[test]
fn the_project_pages_body_is_a_gooey_fill_region() {
    use bevy_cauldron::loading::LoadingPlugin;
    use bevy_cauldron::project_page::PageBody;
    let root = fixtures::Fixture::new(&format!("pages_body_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LoadingPlugin);
    harness::stand_on_the_project_page(&mut app, &root);
    let body = app.world_mut().query_filtered::<Entity, With<PageBody>>().single(app.world()).expect("the page has its body");
    assert_eq!(app.world().get::<Role>(body), Some(&Role::FillRegion), "the page's body is a gooey fill region");
    let node = app.world().get::<Node>(body).expect("the body has a Node");
    assert_eq!(node.row_gap, Val::Px(16.0), "its sections stand the group gap apart");
    assert_eq!(node.flex_grow, 1.0, "and it takes the height the page leaves");
    assert_eq!(node.min_height, Val::Px(0.0), "and may narrow to nothing");
}
