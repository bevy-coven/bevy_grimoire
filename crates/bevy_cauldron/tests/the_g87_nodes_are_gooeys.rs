//! **The frame, the chips, the cards and the placed layers are gooey's, and look as they did** (#260, UI-99, round 8: bevy_gooey G-87).
//!
//! The bars, strips and docks of the frame, a chip's box, a text field, the cards of the home, the name box and the toast strip were raw `Node`s. They are rows, columns, panels,
//! surfaces and placed nodes now (`InkFollow` for the colours the editor paints, `GapPx` for gaps in px, `Shown` for what hides). This file asks a running editor what they are and what
//! they measure: every number below is the raw node's own.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::chrome::{self, Frame, RowRest};
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Mode;
use bevy_gooey::{Role, Shown};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("g87_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..10 {
        app.update();
    }
    app
}

fn px(v: Val) -> f32 {
    match v {
        Val::Px(p) => p,
        other => panic!("expected px, got {other:?}"),
    }
}

/// **The frame's bars, strip, docks, viewport and body are gooey rows and columns with the raw nodes' numbers.**
#[test]
fn the_frame_is_gooey_rows_and_columns_with_the_numbers_it_had() {
    let app = editor("frame");
    let frame = app.world().resource::<Frame>();
    let node = |e: Entity| app.world().get::<Node>(e).unwrap_or_else(|| panic!("{e:?} has a Node"));
    let role = |e: Entity| app.world().get::<Role>(e).copied();
    for (bar, border_bottom, border_top, what) in [(frame.chrome_bar, 1.0, 0.0, "the chrome bar"), (frame.status, 0.0, 1.0, "the status bar")] {
        assert_eq!(role(bar), Some(Role::Row), "{what} is a gooey row");
        let n = node(bar);
        assert_eq!((n.width, n.height), (Val::Percent(100.0), Val::Px(26.0)), "{what} is the window wide and 26 px tall");
        assert_eq!((n.column_gap, n.flex_shrink, n.align_items), (Val::Px(16.0), 0.0, AlignItems::Center), "{what}: 16 px between its parts, never shrinking, centred");
        assert_eq!((px(n.padding.left), px(n.padding.right), n.padding.top), (12.0, 12.0, Val::ZERO), "{what} is 12 px in from each end");
        assert_eq!((px(n.border.bottom), px(n.border.top)), (border_bottom, border_top), "{what} has its 1 px edge on one side");
        assert!(app.world().get::<bevy_gooey::InkFollow>(bar).is_some(), "{what}'s colours are the editor's");
    }
    let strip = node(frame.door_strip);
    assert_eq!(role(frame.door_strip), Some(Role::Row));
    assert_eq!((strip.column_gap, px(strip.padding.left), strip.align_items, strip.flex_shrink), (Val::Px(2.0), 12.0, AlignItems::Stretch, 0.0), "the door strip: tabs 2 px apart, 12 px in, stretched");
    for (dock, what) in [(frame.left, "the left dock"), (frame.right, "the right dock")] {
        let n = node(dock);
        assert_eq!(role(dock), Some(Role::Sized), "{what} is a gooey sized box (a column has a least width of 0, a dock's width is its widest panel)");
        assert_eq!(n.min_width, Val::Auto, "{what} keeps its automatic least width");
        assert_eq!((n.flex_direction, n.align_items, n.flex_shrink), (FlexDirection::Column, AlignItems::FlexStart, 0.0), "{what}: a column of its panels, to the start, never shrinking");
    }
    let view = node(frame.viewport);
    assert_eq!((view.flex_grow, view.min_width, view.min_height, view.flex_direction), (1.0, Val::ZERO, Val::ZERO, FlexDirection::Row), "the viewport is what is left, and a row");
    let body = node(frame.body);
    assert_eq!((body.width, body.flex_grow, body.min_height, body.flex_direction, body.align_items), (Val::Percent(100.0), 1.0, Val::ZERO, FlexDirection::Row, AlignItems::Stretch), "the body is a row of what the bars leave");
}

/// **A chip is a gooey row the editor still paints**: rest, hover and its own edge colour (a held tag chip's accent), and its words are not a button's.
#[test]
fn a_chip_is_a_row_the_editor_paints_by_state() {
    let mut app = editor("chip");
    let holder = app.world_mut().spawn(Node::default()).id();
    let (rest, edge) = (chrome::ink().row_bg, chrome::ink().accent);
    let mut chip = Entity::PLACEHOLDER;
    app.world_mut().commands().entity(holder).with_children(|p| {
        chip = chrome::chip(p, (), "held", chrome::text::BODY, chrome::ink().text, rest, edge).id();
    });
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().get::<Role>(chip), Some(&Role::Sized), "a chip is a gooey sized box laid out as a row");
    assert!(app.world().get::<bevy_gooey::InkFollow>(chip).is_some());
    assert_eq!(app.world().get::<BackgroundColor>(chip).map(|b| b.0), Some(rest), "at rest it is its fill");
    assert_eq!(app.world().get::<BorderColor>(chip).map(|b| b.top), Some(edge), "and its edge is the colour it was given, not the skin's outline");
    let n = app.world().get::<Node>(chip).expect("a Node");
    assert_eq!((px(n.padding.left), px(n.padding.top), px(n.border.left), n.flex_shrink), (6.0, 3.0, 1.0, 0.0), "6 x 3 px in, a 1 px edge, never shrinking");
    // Hover and press are `style_list_rows`' (it reads `Hovered`, which the picking backend owns, so a test cannot set it): a chip whose rest fill is the selected colour stays that colour.
    let mut selected = Entity::PLACEHOLDER;
    app.world_mut().commands().entity(holder).with_children(|p| {
        selected = chrome::chip(p, (), "on", chrome::text::BODY, chrome::ink().text, chrome::ink().row_selected, edge).id();
    });
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(app.world().get::<BackgroundColor>(selected).map(|b| b.0), Some(chrome::ink().row_selected), "a held chip is drawn in the selected fill");
    let kids: Vec<Entity> = app.world().get::<Children>(chip).expect("a chip holds its word").iter().collect();
    assert!(kids.iter().all(|k| app.world().get::<Role>(*k) != Some(&Role::ButtonText)), "a chip's word is not `ButtonText`: protected tests count those");
    assert!(app.world().get::<RowRest>(chip).is_some());
}

/// **A text field is a row with its inset, its floor and its width.**
#[test]
fn a_text_field_keeps_its_inset_its_floor_and_its_width() {
    let mut app = editor("field");
    let holder = app.world_mut().spawn(Node::default()).id();
    let mut field = Entity::PLACEHOLDER;
    app.world_mut().commands().entity(holder).with_children(|p| {
        field = chrome::text_field(p, Val::Px(56.0), (), chrome::text::BODY, ("1".to_owned(), Color::WHITE), ()).id();
    });
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().get::<Role>(field), Some(&Role::Sized));
    let n = app.world().get::<Node>(field).expect("a Node");
    assert_eq!((n.width, n.min_height, n.flex_shrink), (Val::Px(56.0), Val::Px(18.0), 0.0), "56 px wide, 18 px at least, never shrinking");
    assert_eq!((px(n.padding.left), px(n.padding.top)), (4.0, 2.0), "4 x 2 px in");
    assert_eq!(app.world().get::<BackgroundColor>(field).map(|b| b.0), Some(chrome::ink().row_bg), "in the editor's field colour");
}

/// **The name box is hidden through `Shown`, click-through, with a 360 px card 10 px between its lines.**
#[test]
fn the_name_box_is_a_cover_hidden_through_shown_with_its_card() {
    let app = editor("name");
    let world = app.world();
    let boxes: Vec<Entity> = world.iter_entities().filter(|e| e.contains::<chrome::NameBox>()).map(|e| e.id()).collect();
    assert_eq!(boxes.len(), 1, "one name box");
    let layer = boxes[0];
    assert_eq!(world.get::<Shown>(layer).map(|s| s.0), Some(false), "nobody is being asked a name");
    let n = world.get::<Node>(layer).expect("a Node");
    assert_eq!((n.display, n.position_type), (Display::None, PositionType::Absolute), "hidden, and out of the flow");
    assert_eq!((n.left, n.top, n.right, n.bottom), (Val::ZERO, Val::ZERO, Val::ZERO, Val::ZERO), "pinned to every edge of the window (`cover`), which is the window's rectangle the raw node's 100% x 100% was");
    assert_eq!((n.align_items, n.justify_content), (AlignItems::Center, JustifyContent::Center), "its card is centred");
    assert_eq!(world.get::<Pickable>(layer), Some(&Pickable::IGNORE), "and a click falls through it");
    let card = world.get::<Children>(layer).and_then(|k| k.iter().next()).expect("the card");
    let c = world.get::<Node>(card).expect("a Node");
    assert_eq!((c.row_gap, c.min_width, px(c.padding.left), c.flex_direction), (Val::Px(10.0), Val::Px(360.0), 18.0, FlexDirection::Column), "10 px between lines, 360 px at least, 18 px in");
}

/// **The toast strip is hidden through `Shown`, placed 16 px down and the viewport wide.**
#[test]
fn the_toast_strip_is_placed_and_hidden_through_shown() {
    let app = editor("toast");
    let world = app.world();
    let layer = world.iter_entities().find(|e| e.contains::<chrome::ToastLayer>()).map(|e| e.id()).expect("the toast strip");
    assert_eq!(world.get::<Shown>(layer).map(|s| s.0), Some(false), "no toast stands");
    let n = world.get::<Node>(layer).expect("a Node");
    assert_eq!((n.display, n.position_type, n.top, n.left, n.width, n.justify_content), (Display::None, PositionType::Absolute, Val::Px(16.0), Val::Px(0.0), Val::Percent(100.0), JustifyContent::Center));
}

/// **The toolbar's tool group stands off the window's edge by the skin's gap**, now a `Space::Px` margin.
#[test]
fn the_tool_group_stands_off_the_edge_by_a_px_margin() {
    let mut app = editor("toolbar");
    let world = app.world_mut();
    let modes = world.query_filtered::<Entity, With<bevy_cauldron::toolbar::ToolModes>>().iter(world).next().expect("the toolbar's tool group");
    let n = world.get::<Node>(modes).expect("a Node");
    assert_eq!(n.margin.left, Val::Px(5.0), "5 px (the editor skin's gap) in from the window's edge");
}

/// **The home's missing-project card and its page grounds keep their boxes.**
#[test]
fn the_launchers_cards_and_grounds_keep_their_boxes() {
    use bevy_cauldron::launcher::{HomeContent, LauncherPlugin, MissingCard, SidebarRoot};
    use bevy_cauldron::recent;
    use bevy_cauldron::screen::Screen;
    let root = fixtures::Fixture::new(&format!("g87_launcher_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let gone = fixtures::Fixture::new(&format!("g87_gone_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let dir = std::env::temp_dir().join(format!("grimoire_g87_launcher_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    let file = dir.join("recent.yaml");
    recent::record(&file, &gone).expect("recorded");
    recent::record(&file, &root).expect("recorded");
    std::fs::remove_dir_all(&gone).expect("the second project goes");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(recent::RecentProjects(file)).add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    let world = app.world_mut();
    let side = world.query_filtered::<Entity, With<SidebarRoot>>().iter(world).next().expect("the sidebar");
    let home = world.query_filtered::<Entity, With<HomeContent>>().iter(world).next().expect("the page");
    for (e, level, what) in [(side, 1, "the sidebar"), (home, 0, "the page")] {
        assert_eq!(world.get::<Role>(e), Some(&Role::Surface(level)), "{what} is a gooey surface");
        let n = world.get::<Node>(e).expect("a Node");
        assert_eq!((n.flex_direction, n.row_gap, px(n.padding.left)), (FlexDirection::Column, Val::Px(16.0), 12.0), "{what}: a column, 16 px between, 12 px in");
    }
    let card = world.query_filtered::<Entity, With<MissingCard>>().iter(world).next().expect("the vanished project has a card");
    assert_eq!(world.get::<Role>(card), Some(&Role::Card), "a missing project's card is a card");
    let n = world.get::<Node>(card).expect("a Node");
    assert_eq!((n.width, n.row_gap, px(n.padding.top), n.flex_direction), (Val::Px(184.0), Val::Px(3.0), 12.0, FlexDirection::Column), "184 px wide, 3 px between lines, 12 px in");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **No chip, button or row is drawn at another height**: measured on the raw nodes at 4a12b153 (the merged head, before this round), a Map door's ten boxes that carry `RowRest` were, sorted,
/// these heights (logical px). The widths of the ones that hold a project's name move with the name (the fixture's folder is named by the process), so only the heights, which do not, are pinned.
/// The last, a palette row with its thumbnail slot, is 36.67 px tall because it is a row: as a column it was 102.5.
#[test]
fn the_chips_buttons_and_rows_keep_their_drawn_heights() {
    let mut app = editor("sizes");
    let world = app.world_mut();
    let mut drawn: Vec<f32> = world.query_filtered::<&ComputedNode, With<RowRest>>().iter(world).map(|c| c.size().y * c.inverse_scale_factor()).collect();
    drawn.sort_by(f32::total_cmp);
    let was = [0.0, 0.0, 0.0, 20.0, 20.0, 20.0, 20.0, 24.17, 24.17, 36.67];
    assert_eq!(drawn.len(), was.len(), "the Map door carries ten chips, buttons and rows: {drawn:?}");
    for (got, want) in drawn.iter().zip(was) {
        assert!((got - want).abs() < 0.1, "a box is {got} tall, it was {want}: {drawn:?}");
    }
}

/// **The bar's Back button is a chip-sized row, and its spacers are gooey's.**
#[test]
fn the_back_button_is_a_row_and_the_bars_spacers_are_gooeys() {
    let mut app = editor("back");
    let world = app.world_mut();
    let back = world.query_filtered::<Entity, With<chrome::BackButton>>().iter(world).next().expect("the chrome bar's back button");
    assert_eq!(world.get::<Role>(back), Some(&Role::Row), "the back button is a gooey row");
    let n = world.get::<Node>(back).expect("a Node");
    assert_eq!((n.column_gap, px(n.padding.left), px(n.padding.top), n.align_items, n.flex_shrink), (Val::Px(5.0), 6.0, 3.0, AlignItems::Center, 0.0), "5 px between its words, a chip's inset, centred, never shrinking");
    assert_eq!(world.get::<BackgroundColor>(back).map(|b| b.0), Some(chrome::ink().slot_bg), "in the editor's own fill");
    let frame = world.resource::<Frame>();
    let bar = frame.chrome_bar;
    let spacers = world.get::<Children>(bar).into_iter().flat_map(|k| k.iter()).filter(|k| world.get::<Role>(*k) == Some(&Role::Spacer)).count();
    assert_eq!(spacers, 1, "the bar's one spacer pushes the subject to the far end");
}
