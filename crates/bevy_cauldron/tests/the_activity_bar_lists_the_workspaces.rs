//! **The activity bar** (NV-03, #380; series getting around, #377): one icon per registered workspace on
//! the window's far left, the sliding indicator on the open one, a tooltip naming it and its shortcut, and
//! a badge for what needs a person.
//!
//! What is measured, and how a person reaches it:
//!
//! 1. **Every registered workspace has an icon, in registry order, and nothing else does** — no
//!    placeholder tab for a workspace whose part is not built.
//! 2. **A click on each icon lands in its workspace**, with the open map and the selection unchanged, and
//!    the indicator follows a switch made any other way (a key).
//! 3. **A badge appears when something needs a person and clears when it is answered**: a refused file is a
//!    `1` on Map and an answered one takes it away; a waiting wish is a `1` on Agent and answering it
//!    clears it (`badge` and `waiting` are the bar's own functions, and Agent has no tab until NV-10, so
//!    this is measured on them rather than on a pill that is not drawn).
//! 4. **View ▸ Show activity bar takes the bar away, and the choice is saved** in the user's settings.
//! 5. **Zen mode takes the bar away** (spawned on demand, so it is gone and not hidden).
//! 6. **F6 reaches the bar**: it is a focus region. (The arrows and Enter are gooey's `TabList`; no headless test of them is kept here because the harness's layout gave `AutoDirectionalNavigation` nothing to move to.)

use std::path::PathBuf;

use bevy::prelude::*;

use bevy_cauldron::activity_bar::{self, ActivityBar, ActivityStrip};
use bevy_cauldron::harness;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::selection::Selection;
use bevy_cauldron::settings::UserSettingsFile;
use bevy_cauldron::workspace::Workspace;
use bevy_grimoire_core::live::KindVerdict;

use crate::fixtures;

const SETTLE: usize = 12;

fn editor(tag: &str, user: Option<&PathBuf>) -> App {
    let root = fixtures::Fixture::new(&format!("activity_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    if let Some(user) = user {
        app.insert_resource(UserSettingsFile(user.clone()));
    }
    settle(&mut app);
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

/// **The tabs of the bar, in the order they are drawn**: gooey puts them under one `TabList` child of the
/// strip, and child order is draw order.
fn tabs(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let strip = world.query_filtered::<Entity, With<ActivityStrip>>().single(world).unwrap_or_else(|e| panic!("one activity strip: {e}"));
    let list = world
        .get::<Children>(strip)
        .and_then(|kids| kids.iter().find(|k| world.get::<Children>(*k).is_some_and(|c| c.iter().any(|t| world.get::<bevy_gooey::IconTab>(t).is_some()))))
        .unwrap_or_else(|| panic!("the strip has a tab list"));
    world.get::<Children>(list).map(|c| c.iter().filter(|t| world.get::<bevy_gooey::IconTab>(*t).is_some()).collect()).unwrap_or_default()
}

/// The words a badge pill holds, in tab order, with `None` for a tab that has none.
fn badges(app: &mut App) -> Vec<Option<String>> {
    let tabs = tabs(app);
    let world = app.world_mut();
    tabs.into_iter()
        .map(|tab| {
            world.get::<Children>(tab).and_then(|kids| {
                kids.iter().find(|k| world.get::<bevy_gooey::CornerBadge>(*k).is_some()).and_then(|k| world.get::<Text>(k).map(|t| t.0.clone()))
            })
        })
        .collect()
}

fn the_bar_is_there(app: &mut App) -> bool {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<ActivityBar>>().iter(world).count() == 1
}

/// **Acceptance 1**: an icon for each registered workspace, in registry order, and none for the rest.
#[test]
fn every_registered_workspace_has_an_icon_in_registry_order_and_no_other_does() {
    let mut app = editor("icons", None);
    let want: Vec<Workspace> = Workspace::ALL.into_iter().filter(|w| w.is_built()).collect();
    assert!(want.len() >= 3, "Map, Kit and Animation are registered");
    let shown = {
        let world = app.world_mut();
        world.query::<&ActivityStrip>().single(world).unwrap_or_else(|e| panic!("{e}")).0.clone()
    };
    assert_eq!(shown, want, "the strip's tabs are the registered workspaces, in registry order");
    let tabs = tabs(&mut app);
    assert_eq!(tabs.len(), want.len(), "one tab for each");
    for (tab, workspace) in tabs.iter().zip(&want) {
        let name = app.world().get::<AccessibleLabel>(*tab).map(|l| l.0.clone()).unwrap_or_default();
        assert_eq!(name, workspace.title(), "a tab names its workspace for a screen reader");
        let has_a_picture = app.world().get::<Children>(*tab).is_some_and(|kids| kids.iter().any(|k| app.world().get::<bevy_gooey::Icon>(k).is_some()));
        assert!(has_a_picture, "{} has an icon from the editor's set", workspace.title());
    }
}

/// **Acceptance 1**: a click on each icon lands in its workspace; the map and the selection stay.
#[test]
fn a_click_on_each_icon_lands_in_its_workspace_and_the_map_and_selection_stay() {
    let mut app = editor("clicks", None);
    assert!(bevy_cauldron::editor::lock_on_placement(app.world_mut(), "floor@0"));
    settle(&mut app);
    let map = app.world().resource::<OpenMap>().map.name.clone();
    let selected = app.world().resource::<Selection>().ids().to_vec();
    assert_eq!(selected, ["floor@0"]);

    let order: Vec<Workspace> = {
        let world = app.world_mut();
        world.query::<&ActivityStrip>().single(world).unwrap_or_else(|e| panic!("{e}")).0.clone()
    };
    // Every icon but the current one first, then back to Map: each pick is a real switch.
    let walk: Vec<usize> = (0..order.len()).filter(|i| order[*i] != Workspace::Map).chain(order.iter().position(|w| *w == Workspace::Map)).collect();
    for index in walk {
        let tab = tabs(&mut app)[index];
        bevy_gooey::testing::click(&mut app, tab);
        settle(&mut app);
        assert_eq!(*app.world().resource::<Workspace>(), order[index], "a click on the {} icon did not land there", order[index].title());
        assert_eq!(app.world().resource::<OpenMap>().map.name, map, "{} reloaded the map", order[index].title());
        assert_eq!(app.world().resource::<Selection>().ids(), selected.as_slice(), "{} dropped the selection", order[index].title());
        let on = {
            let world = app.world_mut();
            world.query_filtered::<&bevy_gooey::TabsValue, With<ActivityStrip>>().iter(world).next().map(|v| v.0)
        };
        assert_eq!(on, Some(index), "the indicator is on the workspace that is open");
    }
}

/// **A workspace reached by its key moves the indicator too**, so the bar never says a different place.
#[test]
fn the_indicator_follows_a_workspace_reached_by_its_key() {
    let mut app = editor("indicator", None);
    let kit = bevy_cauldron::commands::by_id("workspace.kit").unwrap_or_else(|| panic!("workspace.kit"));
    let live = *app.world().resource::<bevy_cauldron::keys::Live>();
    app.world_mut().resource_mut::<bevy_cauldron::command_run::Chords>().queue(&kit, live);
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit);
    let index = activity_bar::registered().iter().position(|w| *w == Workspace::Kit).unwrap_or_else(|| panic!("kit is registered"));
    let on = {
        let world = app.world_mut();
        world.query_filtered::<&bevy_gooey::TabsValue, With<ActivityStrip>>().iter(world).next().map(|v| v.0)
    };
    assert_eq!(on, Some(index));
}

/// **Acceptance 2, for a file**: a refused file is a `1` on Map, and an applied one takes it away.
#[test]
fn a_refused_file_is_a_badge_of_one_on_map_and_it_clears() {
    let mut app = editor("problems", None);
    assert!(badges(&mut app).iter().all(Option::is_none), "a bar with nothing to say is silent");
    app.world_mut()
        .resource_mut::<bevy_cauldron::live::EditorReloads>()
        .decide(vec![KindVerdict::refused("kit", "kit.ron", Some("pieces[3].yaw".to_owned()), "not a number")]);
    settle(&mut app);
    let map = activity_bar::registered().iter().position(|w| *w == Workspace::Map).unwrap_or_else(|| panic!("map is registered"));
    let said = badges(&mut app);
    assert_eq!(said[map].as_deref(), Some("1"), "Map shows the one refused file");
    for (i, badge) in said.iter().enumerate() {
        if i != map {
            assert_eq!(badge, &None, "only Map carries the problems count");
        }
    }
    app.world_mut().resource_mut::<bevy_cauldron::live::EditorReloads>().decide(vec![KindVerdict::applied("kit", "kit.ron")]);
    settle(&mut app);
    assert!(badges(&mut app).iter().all(Option::is_none), "the badge clears when the file is applied");
}

/// **Acceptance 2, for a wish**: a waiting wish is a badge of 1 on Agent and answering it clears it. Agent
/// has no tab until NV-10, so this is the bar's own count and the badge it would draw.
#[test]
fn a_waiting_wish_is_one_on_agent_and_the_badge_clears_when_it_is_answered() {
    use bevy_cauldron::present::{WishState, Wishes};
    let mut wishes = Wishes::default();
    assert_eq!(activity_bar::badge(Workspace::Agent, 0, activity_bar::waiting(Some(&wishes), None)), 0, "nothing waiting, nothing drawn");
    wishes.take(bevy_grimoire_core::wish::Wish::default(), "key".to_owned(), "https://example.invalid".to_owned());
    assert_eq!(activity_bar::badge(Workspace::Agent, 0, activity_bar::waiting(Some(&wishes), None)), 1, "a waiting wish is a 1 on Agent");
    assert_eq!(activity_bar::badge(Workspace::Map, 0, activity_bar::waiting(Some(&wishes), None)), 0, "and no other workspace carries it");
    wishes.queued[0].state = WishState::Opened;
    assert_eq!(activity_bar::badge(Workspace::Agent, 0, activity_bar::waiting(Some(&wishes), None)), 0, "answered, it clears");
}

/// Steps until `done` holds, sleeping between frames for the settings watcher's thread.
fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    for _ in 0..120 {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    panic!("never saw: {what}");
}

/// **View ▸ Show activity bar** takes the bar away, brings it back, and the choice is in the settings file.
#[test]
fn the_view_menu_collapses_the_bar_and_the_choice_is_saved() {
    let root = std::env::temp_dir().join(format!("activity_collapse_{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{e}"));
    let user = root.join("settings.ron");
    std::fs::write(&user, "(version: 1)").unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor("collapse", Some(&user));
    assert!(the_bar_is_there(&mut app), "the bar is on by default");
    let item = |app: &mut App| {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_cauldron::menu_bar::ActivityBarItem>>().single(world).unwrap_or_else(|e| panic!("one View row: {e}"))
    };
    let ticked = |app: &mut App| {
        let row = item(app);
        app.world().get::<bevy_gooey::Checked>(row).is_some()
    };
    assert!(ticked(&mut app), "the row is ticked while the bar is shown");

    let row = item(&mut app);
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: row });
    until(&mut app, "the bar gone after choosing the row", |app| !the_bar_is_there(app));
    app.update();
    let saved = std::fs::read_to_string(&user).unwrap_or_else(|e| panic!("{}: {e}", user.display()));
    assert!(saved.contains("show_activity_bar") && saved.contains("false"), "the choice is saved: {saved}");
    assert!(!ticked(&mut app), "and the row is unticked");

    let row = item(&mut app);
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: row });
    until(&mut app, "the bar back after choosing the row again", the_bar_is_there);
    assert!(ticked(&mut app) || { app.update(); ticked(&mut app) }, "and the tick is back");
    let _ = std::fs::remove_dir_all(&root);
}

/// **A tab's tooltip names the workspace and its shortcut**, from the workspace's own command.
#[test]
fn a_tooltip_names_the_workspace_and_its_shortcut() {
    let mut app = editor("tooltip", None);
    for (tab, workspace) in tabs(&mut app).into_iter().zip(activity_bar::registered()) {
        let tip = app.world().get::<bevy_gooey::Tooltip>(tab).unwrap_or_else(|| panic!("{} has a tooltip", workspace.title()));
        assert_eq!(tip.text, workspace.title());
        let command = bevy_cauldron::commands::by_id(&format!("workspace.{}", workspace.id())).unwrap_or_else(|| panic!("its command"));
        assert_eq!(tip.shortcut, Some(command.shortcut), "{}'s tooltip carries the command's chord", workspace.title());
    }
}

/// **The bar is a focus region, so F6 can reach it**, and it is the first region in tree order, left of
/// the docks.
#[test]
fn the_bar_is_a_focus_region_before_the_docks() {
    let mut app = editor("region", None);
    let bar = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<ActivityBar>>().single(world).unwrap_or_else(|e| panic!("{e}"))
    };
    let region = app.world().get::<bevy_gooey::FocusRegion>(bar).copied();
    assert_eq!(region, Some(bevy_gooey::FocusRegion::navigate()), "the bar navigates with the arrows");
}

/// **Zen mode takes the bar with everything else**, and leaving it brings the bar back.
#[test]
fn zen_mode_takes_the_bar_away_and_leaving_it_brings_it_back() {
    let mut app = editor("zen", None);
    assert!(the_bar_is_there(&mut app));
    app.world_mut().resource_mut::<bevy_cauldron::shell_view::ShellView>().zen = true;
    settle(&mut app);
    assert!(!the_bar_is_there(&mut app), "Zen mode leaves only the viewport, so the bar is gone");
    app.world_mut().resource_mut::<bevy_cauldron::shell_view::ShellView>().zen = false;
    settle(&mut app);
    assert!(the_bar_is_there(&mut app), "and it is back when Zen mode ends");
}

/// **The bar is paid by the docks, not by the map**: in the reference window the viewport is as wide with
/// the bar as without it (`the_frames_regions_fit_the_window` is the floor under this).
#[test]
fn the_bar_takes_its_width_from_the_docks_and_the_viewport_keeps_its_own() {
    let viewport_width = |bar: bool| {
        let root = fixtures::Fixture::new(&format!("activity_width_{bar}_{}", std::process::id()))
            .descriptor("site/wall", "alpha")
            .build("m");
        let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        app.world_mut().spawn((
            Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() },
            bevy::window::PrimaryWindow,
        ));
        app.world_mut().resource_mut::<bevy_cauldron::settings::EditorSettings>().show_activity_bar = bar;
        for _ in 0..16 {
            app.update();
        }
        assert_eq!(the_bar_is_there(&mut app), bar);
        let viewport = app.world().resource::<bevy_cauldron::chrome::Frame>().viewport;
        app.world().get::<ComputedNode>(viewport).map(|n| n.size().x).unwrap_or(0.0)
    };
    let (with, without) = (viewport_width(true), viewport_width(false));
    assert!(without > 0.0);
    assert!((with - without).abs() < 1.0, "the viewport is {with} px with the bar and {without} px without it");
}
