//! **The review of round 7 (19846a28), pinned** (#260): the compass's arms and dots keep their size, the button boxes keep the skin's padding, the thumbnail slot never shrinks,
//! and the severity rail and the notes' chip never shrink.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Mode;
use bevy_gooey::Role;

use crate::fixtures;

fn logical(computed: &ComputedNode) -> Vec2 {
    computed.size() * computed.inverse_scale_factor()
}

/// **Every arm is 2 px thick and every dot 14 px across, as laid out.** `HostGeometry` leaves every `Sizing` length to the host, and `follow_the_camera` wrote only the arm's width and
/// the dot's position: the arm had no height (it drew nothing) and the dot was as big as its letter.
#[test]
fn the_compass_arms_and_dots_are_laid_out_at_their_size() {
    let root = fixtures::Fixture::new(&format!("round7_compass_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let world = app.world_mut();
    let (mut arms, mut dots) = (0, 0);
    for (entity, computed, node, kids) in world.query_filtered::<(Entity, &ComputedNode, &Node, Option<&Children>), (With<bevy_gooey::HostGeometry>, Without<bevy_cauldron::terminal_pane::TerminalCaret>)>().iter(world) {
        let size = logical(computed);
        if kids.is_some() {
            dots += 1;
            assert!((size.x - 14.0).abs() < 1.0 && (size.y - 14.0).abs() < 1.0, "{entity:?}: a compass dot is 14 px across, laid out {size:?} (node {:?} x {:?})", node.width, node.height);
        } else {
            arms += 1;
            assert!(size.x > 0.0, "{entity:?}: a compass arm has a length: {size:?}");
            assert!((size.y - 2.0).abs() < 1.0, "{entity:?}: a compass arm is 2 px thick, laid out {size:?} (node {:?} x {:?})", node.width, node.height);
        }
    }
    assert_eq!((arms, dots), (3, 3), "three arms and three dots");
}

/// **A button box is padded by the skin, whichever it is**: the project card measured 9 x 3.6 px at the raw node (2f83f0c3), and every other button box carries the same role.
fn assert_skin_padding(app: &App, entity: Entity, what: &str) {
    assert_eq!(app.world().get::<Role>(entity), Some(&Role::Button), "{what} is a gooey button");
    let node = app.world().get::<Node>(entity).expect("a Node");
    let (Val::Px(x), Val::Px(y)) = (node.padding.left, node.padding.top) else { panic!("{what}: padding is in px: {:?}", node.padding) };
    assert!((x - 9.0).abs() < 0.01 && (y - 3.6).abs() < 0.01, "{what} is padded by the skin, 9 x 3.6 px: {x} x {y}");
}

#[test]
fn a_map_card_and_a_kit_row_are_padded_by_the_skin() {
    use bevy_cauldron::loading::LoadingPlugin;
    use bevy_cauldron::project_page::{KitRow, MapCard};
    let root = fixtures::Fixture::new(&format!("round7_page_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LoadingPlugin);
    harness::stand_on_the_project_page(&mut app, &root);
    for _ in 0..4 {
        app.update();
    }
    let card = app.world_mut().query_filtered::<Entity, With<MapCard>>().iter(app.world()).next().expect("the project page draws a map card");
    assert_skin_padding(&app, card, "a map card");
    let row = app.world_mut().query_filtered::<Entity, With<KitRow>>().iter(app.world()).next().expect("and a kit row");
    assert_skin_padding(&app, row, "a kit row");
}

#[test]
fn a_news_entry_is_padded_by_the_skin() {
    use bevy_cauldron::launcher::{ChangelogText, ClipboardOff, LauncherPlugin, NewsEntry};
    use bevy_cauldron::recent::{self, RecentProjects};
    use bevy_cauldron::screen::Screen;
    let root = fixtures::Fixture::new(&format!("round7_news_{}", std::process::id())).descriptor("bench", "props").build("m");
    let dir = std::env::temp_dir().join(format!("grimoire_round7_news_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    let file = dir.join("recent.yaml");
    recent::record(&file, &root).expect("the project is recorded");
    let text = "# Changelog\n\n## 2026-11-02\n\n- **outside** Something a person sees *For you: A thing, said plainly.* *No clip: a test*\n";
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(RecentProjects(file)).insert_resource(ChangelogText(text.to_owned())).insert_resource(ClipboardOff).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..4 {
        app.update();
    }
    // The What's new page is the launcher's second page: open it through its sidebar row's page state.
    let entry = app.world_mut().query_filtered::<Entity, With<NewsEntry>>().iter(app.world()).next();
    let entry = entry.expect("the What's new page lists the entry");
    assert_skin_padding(&app, entry, "a news entry");
}

/// **The palette's thumbnail slot never shrinks** (round 6 asked for it through a `RowLayout` that a bare `sized` box ignores; it is a row now).
#[test]
fn the_palette_thumbnail_slot_never_shrinks() {
    let root = fixtures::Fixture::new(&format!("round7_slot_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..10 {
        app.update();
    }
    let world = app.world_mut();
    let mut slots = 0;
    for (sizing, node, role) in world.query::<(&bevy_gooey::Sizing, &Node, &Role)>().iter(world) {
        if sizing.width == Some(bevy_gooey::Extent::Px(bevy_cauldron::chrome::THUMB_SLOT)) && sizing.height == sizing.width {
            slots += 1;
            assert_eq!(*role, Role::Row, "the slot is a gooey row, the role that writes `shrink`");
            assert_eq!(node.flex_shrink, 0.0, "a thumbnail slot never shrinks");
        }
    }
    assert!(slots >= 1, "the Map palette draws a thumbnail slot: {slots}");
}

/// **A severity rail and a small box with a shrink of 0 never shrink.** (The rail and the notes' "auto" chip were a raw `Node` with the default shrink of 1.)
#[test]
fn a_severity_rail_and_a_chip_box_do_not_shrink() {
    use bevy_cauldron::chrome;
    let root = fixtures::Fixture::new(&format!("round7_rail_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    let holder = app.world_mut().spawn(Node::default()).id();
    let mut rail = Entity::PLACEHOLDER;
    let mut chip = Entity::PLACEHOLDER;
    let mut commands = app.world_mut().commands();
    commands.entity(holder).with_children(|p| {
        rail = chrome::severity_rail(p, Color::WHITE, ()).id();
        chip = p.spawn(chrome::small_box(chrome::CHIP_SPACING, bevy_gooey::Sides::all(chrome::EDGE_W), Color::WHITE, Some(0.0))).id();
    });
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().get::<Node>(rail).expect("a rail").flex_shrink, 0.0, "a severity rail never shrinks");
    assert_eq!(app.world().get::<Node>(chip).expect("a chip").flex_shrink, 0.0, "a chip box never shrinks");
}
