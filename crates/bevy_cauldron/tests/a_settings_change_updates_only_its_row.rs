//! **Changing a setting updates that row and nothing else** (#429).
//!
//! The owner, on 0.5.3: "Whenever I adjust a setting, it scrolls me back to the top, losing my place" and
//! "Changing settings in here causes a <200ms flicker as things resize or the UI is repainted". The screen
//! built every row again on each write, so the list was a new list: its scroll offset was zero, its focus
//! gone, and for a frame its layout was not the one before. A change now touches only its own row: the
//! control's value, the *Modified* mark and the Reset button.
//!
//! Each test records the settings list on every frame around a change: the rows' entities, where each is
//! laid out, the list's scroll offset, the focused entity and how many nodes the dialog holds.

use std::path::PathBuf;

use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::ui::{Checked, ComputedNode, ScrollPosition, UiGlobalTransform};
use bevy::ui_widgets::{SliderValue, ValueChange};
use bevy_cauldron::project::Project;
use bevy_cauldron::settings::UserSettingsFile;
use bevy_cauldron::settings_screen::{OpenSettings, Scope, SettingsDialog};
use bevy_gooey::testing::{click, find_by_label};
use bevy_gooey::{ChoiceValue, ListView};

use crate::fixtures;

/// How many frames each test records after a change.
const FRAMES_AFTER: usize = 8;

/// The most nodes a change may add to the dialog or take from it: the *Modified* label and the Reset button with
/// the nodes gooey builds in them (measured: six).
const MARK_NODES: usize = 6;

/// The most the changed row's height may move: the Reset button is taller than the text beside it (measured: 12 px).
const MARK_GROWTH_PX: f32 = 24.0;

struct Rig {
    app: App,
    user: PathBuf,
}

fn rig(tag: &str) -> Rig {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let user = root.join(format!("user_config_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    app.insert_resource(UserSettingsFile(user.clone()));
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().contains_resource::<Project>(), "the rig opens a project");
    app.world_mut().write_message(OpenSettings);
    for _ in 0..4 {
        app.update();
    }
    Rig { app, user }
}

/// What the list and the dialog looked like on one frame.
#[derive(Clone, Debug)]
struct Frame {
    /// The list's rows, in order.
    rows: Vec<Entity>,
    /// Each row's laid-out top and height, in pixels.
    placed: Vec<(f32, f32)>,
    /// Every entity under the dialog.
    nodes: Vec<Entity>,
    /// Where the list is drawn scrolled to, in logical px (layout clamps it to the content).
    scroll: f32,
    /// Where the list says it is scrolled to: what a later change to the content would draw.
    stored: f32,
    focus: Option<Entity>,
    list: Entity,
}

/// The one settings list on screen: the page of the tab shown (the others are hidden and have no size).
fn shown_list(app: &mut App) -> Entity {
    let world = app.world_mut();
    let dialog = world.query_filtered::<Entity, With<SettingsDialog>>().single(world).expect("the settings dialog is open");
    let mut lists = world.query_filtered::<(Entity, &ComputedNode, &InheritedVisibility), With<ListView>>();
    let candidates: Vec<Entity> = lists.iter(world).filter(|(_, n, shown)| n.size().y > 0.0 && shown.get()).map(|(e, _, _)| e).collect();
    let found: Vec<Entity> = candidates
        .into_iter()
        .filter(|list| {
            let mut at = *list;
            while let Some(parent) = world.get::<ChildOf>(at) {
                at = parent.parent();
                if at == dialog {
                    return true;
                }
            }
            false
        })
        .collect();
    assert_eq!(found.len(), 1, "exactly one settings list is on screen, got {found:?}");
    found[0]
}

fn record(app: &mut App, list: Entity) -> Frame {
    let world = app.world_mut();
    let rows_column = world.get::<Children>(list).and_then(|c| c.first().copied());
    let rows: Vec<Entity> = rows_column.and_then(|column| world.get::<Children>(column)).map(|c| c.iter().collect()).unwrap_or_default();
    let placed = rows
        .iter()
        .map(|row| {
            let node = world.get::<ComputedNode>(*row).map_or(0.0, |n| n.size().y);
            let top = world.get::<UiGlobalTransform>(*row).map_or(0.0, |t| t.translation.y - node / 2.0);
            (top, node)
        })
        .collect();
    let dialog = world.query_filtered::<Entity, With<SettingsDialog>>().single(world).expect("the settings dialog is open");
    let mut nodes: Vec<Entity> = Vec::new();
    let mut pending = vec![dialog];
    while let Some(entity) = pending.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter() {
                nodes.push(child);
                pending.push(child);
            }
        }
    }
    // The drawn offset is layout's, clamped to the content; the stored one is what is written back to the list.
    let scroll = world.get::<ComputedNode>(list).map_or(0.0, |n| n.scroll_position.y * n.inverse_scale_factor());
    let stored = world.get::<ScrollPosition>(list).map_or(0.0, |s| s.y);
    let focus = world.resource::<InputFocus>().get();
    Frame { rows, placed, nodes, scroll, stored, focus, list }
}

/// Scrolls the shown list to its end and lets it settle; returns the frame it settled on.
fn scrolled_to_the_end(app: &mut App) -> Frame {
    let list = shown_list(app);
    app.world_mut().get_mut::<ScrollPosition>(list).expect("a list scrolls").y = 100_000.0;
    for _ in 0..10 {
        app.update();
    }
    let frame = record(app, list);
    assert!(frame.scroll > 0.0, "the list scrolled to its end ({} px) so a rebuild would show", frame.scroll);
    frame
}

/// Runs `FRAMES_AFTER` frames and checks each against `before`: the list is the same list, and no more than
/// `max_gone` nodes (the marks of a row that is no longer modified) left the dialog. The focus ends on `focus_after`;
/// the drawn scroll offset stays, unless the list got shorter at its end (`follows_end`), when it may only go back.
fn assert_the_list_is_untouched(app: &mut App, before: &Frame, changed_row: usize, what: &str, max_gone: usize, focus_after: Option<Entity>, follows_end: bool) -> Frame {
    let mut last = before.clone();
    for n in 1..=FRAMES_AFTER {
        app.update();
        let now = record(app, before.list);
        assert_eq!(now.rows, before.rows, "{what}: frame {n}: the list's rows were despawned or replaced");
        let gone: Vec<&Entity> = before.nodes.iter().filter(|e| !now.nodes.contains(e)).collect();
        assert!(gone.len() <= max_gone, "{what}: frame {n}: {} nodes under the dialog were despawned, at most {max_gone} (the marks) may go: {gone:?}", gone.len());
        let added = now.nodes.len() - before.nodes.len().min(now.nodes.len());
        assert!(added <= MARK_NODES, "{what}: frame {n}: the change added {added} nodes; a mark and a Reset is at most {MARK_NODES}");
        if follows_end {
            assert!(now.scroll <= before.scroll, "{what}: frame {n}: a list that got shorter scrolled further ({} to {})", before.scroll, now.scroll);
        } else {
            assert_eq!(now.scroll, before.scroll, "{what}: frame {n}: the list's drawn scroll offset moved");
        }
        assert_eq!(now.focus, focus_after, "{what}: frame {n}: the focus is not where it should be (before: {:?})", before.focus);
        // Every row but the changed one keeps its height, and the rows down to the changed one keep their
        // distance from each other: the card is centred, so a row that grows by a Reset button moves it all by
        // half of that, which is a translation and not a different layout.
        for (i, (was, is)) in before.placed.iter().zip(&now.placed).enumerate() {
            if i != changed_row {
                assert_eq!(was.1, is.1, "{what}: frame {n}: row {i} was relaid: its height went from {} to {}", was.1, is.1);
            }
            if i > 0 && i <= changed_row {
                let (gap_was, gap_is) = (was.0 - before.placed[i - 1].0, is.0 - now.placed[i - 1].0);
                assert_eq!(gap_was, gap_is, "{what}: frame {n}: row {i} was relaid: its distance from the row above went from {gap_was} to {gap_is}");
            }
        }
        let grew = now.placed[changed_row].1 - before.placed[changed_row].1;
        assert!(grew.abs() <= MARK_GROWTH_PX, "{what}: frame {n}: the changed row's height moved by {grew} px, more than a mark's {MARK_GROWTH_PX}");
        last = now;
    }
    assert!(last.stored <= last.scroll + 1.0, "{what}: the stored scroll offset ({}) is past the drawn one ({}): stale", last.stored, last.scroll);
    last
}

fn focus(app: &mut App, entity: Entity) {
    app.world_mut().resource_mut::<InputFocus>().set(entity, bevy::input_focus::FocusCause::Navigated);
    app.update();
}

#[test]
fn stepping_the_last_row_keeps_the_scroll_the_focus_and_every_row() {
    let mut r = rig("settings_change_last_row");
    let before = scrolled_to_the_end(&mut r.app);
    let last = before.rows.len() - 1;
    let agent = find_by_label(&mut r.app, "Terminal agent").expect("the last row is on screen");
    focus(&mut r.app, agent);
    let before = record(&mut r.app, before.list);
    assert_eq!(before.focus, Some(agent), "the row's control has the keyboard focus");

    r.app.world_mut().commands().trigger(ValueChange { source: agent, value: 1usize, is_final: true });
    let after = assert_the_list_is_untouched(&mut r.app, &before, last, "stepping Terminal agent", 0, before.focus, false);

    assert!(std::fs::read_to_string(&r.user).unwrap().contains("terminal_agent"), "the change was written to the user's file");
    assert_eq!(r.app.world().get::<ChoiceValue>(agent).map(|c| c.0), Some(1), "the control shows the new value");
    assert!(find_by_label(&mut r.app, "Reset Terminal agent").is_ok(), "the row now says Modified and offers a Reset");
    assert!(after.nodes.len() > before.nodes.len(), "and the mark and the Reset were added to the row");
}

#[test]
fn a_reset_puts_the_control_back_without_building_the_list_again() {
    let mut r = rig("settings_change_reset");
    std::fs::write(&r.user, "{version: 1, terminal_agent: OhMyPi, show_grid: false}").unwrap();
    for _ in 0..30 {
        r.app.update();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let before = scrolled_to_the_end(&mut r.app);
    let last = before.rows.len() - 1;
    let agent = find_by_label(&mut r.app, "Terminal agent").expect("the last row is on screen");
    assert_eq!(r.app.world().get::<ChoiceValue>(agent).map(|c| c.0), Some(1), "the file's Oh My Pi is shown");
    let reset = find_by_label(&mut r.app, "Reset Terminal agent").expect("a set key has a Reset");
    focus(&mut r.app, reset);
    let before = record(&mut r.app, before.list);

    r.app.world_mut().commands().trigger(bevy_gooey::Activate { entity: reset });
    let after = assert_the_list_is_untouched(&mut r.app, &before, last, "resetting Terminal agent", MARK_NODES, Some(agent), true);

    assert_eq!(r.app.world().get::<ChoiceValue>(agent).map(|c| c.0), Some(0), "the control went back to what the layer below says: Off, on a machine with no agent");
    assert!(find_by_label(&mut r.app, "Reset Terminal agent").is_err(), "an unset key has no Reset");
    assert!(after.nodes.len() < before.nodes.len(), "the mark and the Reset were taken from the row");
    assert!(!std::fs::read_to_string(&r.user).unwrap().contains("terminal_agent"), "the key is out of the file");
    // **The focus moved to the row's control**, which is on screen and is the one thing the person was working on.
    assert_eq!(after.focus, Some(agent), "the keyboard focus is on the row's control, not on the Reset that went");
    assert!(r.app.world().get_entity(after.focus.unwrap()).is_ok(), "and it is an entity that exists");
    // **The list got shorter at its end, so its scroll went back with it** and is stored as drawn.
    assert!(after.scroll < before.scroll, "the end of the list came up by the mark's height ({} to {})", before.scroll, after.scroll);
    assert_eq!(after.stored, after.scroll, "the stored offset is the drawn one");
    // And when the row grows again the list does not jump to where it had been.
    r.app.world_mut().commands().trigger(ValueChange { source: agent, value: 1usize, is_final: true });
    let regrown = assert_the_list_is_untouched(&mut r.app, &after, last, "stepping Terminal agent again", 0, Some(agent), false);
    assert_eq!(regrown.scroll, after.scroll, "no jump when the list grows back");
}

#[test]
fn a_switch_and_a_slider_follow_the_file_in_place() {
    let mut r = rig("settings_change_in_place");
    let top = shown_list(&mut r.app);
    let before = record(&mut r.app, top);
    let grid = find_by_label(&mut r.app, "Grid lines").expect("the Grid lines switch is listed");
    let scale = find_by_label(&mut r.app, "Text size").expect("the Text size slider is listed");
    assert!(r.app.world().get::<Checked>(grid).is_some(), "the grid is on by default");

    // An edit made by hand, as the screen reads the file a few times a second.
    std::fs::write(&r.user, "{version: 1, show_grid: false, text_scale: 1.5}").unwrap();
    for _ in 0..30 {
        r.app.update();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let after = record(&mut r.app, top);

    assert_eq!(after.rows, before.rows, "the rows are the same rows");
    assert!(before.nodes.iter().all(|n| after.nodes.contains(n)), "no node under the dialog was despawned");
    assert!(r.app.world().get::<Checked>(grid).is_none(), "the switch turned off in place");
    assert_eq!(r.app.world().get::<SliderValue>(scale).map(|v| v.0), Some(1.5), "the slider moved in place");
    assert!(find_by_label(&mut r.app, "150%").is_ok(), "and says so");
    assert!(find_by_label(&mut r.app, "Reset Grid lines").is_ok() && find_by_label(&mut r.app, "Reset Text size").is_ok(), "both rows are marked modified");
}

#[test]
fn a_change_with_a_search_active_keeps_the_search_and_its_rows() {
    let mut r = rig("settings_change_searching");
    let search = find_by_label(&mut r.app, "Search settings").expect("the screen has a search field");
    click(&mut r.app, search);
    let field = r
        .app
        .world_mut()
        .query::<(Entity, &bevy_gooey::TextInputValue)>()
        .iter(r.app.world())
        .filter(|(_, v)| v.0.is_empty())
        .map(|(e, _)| e)
        .next()
        .expect("the search row holds no text");
    r.app.world_mut().commands().trigger(ValueChange { source: field, value: "grid".to_owned(), is_final: false });
    for _ in 0..4 {
        r.app.update();
    }
    let list = shown_list(&mut r.app);
    let before = record(&mut r.app, list);
    assert_eq!(before.rows.len(), 2, "two grid settings are listed");
    let grid = find_by_label(&mut r.app, "Grid lines").expect("the switch is listed");
    focus(&mut r.app, grid);
    let before = record(&mut r.app, list);

    r.app.world_mut().commands().trigger(ValueChange { source: grid, value: false, is_final: true });
    assert_the_list_is_untouched(&mut r.app, &before, 0, "flipping Grid lines under a search", 0, before.focus, false);

    assert!(r.app.world().get::<Checked>(grid).is_none(), "the switch is off");
    assert!(find_by_label(&mut r.app, "Theme").is_err(), "the search still hides what it hid");
}

#[test]
fn switching_whose_file_is_edited_changes_values_in_place() {
    let mut r = rig("settings_change_scope");
    std::fs::write(&r.user, "{version: 1, show_grid: false}").unwrap();
    for _ in 0..30 {
        r.app.update();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let list = shown_list(&mut r.app);
    let before = record(&mut r.app, list);
    let grid = find_by_label(&mut r.app, "Grid lines").expect("the switch is listed");
    assert!(r.app.world().get::<Checked>(grid).is_none(), "the user's file turns the grid off");
    {
        let world = r.app.world_mut();
        let mut dialogs = world.query::<&mut SettingsDialog>();
        let mut dialog = dialogs.single_mut(world).expect("the dialog is open");
        dialog.scope = Scope::Project;
    }
    for _ in 0..4 {
        r.app.update();
    }
    let after = record(&mut r.app, list);
    assert_eq!(after.rows, before.rows, "the rows are the same rows");
    assert!(after.nodes.contains(&grid), "the control is the same control");
    assert!(r.app.world().get::<Checked>(grid).is_none(), "the project file sets nothing, so the control shows what it inherits: off");
    assert!(find_by_label(&mut r.app, "Reset Grid lines").is_err(), "and the project's file has nothing to reset");
}

#[test]
fn no_setting_is_built_as_an_unreadable_row_in_either_scope_so_a_change_never_builds_the_list_again() {
    // A row is "unreadable value" only when the value a layer gives is not the kind its control shows; that is the one
    // corner that falls back to building the list again, so it is pinned as unreachable: every setting of the catalogue
    // has its control, with a file that sets some keys and with one that sets none, in the user's scope and the project's.
    let mut r = rig("settings_change_no_unreadable");
    std::fs::write(&r.user, "{version: 1, theme: Parchment, show_grid: false, text_scale: 1.5, terminal_cursor: Bar}").unwrap();
    for scope in [Scope::User, Scope::Project] {
        {
            let world = r.app.world_mut();
            let mut dialogs = world.query::<&mut SettingsDialog>();
            dialogs.single_mut(world).expect("the dialog is open").scope = scope;
        }
        for _ in 0..30 {
            r.app.update();
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let list = shown_list(&mut r.app);
        let rows = record(&mut r.app, list).rows.len();
        assert_eq!(rows, bevy_cauldron::setting_catalogue::CATALOGUE.len(), "{scope:?}: one row per setting");
        let world = r.app.world_mut();
        let mut texts = world.query::<&Text>();
        let unreadable: Vec<String> = texts.iter(world).filter(|t| t.0.contains("unreadable value")).map(|t| t.0.clone()).collect();
        assert!(unreadable.is_empty(), "{scope:?}: rows built as unreadable: {unreadable:?}");
    }
}
