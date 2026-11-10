//! **The AGENT block lists each waiting wish, and the person opens or discards it** (#354, AW-02).
//!
//! AW-01 (#353) queues a wish and its issue address. This is the person's half: the wish is a row in
//! the Map door's AGENT block, saying whose it is, the `tool` line, and the three other fields when
//! it is expanded. Two buttons answer it, and both answers reach the agent through
//! `grimoire_state.wishes`:
//!
//! - **Open on GitHub** opens the wish's own issue address with the editor's one opener. The editor
//!   never posts: GitHub is where the person presses Submit, so `opened` means the page was opened
//!   and not that an issue exists.
//! - **Discard** drops the wish, and opens nothing.
//!
//! # What is being proved
//!
//! 1. **Open on GitHub opens exactly that wish's address**, and the agent reads `opened` from
//!    `grimoire_state.wishes`.
//! 2. **Discard opens nothing**, and the agent reads `discarded`.
//! 3. **With the block hidden, the status band counts the waiting wishes** ("1 waiting for you"), and
//!    a click on it opens the block on that row.
//! 4. **A search that fails says why in one line, and Open on GitHub still opens.** The check informs
//!    the person and never blocks them.
//! 5. **A search that matches lists the issues** it found, before the person opens a new one.

use bevy::prelude::*;
use bevy_cauldron::familiar::{Familiar, FamiliarItem};
use bevy_cauldron::harness;
use bevy_cauldron::present::Wishes;
use bevy_cauldron::wish_view::{WishBody, WishChip, WishChipText, WishDiscard, WishMatch, WishOf, WishOpen, WishSearchLine, WishSearchOff, WishTitle, WishUnfold};
use bevy_gooey::ValueChange;

use crate::fixtures;

/// **A project with one map and one piece**, saved.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(name)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m")
}

fn editor_with_bridge(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    let answer = app
        .world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))?;
    updates(app, 4);
    Ok(answer)
}

fn ready(name: &str) -> App {
    let root = project(name);
    let mut app = editor_with_bridge(&root);
    updates(&mut app, 8);
    app
}

/// **Present a result and answer for its id**, which is the hand-back a wish cites.
fn present(app: &mut App) -> String {
    let presented = call(
        app,
        "grimoire/present",
        serde_json::json!({
            "title": "the courtyard",
            "summary": "A 20 x 6 x 20 m courtyard with its east wall placed.",
            "operation_ids": [],
        }),
    )
    .unwrap_or_else(|e| panic!("the present answers: {e}"));
    presented["presentation"]
        .as_str()
        .unwrap_or_else(|| panic!("the present names the presentation: {presented}"))
        .to_owned()
}

/// **Queue a wish the way an agent does**, and answer with its id.
fn queue_a_wish(app: &mut App) -> String {
    let after = present(app);
    let made = call(
        app,
        "grimoire/wish",
        serde_json::json!({
            "after": {"presentation": after},
            "task": "place a wall along the courtyard's east side",
            "hardship": "three calls to grimoire_state to find the wall's piece id",
            "tool": "grimoire_find_piece: searches the library by label and answers the ids.",
            "example_call": "grimoire_find_piece({\"label\": \"wall\"})",
        }),
    )
    .unwrap_or_else(|e| panic!("the wish must be taken: {e}"));
    made["id"].as_str().unwrap_or_else(|| panic!("the wish names its id: {made}")).to_owned()
}

/// **The wishes as an agent reads them**, from `grimoire/state`.
fn wishes(app: &mut App) -> Vec<serde_json::Value> {
    let state = call(app, "grimoire/state", serde_json::json!({})).unwrap_or_else(|e| panic!("{e}"));
    state["wishes"]
        .as_array()
        .unwrap_or_else(|| panic!("wishes is a list: {}", state["wishes"]))
        .clone()
}

/// **The address this wish will be filed at**, as the editor built it when the agent made it.
fn url_of(app: &App, id: &str) -> String {
    app.world()
        .resource::<Wishes>()
        .queued
        .iter()
        .find(|q| q.wish.id == id)
        .unwrap_or_else(|| panic!("the editor holds wish `{id}`"))
        .url
        .clone()
}

/// The person chose View > Show agent.
fn show_the_block(app: &mut App, on: bool) {
    let row = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<FamiliarItem>>()
            .single(world)
            .expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: on, is_final: true });
    updates(app, 6);
}

fn block_is_open(app: &mut App) -> bool {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<Familiar>>().iter(world).next().is_some()
}

/// **One part of one wish's row in the block**: the part by its own marker, the wish by the [`WishOf`]
/// every part of a row carries.
fn entity_for<T: Component>(app: &mut App, id: &str, what: &str) -> Entity {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &T, &WishOf)>();
    query
        .iter(world)
        .find(|(_, _, of)| of.0 == id)
        .map(|(entity, _, _)| entity)
        .unwrap_or_else(|| panic!("the AGENT block shows no {what} for `{id}`"))
}

/// Whether the block still shows the part `T` for `id`.
fn shows<T: Component>(app: &mut App, id: &str) -> bool {
    let world = app.world_mut();
    let mut query = world.query::<(&T, &WishOf)>();
    query.iter(world).any(|(_, of)| of.0 == id)
}

/// The words a part of a wish's row carries.
fn words<T: Component>(app: &mut App, id: &str, what: &str) -> String {
    let entity = entity_for::<T>(app, id, what);
    app.world()
        .get::<Text>(entity)
        .map(|text| text.0.clone())
        .unwrap_or_else(|| panic!("the {what} for `{id}` carries no words"))
}

/// **The person unfolds the row**, by pressing its header — the gesture a list row answers.
fn unfold(app: &mut App, id: &str) {
    let header = entity_for::<WishUnfold>(app, id, "row header");
    press(app, header);
}

fn press(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity });
    updates(app, 6);
}

/// **A primary press on `target`**, through the `PointerClick` a person's press produces: the editor
/// draws into an image and a test app has no window, so the pointer names the surface.
fn click(app: &mut App, target: Entity) {
    app.world_mut().commands().trigger(bevy::picking::events::PointerClick {
        entity: target,
        pointer: bevy::picking::events::Pointer::new(
            bevy::picking::pointer::PointerId::Mouse,
            bevy::picking::pointer::Location {
                target: bevy::camera::NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget {
                    handle: bevy::asset::Handle::default(),
                    scale_factor: 1.0,
                }),
                position: bevy::math::Vec2::new(100.0, 700.0),
            },
        ),
        button: bevy::picking::pointer::PointerButton::Primary,
        hit: bevy::picking::backend::HitData::new(target, 0.0, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    });
    updates(app, 6);
}

/// **The row says whose it is and the tool line**, before anything leaves the editor.
fn row_names_the_agent_and_the_tool(app: &mut App, id: &str) {
    let title = words::<WishTitle>(app, id, "title");
    assert!(
        title.contains("grimoire_find_piece"),
        "the row's one-line title does not name the tool: {title:?}"
    );
    assert!(
        title.contains("omp") && title.contains("18.16"),
        "the row does not name the agent that made the wish: {title:?}"
    );
}

/// **1. Open on GitHub opens that wish's own address, and the agent reads `opened`.**
#[test]
fn a_wish_row_is_opened_on_github_and_the_agent_reads_opened() {
    let mut app = ready(&format!("wish_open_{}", std::process::id()));
    // **The client says who it is**, as the MCP server tells the editor after its handshake.
    call(
        &mut app,
        "grimoire/agent_hello",
        serde_json::json!({"token": null, "client": {"name": "omp", "version": "18.16.0"}, "tools": 23}),
    )
    .unwrap_or_else(|e| panic!("the hello answers: {e}"));
    let id = queue_a_wish(&mut app);
    let url = url_of(&app, &id);
    show_the_block(&mut app, true);
    row_names_the_agent_and_the_tool(&mut app, &id);

    unfold(&mut app, &id);
    let open = entity_for::<WishOpen>(&mut app, &id, "Open on GitHub button");
    press(&mut app, open);

    assert_eq!(
        harness::opened_links(&app),
        vec![url],
        "Open on GitHub did not open the wish's own issue address"
    );
    let read = wishes(&mut app);
    assert_eq!(read.len(), 1, "the queue lost the wish: {read:?}");
    assert_eq!(read[0]["state"], "opened", "the agent does not read the wish as opened: {read:?}");
    assert!(!shows::<WishOpen>(&mut app, &id), "an opened wish still offers the button that opened it");
}

/// **A row spawned before the agent said who it is names it afterwards**, which is the order a real
/// sitting arrives in: the wish is made, the block is opened, and `grimoire/agent_hello` lands later.
#[test]
fn a_row_names_the_agent_that_introduces_itself_after_it_was_spawned() {
    let mut app = ready(&format!("wish_late_name_{}", std::process::id()));
    let id = queue_a_wish(&mut app);
    show_the_block(&mut app, true);
    let before = words::<WishTitle>(&mut app, &id, "title");
    assert!(before.starts_with("an agent:"), "no client has introduced itself yet: {before:?}");

    // **The agent says who it is after the row exists**, which is what the MCP server does once.
    call(
        &mut app,
        "grimoire/agent_hello",
        serde_json::json!({"token": null, "client": {"name": "omp", "version": "18.16.0"}, "tools": 23}),
    )
    .unwrap_or_else(|e| panic!("the hello answers: {e}"));

    let after = words::<WishTitle>(&mut app, &id, "title");
    assert!(
        after.starts_with("omp 18.16.0:"),
        "the row still names nobody after the agent introduced itself: {after:?}"
    );
}

/// **2. Discard opens nothing, and the agent reads `discarded`.**
#[test]
fn discard_opens_nothing_and_the_agent_reads_discarded() {
    let mut app = ready(&format!("wish_discard_{}", std::process::id()));
    let id = queue_a_wish(&mut app);
    show_the_block(&mut app, true);

    unfold(&mut app, &id);
    let discard = entity_for::<WishDiscard>(&mut app, &id, "Discard button");
    press(&mut app, discard);

    assert!(harness::opened_links(&app).is_empty(), "Discard opened something: {:?}", harness::opened_links(&app));
    let read = wishes(&mut app);
    assert_eq!(read[0]["state"], "discarded", "the agent does not read the wish as discarded: {read:?}");
    assert!(!shows::<WishDiscard>(&mut app, &id), "a discarded wish still offers the button that dropped it");
}

/// **3. A hidden block counts its waiting wishes in the status band, and the chip opens the block.**
#[test]
fn a_hidden_block_shows_the_chip_and_the_chip_opens_the_block_on_the_row() {
    let mut app = ready(&format!("wish_chip_{}", std::process::id()));
    let id = queue_a_wish(&mut app);
    assert!(!block_is_open(&mut app), "the block is not there until it is asked for");

    let chip = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<WishChip>>()
            .single(world)
            .expect("a waiting wish with the block hidden is counted in the status band")
    };
    let said = app
        .world_mut()
        .query_filtered::<&Text, With<WishChipText>>()
        .iter(app.world())
        .next()
        .map(|text| text.0.clone())
        .unwrap_or_else(|| panic!("the chip carries the count"));
    assert_eq!(said, "1 waiting for you", "the chip does not count the waiting wish: {said:?}");

    click(&mut app, chip);

    assert!(block_is_open(&mut app), "the chip did not open the AGENT block");
    assert!(
        shows::<WishBody>(&mut app, &id),
        "the chip opened the block but not the row it was counting"
    );
}

/// **4. The check never blocks the person**: a search that fails says why, and Open still opens.
#[test]
fn a_failed_search_names_its_reason_and_open_still_opens() {
    let mut app = ready(&format!("wish_search_fail_{}", std::process::id()));
    let id = queue_a_wish(&mut app);
    show_the_block(&mut app, true);
    unfold(&mut app, &id);

    let said = words::<WishSearchLine>(&mut app, &id, "search line");
    assert!(said.contains("GitHub"), "the failed search does not say what failed: {said:?}");
    assert!(
        !said.contains("found") || said.contains("not"),
        "a failed search reads as a search that found something: {said:?}"
    );

    let url = url_of(&app, &id);
    let open = entity_for::<WishOpen>(&mut app, &id, "Open on GitHub button");
    press(&mut app, open);
    assert_eq!(harness::opened_links(&app), vec![url], "a failed search stopped the person opening the wish");
}

/// **5. A search that matches lists the issues**, so the person can join one instead of filing another.
#[test]
fn a_search_that_matches_lists_the_issues_before_opening() {
    let mut app = ready(&format!("wish_search_hit_{}", std::process::id()));
    app.insert_resource(WishSearchOff(Ok(vec![
        WishMatch {
            number: 401,
            title: "Agent wish: grimoire_find_piece".to_owned(),
            url: "https://github.com/dark-incorporated/bevy_dark_factory/issues/401".to_owned(),
        },
        WishMatch {
            number: 402,
            title: "Agent wish: grimoire_find_piece, by label".to_owned(),
            url: "https://github.com/dark-incorporated/bevy_dark_factory/issues/402".to_owned(),
        },
    ])));
    let id = queue_a_wish(&mut app);
    show_the_block(&mut app, true);
    unfold(&mut app, &id);

    let said = words::<WishSearchLine>(&mut app, &id, "search line");
    assert!(said.contains("#401"), "the matching issues are not listed: {said:?}");
    assert!(said.contains("#402"), "the matching issues are not listed: {said:?}");
    assert!(
        said.contains("https://github.com/dark-incorporated/bevy_dark_factory/issues/401"),
        "a match is not shown as a link: {said:?}"
    );
    // **The wish is still waiting**: a match informs the person, and the answer is still theirs.
    assert_eq!(wishes(&mut app)[0]["state"], "waiting");
    assert!(shows::<WishOpen>(&mut app, &id), "a search that matched took the Open button away");
}

