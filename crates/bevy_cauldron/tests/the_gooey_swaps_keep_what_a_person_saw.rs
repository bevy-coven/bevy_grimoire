//! **The gooey swaps of #260 keep what a person saw** (review of 4c1aec04).
//!
//! Three swaps changed more than their node: a card's picture lost its crop, the dialogs' cards were given a width gooey's
//! dialog skin used to overwrite, and a Stop row's text became a pickable child. Each is pinned here at what is *drawn*
//! (a rect, a measured width, a click), so the next conversion cannot move it silently.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::chrome::GAP_TIGHT;
use bevy_gooey::Role;

use crate::fixtures;

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("keep_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, 8);
    app
}

/// A headless editor opened on `mode`, settled.
fn editor_on(tag: &str, mode: Mode) -> App {
    let root = fixtures::Fixture::new(&format!("keep_on_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, 8);
    app
}

/// The one entity carrying marker `M`.
fn the<M: Component>(app: &mut App) -> Entity {
    let mut found = app.world_mut().query_filtered::<Entity, With<M>>();
    found.iter(app.world()).next().unwrap_or_else(|| panic!("an entity carrying {} is on screen", std::any::type_name::<M>()))
}

/// The logical width a node is laid out at.
fn width_of(app: &App, entity: Entity) -> f32 {
    let computed = app.world().get::<ComputedNode>(entity).unwrap_or_else(|| panic!("{entity:?} is laid out"));
    computed.size().x * computed.inverse_scale_factor()
}

/// The dialog card an entity belongs to: itself, its nearest ancestor, or the first panel below it.
fn card_of(app: &App, entity: Entity) -> Entity {
    let is_panel = |e: Entity| app.world().get::<Role>(e) == Some(&Role::Panel);
    if is_panel(entity) {
        return entity;
    }
    let mut up = app.world().get::<ChildOf>(entity).map(ChildOf::parent);
    while let Some(e) = up {
        if is_panel(e) {
            return e;
        }
        up = app.world().get::<ChildOf>(e).map(ChildOf::parent);
    }
    let mut stack = vec![entity];
    while let Some(e) = stack.pop() {
        if is_panel(e) {
            return e;
        }
        if let Some(children) = app.world().get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
    panic!("no dialog card at {entity:?}")
}

/// **A card's picture keeps its crop.** `project_picture` hands back the middle band of the portrait that fits the card's 16:9 box; the box
/// (gooey's `image_box`) must draw that band, not the whole picture squeezed into it.
#[test]
fn a_cards_picture_is_the_middle_band_that_fits_its_box() {
    use bevy_cauldron::launcher::{LauncherPlugin, ProjectCard};
    use bevy_cauldron::screen::Screen;
    // A portrait that is not the card's shape: twice as wide as it is tall.
    let (w, h) = (128.0_f32, 64.0_f32);
    let root = fixtures::Fixture::new(&format!("keep_card_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let thumbs = root.join("target/grimoire_thumbs");
    std::fs::create_dir_all(&thumbs).expect("a thumbnail folder");
    image::RgbaImage::from_pixel(w as u32, h as u32, image::Rgba([90, 120, 160, 255])).save(thumbs.join("0000000000000001.png")).expect("a baked portrait");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    let file = root.join("recent.yaml");
    bevy_cauldron::recent::record(&file, &root).expect("the project is recorded");
    app.insert_resource(bevy_cauldron::recent::RecentProjects(file));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    settle(&mut app, 4);
    let card = app.world_mut().query_filtered::<Entity, With<ProjectCard>>().single(app.world()).expect("the project has a card");
    let picture = app
        .world()
        .get::<Children>(card)
        .into_iter()
        .flat_map(|children| children.iter())
        .find(|child| app.world().get::<ImageNode>(*child).is_some())
        .expect("the card holds its picture");
    // The card's box is 160 x 90 (16:9): the band is as tall as the portrait and 64 / 90 * 160 wide.
    let wide = h / 90.0 * 160.0;
    let want = Rect::from_center_size(Vec2::new(w / 2.0, h / 2.0), Vec2::new(wide, h));
    let drawn = app.world().get::<ImageNode>(picture).and_then(|node| node.rect).expect("the picture is cropped to the band that fits its box, not drawn whole");
    assert!((drawn.min - want.min).length() < 0.01 && (drawn.max - want.max).length() < 0.01, "the card draws the middle band {want:?}, not {drawn:?}");
    assert_eq!(app.world().get::<Role>(picture), Some(&Role::ImageBox), "the picture's box is gooey's `image_box`");
}

/// The width every dialog card here had before it was built with `sized_panel`: gooey's dialog skin capped a card under its scrim at 36 text sizes, whatever
/// `width` the card asked for (measured at the harness window with the old construction, `424d37b`'s rules on `b750eae`: 396.67 px; the new construction
/// with a px width drew 840).
const DIALOG_CARD_PX: f32 = 396.67;

fn assert_dialog_width(app: &App, anything_in_the_dialog: Entity, what: &str) {
    let card = card_of(app, anything_in_the_dialog);
    let width = width_of(app, card);
    assert!((width - DIALOG_CARD_PX).abs() < 1.0, "{what}'s card is {width} px wide, and every dialog card was {DIALOG_CARD_PX} px before");
}

/// **The Settings, Keymap and project-configuration cards render at the width they had.**
#[test]
fn the_settings_keymap_and_configuration_cards_are_as_wide_as_they_were() {
    use bevy_cauldron::keymap_screen::{KeymapDialog, OpenKeymap};
    use bevy_cauldron::project_config::{OpenProjectConfig, ProjectConfigDialog};
    use bevy_cauldron::settings_screen::{OpenSettings, SettingsDialog};
    let mut app = editor("dialogs");
    app.world_mut().write_message(OpenSettings);
    settle(&mut app, 6);
    let settings = app.world_mut().query_filtered::<Entity, With<SettingsDialog>>().single(app.world()).expect("Settings is open");
    assert_dialog_width(&app, settings, "Settings");
    app.world_mut().write_message(OpenKeymap);
    settle(&mut app, 6);
    let keymap = app.world_mut().query_filtered::<Entity, With<KeymapDialog>>().single(app.world()).expect("the Keymap is open");
    assert_dialog_width(&app, keymap, "the Keymap");
    app.world_mut().write_message(OpenProjectConfig);
    settle(&mut app, 6);
    let config = app.world_mut().query_filtered::<Entity, With<ProjectConfigDialog>>().single(app.world()).expect("the project configuration is open");
    assert_dialog_width(&app, config, "the project configuration");
}

/// **The New project form's card renders at the width it had.**
#[test]
fn the_new_project_card_is_as_wide_as_it_was() {
    use bevy_cauldron::launcher::LauncherPlugin;
    use bevy_cauldron::new_project::FormCard;
    use bevy_cauldron::screen::Screen;
    let root = fixtures::Fixture::new(&format!("keep_form_{}", std::process::id())).descriptor("bench", "props").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    settle(&mut app, 4);
    let button = bevy_gooey::testing::find_by_label(&mut app, "New project").unwrap_or_else(|e| panic!("{e:?}"));
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: button });
    settle(&mut app, 4);
    let card = app.world_mut().query_filtered::<Entity, With<FormCard>>().single(app.world()).expect("the form is open");
    assert_dialog_width(&app, card, "the New project form");
}

/// **A press on a Stop row's words stops the job**, as a press on the row did: the words are a pickable child of gooey's `plain_button` now, and the click
/// the row's observer hears is the child's.
#[test]
fn a_click_on_the_stop_rows_words_stops_the_job() {
    use bevy_cauldron::jobs::{CancelJob, Jobs, JobsItem};
    let mut fixture = fixtures::Fixture::new(&format!("keep_jobs_{}", std::process::id()));
    for i in 0..3 {
        fixture = fixture.descriptor(&format!("piece_{i}"), "alpha");
    }
    let root = fixture.build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, 3);
    let item = app.world_mut().query_filtered::<Entity, With<JobsItem>>().iter(app.world()).next().expect("the band carries a job item");
    click(&mut app, item);
    let row = {
        let mut q = app.world_mut().query::<(Entity, &CancelJob)>();
        q.iter(app.world()).find(|(_, r)| r.0 == "thumbs").map(|(e, _)| e).expect("a row for the bake")
    };
    let words = app
        .world()
        .get::<Children>(row)
        .into_iter()
        .flat_map(|children| children.iter())
        .find(|child| app.world().get::<Text>(*child).is_some())
        .expect("the row holds its words");
    click(&mut app, words);
    settle(&mut app, 3);
    let jobs = app.world().resource::<Jobs>();
    assert!(!jobs.snapshot().iter().any(|(key, _)| *key == "thumbs"), "a press on the Stop row's words stopped the bake, as a press on the row does");
}

/// A primary press on `target`, as a person's press produces it.
fn click(app: &mut App, target: Entity) {
    app.world_mut().commands().trigger(bevy::picking::events::PointerClick {
        entity: target,
        pointer: bevy::picking::events::Pointer::new(
            bevy::picking::pointer::PointerId::Mouse,
            bevy::picking::pointer::Location {
                target: bevy::camera::NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget { handle: bevy::asset::Handle::default(), scale_factor: 1.0 }),
                position: bevy::math::Vec2::new(100.0, 700.0),
            },
        ),
        button: bevy::picking::pointer::PointerButton::Primary,
        hit: bevy::picking::backend::HitData::new(target, 0.0, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    });
    app.update();
}

/// **The Familiar block is as wide as the pane it stands in**, now that it no longer asks for `width: 100%`.
#[test]
fn the_familiar_block_fills_its_pane() {
    use bevy_cauldron::familiar::Familiar;
    let root = fixtures::Fixture::new(&format!("keep_familiar_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Agent).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, 8);
    let block = app.world_mut().query_filtered::<Entity, With<Familiar>>().single(app.world()).expect("the block is on screen");
    let parent = app.world().get::<ChildOf>(block).map(ChildOf::parent).expect("the block has a pane");
    let pane = app.world().get::<ComputedNode>(parent).expect("the pane is laid out");
    let room = pane.content_box().width() * pane.inverse_scale_factor();
    let width = width_of(&app, block);
    assert!((width - room).abs() < 1.0, "the block is {width} px wide in a pane with {room} px of room");
}

/// **The workspace strip keeps its 35 px** now that its `min_height` is gooey's `Sizing` and not a patch: the edge tabs' own inset is smaller, so the strip holds the height the
/// panels under it were laid out from.
#[test]
fn the_workspace_strip_keeps_its_height_through_sizing() {
    let mut app = editor_on("strip", Mode::Map);
    let world = app.world_mut();
    let mut q = world.query::<(&Role, &Node, &bevy_gooey::Sizing)>();
    let strips = q.iter(world).filter(|(role, node, sizing)| **role == Role::Row && node.min_height == Val::Px(35.0) && sizing.min_height == Some(bevy_gooey::Extent::Px(35.0))).count();
    assert_eq!(strips, 1, "one gooey row holds the workspace's tabs at 35 px");
}

/// **The Design workspace's page is gooey regions**: its tree holder, its top and its body are the fill regions and the rigid column they stood as, with the gaps they had.
#[test]
fn the_design_workspaces_page_is_built_from_gooey_regions() {
    use bevy_cauldron::design_view::{DesignBody, DesignTop};
    let mut app = editor_on("design", Mode::Design);
    let top = the::<DesignTop>(&mut app);
    assert_eq!(app.world().get::<Role>(top), Some(&Role::Column), "the page's top is a gooey column");
    let node = app.world().get::<Node>(top).expect("the top has a Node");
    assert_eq!((node.row_gap, node.flex_shrink), (Val::Px(GAP_TIGHT), 0.0), "with the tight gap, never squeezed");
    let body = the::<DesignBody>(&mut app);
    assert_eq!(app.world().get::<Role>(body), Some(&Role::FillRegion), "the page's body is a gooey fill region");
    let node = app.world().get::<Node>(body).expect("the body has a Node");
    assert_eq!((node.flex_grow, node.min_height, node.min_width), (1.0, Val::Px(0.0), Val::Px(0.0)), "that takes the room left and may narrow to nothing");
}
