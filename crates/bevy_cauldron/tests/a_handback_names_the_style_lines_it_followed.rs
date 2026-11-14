//! **A hand-back names the style lines it followed** (SG-09, #408).
//!
//! `grimoire_present` takes an optional `followed: [{path, line}]`: the lines of the style `grimoire_style` served the agent that it worked to. The
//! person's hand-back card lists each line's text with a link that opens it on the Design page, so they can argue with a specific line instead of a
//! general impression. A line that is not one the agent was served is refused, so a citation is never decoration.
//!
//! # What is proved
//!
//! 1. After `grimoire_style`, a `grimoire_present` citing two of its lines puts both texts on the card, each link opens the page on its line, and
//!    `grimoire_state` returns the two entries with the hand-back and none of their text.
//! 2. A line of a page the agent was never served, a line past the page's end, a blank line, and a line of a page that changed after it was served are
//!    each refused with the entry and the reason, and nothing is put in front of the person.
//! 3. The card lists at most five lines and a count of the rest, and every line stays in the state.

use bevy::prelude::*;
use bevy_cauldron::design_view::DesignView;
use bevy_cauldron::familiar::{FamiliarItem, FollowedLink, HandBackCard, HandBackHere};
use bevy_cauldron::harness;
use bevy_cauldron::style_followed::{FOLLOWED_SHOWN_MAX, FollowedParam, STYLE_SERVED_MAX, StyleServed, check, served_pages};
use bevy_grimoire_core::trust::TrustStore;
use bevy_gooey::{Activate, ValueChange};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::fixtures;

const GAME_PAGE: &str = "design/style.md";
const LEVEL_PAGE: &str = "design/levels/m/style.md";
const GAME_TEXT: &str = "# The game\n\nWarm light.\nQuiet streets.\nNo neon.\nStone, not glass.\nOne sound at a time.\nNo crowds.\nSlow weather.\n";
const LEVEL_TEXT: &str = "# The courtyard\n\nA small walled yard.\n";

/// **An editor with the game's and the level's pages allowed**, the AGENT block open and answering hand-backs.
fn ready(name: &str) -> (App, PathBuf) {
    ready_in(name, true)
}

/// The same, with a window to type into (`windowed`) or without one, which is what a real pointer needs to reach the surface.
fn ready_in(name: &str, windowed: bool) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    for (page, text) in [(GAME_PAGE, GAME_TEXT), (LEVEL_PAGE, LEVEL_TEXT)] {
        let file = root.join(page);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder is made");
        std::fs::write(&file, text).expect("the page is written");
    }
    let store = root.join("trusted.yaml");
    let mut trusted = TrustStore::default();
    for (page, text) in [(GAME_PAGE, GAME_TEXT), (LEVEL_PAGE, LEVEL_TEXT)] {
        trusted.trust_text(&root, page, text).expect("the text resolves");
    }
    trusted.save(&store).expect("the store saves");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.add_plugins(bevy_cauldron::launcher::LauncherPlugin);
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().trust.file = Some(store);
    if windowed {
        app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    }
    updates(&mut app, 8);
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 6);
    app.world_mut().resource_mut::<HandBackHere>().0 = true;
    updates(&mut app, 2);
    (app, root)
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn call(app: &mut App, method: &str, params: Value) -> Result<Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    let answer = app.world_mut().run_system_with(id, Some(params)).map_err(|e| format!("{method}: {e:?}"))?.map_err(|e| format!("{method}: {e:?}"))?;
    updates(app, 4);
    Ok(answer)
}

/// **The agent reads the style of the game**, which is what makes its pages lines it may cite.
fn read_the_style(app: &mut App, map: Option<&str>) {
    let served = call(app, "grimoire/style", json!({"map": map})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(served["withheld"], json!([]), "the pages are allowed: {served}");
}

fn present(app: &mut App, followed: Value) -> Result<Value, String> {
    call(app, "grimoire/present", json!({"title": "the courtyard", "summary": "Laid the yard out.", "followed": followed}))
}

fn card_text(app: &mut App) -> String {
    let world = app.world_mut();
    world.query::<&Text>().iter(world).map(|text| text.0.clone()).collect::<Vec<_>>().join("\n")
}

fn links(app: &mut App) -> Vec<(Entity, String, u32)> {
    let world = app.world_mut();
    world.query::<(Entity, &FollowedLink)>().iter(world).map(|(entity, link)| (entity, link.path.clone(), link.line)).collect()
}

/// **1. Both texts are on the card, each link opens its page on the line, and the state carries the entries and no text.**
#[test]
fn the_card_lists_the_lines_followed_and_each_link_opens_its_page() {
    let (mut app, _root) = ready("followed_card");
    read_the_style(&mut app, None);
    let made = present(&mut app, json!([{"path": GAME_PAGE, "line": 3}, {"path": GAME_PAGE, "line": 4}])).unwrap_or_else(|e| panic!("{e}"));
    assert!(made["presentation"].is_string(), "{made}");

    let world = app.world_mut();
    assert_eq!(world.query_filtered::<Entity, With<HandBackCard>>().iter(world).count(), 1, "the hand-back is a card in the block");
    let shown = card_text(&mut app);
    assert!(shown.contains("Warm light.") && shown.contains("Quiet streets."), "the card lists both lines' texts: {shown}");
    let found = links(&mut app);
    assert_eq!(found.iter().map(|(_, path, line)| (path.as_str(), *line)).collect::<Vec<_>>(), vec![(GAME_PAGE, 3), (GAME_PAGE, 4)]);

    // Each link opens its page on its line.
    let (second, _, _) = found[1].clone();
    app.world_mut().commands().trigger(Activate { entity: second });
    app.world_mut().flush();
    let view = app.world().resource::<DesignView>();
    assert_eq!(view.selected(), Some(GAME_PAGE), "the Design page is on the cited page");
    assert_eq!(view.line_asked(), Some(4), "and on the cited line");

    // **The state carries the entries and none of the text.**
    let state = call(&mut app, "grimoire/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state["presentation"]["followed"], json!([{"path": GAME_PAGE, "line": 3}, {"path": GAME_PAGE, "line": 4}]));
    let said = state["presentation"].to_string();
    assert!(!said.contains("Warm") && !said.contains("Quiet"), "a page's text is not in the state: {said}");
}

/// **2. The citations that are not lines the agent was served are each refused with the entry and the reason.**
#[test]
fn a_line_never_served_past_the_end_blank_or_changed_is_refused() {
    let (mut app, root) = ready("followed_refused");

    // Nothing has been served yet: no page is a line the agent was given.
    let none = present(&mut app, json!([{"path": GAME_PAGE, "line": 3}])).expect_err("nothing was served");
    assert!(none.contains("followed entry 1") && none.contains(GAME_PAGE) && none.contains("grimoire_style"), "{none}");

    // The game's chain was served, and the level's page was not.
    read_the_style(&mut app, None);
    let other = present(&mut app, json!([{"path": LEVEL_PAGE, "line": 3}])).expect_err("the level's page was never served");
    assert!(other.contains("followed entry 1") && other.contains(LEVEL_PAGE) && other.contains("not a page of any chain"), "{other}");

    let past = present(&mut app, json!([{"path": GAME_PAGE, "line": 40}])).expect_err("past the end");
    assert!(past.contains("followed entry 1") && past.contains("no line 40"), "{past}");

    let blank = present(&mut app, json!([{"path": GAME_PAGE, "line": 2}])).expect_err("a blank line");
    assert!(blank.contains("followed entry 1") && blank.contains("blank"), "{blank}");

    let zero = present(&mut app, json!([{"path": GAME_PAGE, "line": 0}])).expect_err("lines count from 1");
    assert!(zero.contains("from 1"), "{zero}");

    // The second entry is the one that is wrong, and the answer says which.
    let second = present(&mut app, json!([{"path": GAME_PAGE, "line": 3}, {"path": GAME_PAGE, "line": 2}])).expect_err("the second is blank");
    assert!(second.contains("followed entry 2"), "{second}");
    let twice = present(&mut app, json!([{"path": GAME_PAGE, "line": 3}, {"path": GAME_PAGE, "line": 3}])).expect_err("listed twice");
    assert!(twice.contains("twice"), "{twice}");

    // **A refusal puts nothing in front of the person.**
    let world = app.world_mut();
    assert_eq!(world.query_filtered::<Entity, With<HandBackCard>>().iter(world).count(), 0);
    assert!(app.world().resource::<bevy_cauldron::present::Presenting>().0.is_none());

    // **And a page that changed after it was served is not one the agent followed.**
    std::fs::write(root.join(GAME_PAGE), GAME_TEXT.replace("Warm light.", "Cold light.")).expect("an outside edit");
    let changed = present(&mut app, json!([{"path": GAME_PAGE, "line": 3}])).expect_err("the page changed");
    assert!(changed.contains("followed entry 1") && changed.contains("changed"), "{changed}");
    // The sentence names the entry and the page and says none of what the page now says.
    assert!(!changed.contains("Cold") && !changed.contains("Warm"), "{changed}");
}

/// **3. The card lists five lines and a count of the rest, and the state keeps every line.**
#[test]
fn the_card_lists_five_lines_and_counts_the_rest() {
    let (mut app, _root) = ready("followed_many");
    read_the_style(&mut app, None);
    let cited: Vec<Value> = (3..=9).map(|line| json!({"path": GAME_PAGE, "line": line})).collect();
    assert_eq!(cited.len(), 7);
    present(&mut app, json!(cited)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(links(&mut app).len(), FOLLOWED_SHOWN_MAX, "the card lists at most five");
    let shown = card_text(&mut app);
    assert!(shown.contains("and 2 more"), "and says how many it left out: {shown}");
    let state = call(&mut app, "grimoire/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state["presentation"]["followed"].as_array().map(Vec::len), Some(7), "every line stays in the state");
}

/// **A hand-back that cites nothing is as it always was.**
#[test]
fn a_handback_with_no_style_involved_cites_nothing() {
    let (mut app, _root) = ready("followed_none");
    present(&mut app, json!([])).unwrap_or_else(|e| panic!("{e}"));
    assert!(links(&mut app).is_empty());
    let state = call(&mut app, "grimoire/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state["presentation"]["followed"], json!([]));
}

fn close_the_block(app: &mut App) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: false, is_final: true });
    updates(app, 6);
}

/// **Scroll the panel `entity` is in until `entity` is inside it**, turning the wheel over the panel as a person does: a pointer move onto it, then wheel notches through
/// picking, the hover map and the observer `bevy_ui_widgets` installs (the way `a_wheel_over_the_inspector_scrolls_it.rs` drives it).
fn scroll_into_view(app: &mut App, entity: Entity) {
    use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
    use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};
    use bevy::ui::{ComputedNode, OverflowAxis, UiGlobalTransform};

    let panel = std::iter::successors(Some(entity), |e| app.world().get::<ChildOf>(*e).map(ChildOf::parent))
        .find(|e| app.world().get::<Node>(*e).is_some_and(|node| node.overflow.y == OverflowAxis::Scroll))
        .expect("the link is inside a scrolling panel");
    let rect = |app: &App, e: Entity| {
        let node = app.world().get::<ComputedNode>(e).expect("laid out");
        let at = app.world().get::<UiGlobalTransform>(e).expect("placed");
        Rect::from_center_size(at.translation, node.size())
    };
    let handle = app.world().resource::<bevy_cauldron::surface::Surface>().image.clone();
    for _ in 0..60 {
        let (panel_rect, link_rect) = (rect(app, panel), rect(app, entity));
        if link_rect.min.y >= panel_rect.min.y && link_rect.max.y <= panel_rect.max.y {
            return;
        }
        // The surface's pointer is in physical pixels, as `bevy_gooey::testing::click` puts it. **A quarter of the way in, not the centre**: the panel's centre lies under the
        // split's divider (measured: a wheel there reached the divider and the panel did not move), and a quarter in is on the panel's own content.
        let at = Vec2::new(panel_rect.min.x + panel_rect.width() / 4.0, panel_rect.center().y);
        let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle: handle.clone(), scale_factor: 1.0 }), position: at };
        for action in [
            PointerAction::Move { delta: Vec2::ONE },
            PointerAction::Scroll { unit: bevy::input::mouse::MouseScrollUnit::Line, x: 0.0, y: -1.0, phase: bevy::input::touch::TouchPhase::Moved },
        ] {
            app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location.clone(), action));
            updates(app, 2);
        }
    }
    panic!("the wheel did not bring the link into its panel");
}

/// **The first followed link is inside the panel as it opens, and a real click on it opens the page on its line.**
#[test]
fn the_first_followed_link_takes_a_real_click_as_the_card_opens() {
    let (mut app, _root) = ready_in("followed_click_first", false);
    harness::resize_surface(&mut app, 1024, 640).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 6);
    read_the_style(&mut app, None);
    present(&mut app, json!([{"path": GAME_PAGE, "line": 3}, {"path": GAME_PAGE, "line": 4}])).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 4);
    let found = links(&mut app);
    bevy_gooey::testing::click(&mut app, found[0].0);
    updates(&mut app, 6);
    let view = app.world().resource::<DesignView>();
    assert_eq!(view.selected(), Some(GAME_PAGE), "the click opened the page");
    assert_eq!(view.last_line_asked(), Some(3), "on the line the link names");
}

/// **A real click on a followed link, in a 1024 x 640 window, opens the page on its line**: each link has a rect at least 40 px wide, inside the surface.
#[test]
fn a_followed_link_has_a_rect_a_pointer_could_hit_and_its_press_opens_the_page_on_its_line() {
    let (mut app, _root) = ready_in("followed_click", false);
    harness::resize_surface(&mut app, 1024, 640).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 6);
    read_the_style(&mut app, None);
    present(&mut app, json!([{"path": GAME_PAGE, "line": 3}, {"path": GAME_PAGE, "line": 4}])).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 4);
    let found = links(&mut app);
    assert_eq!(found.len(), 2);
    for (entity, _, line) in &found {
        let world = app.world();
        let node = world.get::<bevy::ui::ComputedNode>(*entity).expect("the link is laid out");
        let at = world.get::<bevy::ui::UiGlobalTransform>(*entity).expect("the link has a place");
        let scale = node.inverse_scale_factor;
        let (w, h) = (node.size().x * scale, node.size().y * scale);
        let (cx, cy) = (at.translation.x * scale, at.translation.y * scale);
        assert!(w >= 40.0, "the link to line {line} is {w} px wide");
        assert!(cx - w / 2.0 >= 0.0 && cx + w / 2.0 <= 1024.0 && cy - h / 2.0 >= 0.0 && cy + h / 2.0 <= 640.0, "the link to line {line} is at ({cx}, {cy}), {w} x {h}, outside the surface");
    }
    // **A real click on the second link needs the panel scrolled to it first**, as a person's wheel does. Measured at 1024 x 640 (layout pixels): the card sits at the bottom
    // of the AGENT block's scrolling panel, which is 148 px tall (y 217..365), so the second link, at y 378..407, is below the panel's bottom edge, and a pointer there is over
    // the layout split under the panel, not over the link. That is what the panel's clip is for, not a defect: the person scrolls, and the first link, inside the panel, takes
    // the click as it is (`the_first_followed_link_takes_a_real_click_as_the_card_opens`). The earlier reading that the click "lands on an unnamed Node" was this clip.
    scroll_into_view(&mut app, found[1].0);
    bevy_gooey::testing::click(&mut app, found[1].0);
    updates(&mut app, 6);
    let view = app.world().resource::<DesignView>();
    assert_eq!(view.selected(), Some(GAME_PAGE), "the press opened the page");
    assert_eq!(view.last_line_asked(), Some(4), "on the line the link names");
}

/// **With the block closed the question is the modal, and it lists the followed lines too.**
#[test]
fn the_modal_lists_the_followed_lines_when_the_block_is_closed() {
    let (mut app, _root) = ready("followed_modal");
    close_the_block(&mut app);
    read_the_style(&mut app, None);
    present(&mut app, json!([{"path": GAME_PAGE, "line": 3}, {"path": GAME_PAGE, "line": 4}])).unwrap_or_else(|e| panic!("{e}"));
    let presented = app.world().resource::<bevy_cauldron::present::Presenting>().0.clone().expect("a presentation");
    assert!(!presented.inline, "the block is closed, so it is the modal");
    assert!(presented.body.contains("design/style.md:3") && presented.body.contains("Warm light.") && presented.body.contains("Quiet streets."), "{}", presented.body);
}

/// **A client that says hello again is a new agent**: what an earlier one was served is not what it read.
#[test]
fn a_new_client_must_ask_grimoire_style_again() {
    let (mut app, _root) = ready("followed_hello");
    read_the_style(&mut app, None);
    assert!(!app.world().resource::<StyleServed>().is_empty());
    // The handshake's own answer does not matter here (it checks a token); what it did to the record does.
    let _ = call(&mut app, "grimoire/agent_hello", json!({"client": {"name": "omp", "version": "1"}}));
    assert!(app.world().resource::<StyleServed>().is_empty(), "a new client's record starts empty");
    let refused = present(&mut app, json!([{"path": GAME_PAGE, "line": 3}])).expect_err("nothing was served to this client");
    assert!(refused.contains("not a page of any chain"), "{refused}");
}

/// **What is noted as served is the text that was checked, not the text on disk now.**
#[test]
fn what_is_noted_is_the_text_that_was_checked_and_served() {
    use bevy_grimoire_core::design::PlaceKind;
    use bevy_grimoire_core::style::{File, FileState};
    let file = |path: &str, state| File { place: PlaceKind::Game, path: path.to_owned(), bytes: 3, state };
    let files = [file("design/style.md", FileState::Served), file("design/dialogue.md", FileState::Withheld), file("design/sound.md", FileState::Served)];
    // The disk may say anything by now: only what the trust decisions were made about is noted, and only for a file served.
    let checked = [
        ("design/style.md".to_owned(), "the checked text".to_owned()),
        ("design/dialogue.md".to_owned(), "withheld".to_owned()),
    ];
    assert_eq!(served_pages(&files, &checked), vec![("design/style.md".to_owned(), "the checked text".to_owned())]);
}

/// **The record is the open project's, bounded, and can be cleared.**
#[test]
fn the_record_is_per_project_bounded_and_clearable() {
    let dir = |name: &str| {
        let root = std::env::temp_dir().join(format!("{name}_{}", std::process::id()));
        std::fs::create_dir_all(root.join("design")).expect("a project");
        std::fs::write(root.join("design/style.md"), "# A\n\nOne line.\n").expect("a page");
        root
    };
    let (a, b) = (dir("served_a"), dir("served_b"));
    let cite = |root: &std::path::Path, served: &StyleServed| check(served, root, &[FollowedParam { path: "design/style.md".to_owned(), line: 3 }]);
    let mut served = StyleServed::default();
    served.record(&a, "design/style.md", "# A\n\nOne line.\n".to_owned());
    assert!(cite(&a, &served).is_ok());
    // Another project's chain replaces it: the first project closed.
    served.record(&b, "design/style.md", "# A\n\nOne line.\n".to_owned());
    assert!(cite(&a, &served).is_err(), "a closed project's pages are forgotten");
    assert!(cite(&b, &served).is_ok());
    // Bounded: the oldest page goes first.
    for n in 0..=STYLE_SERVED_MAX {
        served.record(&b, &format!("design/page-{n}.md"), "x\n".to_owned());
    }
    assert_eq!(served.len(), STYLE_SERVED_MAX);
    assert!(cite(&b, &served).is_err(), "the oldest page was dropped");
    served.clear();
    assert!(served.is_empty());
    for root in [a, b] {
        std::fs::remove_dir_all(root).expect("cleaned");
    }
}
