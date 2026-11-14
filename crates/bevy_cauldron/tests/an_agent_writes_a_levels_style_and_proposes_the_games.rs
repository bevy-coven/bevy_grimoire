//! **An agent writes a level's or an area's style directly, and the game's or a region's by proposal** (SG-07, #406).
//!
//! A level's and an area's pages are changed by the registered operation `style_edit {path, edits: [{find, replace}]}`: logged, undoable,
//! followed (its `Shown::File` carries the changed lines) and allowed, because it was made through the editor in front of the person. The game's
//! and a region's pages hold the whole game together, so `grimoire_style_propose` only queues the change as a row in the AGENT block, and the
//! person's **Keep** writes it (through the same operation, as theirs) and **Discard** drops it.
//!
//! # What is proved
//!
//! 1. `style_edit` on a level's `style.md` writes the new text, answers the changed lines in `shown`, and its undo puts the file back byte for byte.
//! 2. A `find` that matches twice, an edit to the game's page, to a `board.yaml` and to a page the person has not allowed are each refused with the
//!    reason, and nothing is written.
//! 3. A proposal to `design/style.md` waits as a row; Keep writes it and `grimoire_state` reports `kept`; Discard leaves the file byte for byte; Keep
//!    after an outside edit is refused naming the page.
//! 4. After a kept proposal, the next `grimoire_style` serves the new text with no trust question.
//! 5. Nothing a page says reaches `grimoire_state`.

use bevy::prelude::*;
use bevy_cauldron::familiar::FamiliarItem;
use bevy_cauldron::harness;
use bevy_cauldron::wish_view::{StyleDiscard, StyleKeep, WishOf, WishUnfold};
use bevy_grimoire_core::trust::TrustStore;
use bevy_gooey::{Activate, ValueChange};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::fixtures;

const GAME_PAGE: &str = "design/style.md";
const LEVEL_PAGE: &str = "design/levels/m/style.md";
const GAME_TEXT: &str = "# The game\n\nWarm light.\nQuiet streets.\n";
const LEVEL_TEXT: &str = "# The courtyard\n\nA small walled yard.\nMoss on the stones.\nA well in the middle.\n";

/// **An editor on a project whose game and level pages exist and are allowed**, with a trust store of the test's own.
fn ready(name: &str) -> (App, PathBuf, PathBuf) {
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
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().trust.file = Some(store.clone());
    // **The follow crosses to the Design door and the door's state is rebuilt**, so the store is also named by the resource that outlives a door (its file is `trusted.yaml` in it).
    app.insert_resource(bevy_cauldron::trusted::StoreRoot(root.clone()));
    app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    updates(&mut app, 8);
    show_the_block(&mut app);
    (app, root, store)
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

fn show_the_block(app: &mut App) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FamiliarItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(app, 6);
}

/// **Apply `style_edit` as an agent**, answering the whole answer.
fn edit(app: &mut App, path: &str, edits: Value) -> Value {
    call(app, "grimoire/operation_apply", json!({"operation": "style_edit", "parameters": {"path": path, "edits": edits}, "dry_run": false})).unwrap_or_else(|e| panic!("{e}"))
}

fn refusal_of(answer: &Value) -> String {
    assert_eq!(answer["outcome"], "refused", "expected a refusal: {answer}");
    answer["refusal"].as_str().unwrap_or_else(|| panic!("a refusal names its reason: {answer}")).to_owned()
}

fn read(root: &Path, page: &str) -> String {
    std::fs::read_to_string(root.join(page)).unwrap_or_else(|e| panic!("{page}: {e}"))
}

/// **1. An edit to a level's page is applied, followed, and undone byte for byte.**
#[test]
fn an_edit_to_a_levels_style_is_written_followed_and_undone_byte_for_byte() {
    let (mut app, root, _store) = ready("style_edit_level");
    let before = read(&root, LEVEL_PAGE);
    let answer = edit(&mut app, LEVEL_PAGE, json!([{"find": "Moss on the stones.", "replace": "Moss on the stones, and ivy on the wall."}]));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    let after = read(&root, LEVEL_PAGE);
    assert!(after.contains("Moss on the stones, and ivy on the wall."), "the new text is on disk: {after}");
    assert_eq!(after, before.replace("Moss on the stones.", "Moss on the stones, and ivy on the wall."), "and nothing else changed");
    // **Followed**: the answer names the page and the changed lines, so the view goes to the Design page on them.
    assert_eq!(answer["shown"]["kind"], "file", "{answer}");
    assert_eq!(answer["shown"]["path"], LEVEL_PAGE);
    assert_eq!(answer["shown"]["lines"], json!([4, 4]), "the line that moved");

    let id = answer["operation_id"].as_str().expect("the operation has an id").to_owned();
    let undone = call(&mut app, "grimoire/operation_undo", json!({"operation_id": id, "dry_run": false})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    assert_eq!(read(&root, LEVEL_PAGE), before, "the undo puts the file back byte for byte");
}

/// **2. The refusals, each with its reason, and nothing written.**
#[test]
fn a_find_that_matches_twice_the_games_page_a_board_and_an_unallowed_page_are_refused() {
    let (mut app, root, store) = ready("style_edit_refused");
    // Two matches.
    std::fs::write(root.join(LEVEL_PAGE), "# The courtyard\n\nA stone.\nAnother stone.\n").expect("rewritten");
    let mut trusted = TrustStore::load(&store).expect("the store reads");
    trusted.trust_text(&root, LEVEL_PAGE, "# The courtyard\n\nA stone.\nAnother stone.\n").expect("allowed");
    trusted.save(&store).expect("saved");
    let twice = refusal_of(&edit(&mut app, LEVEL_PAGE, json!([{"find": "stone", "replace": "rock"}])));
    assert!(twice.contains("2 times") && twice.contains("exactly once"), "{twice}");
    let none = refusal_of(&edit(&mut app, LEVEL_PAGE, json!([{"find": "granite", "replace": "rock"}])));
    assert!(none.contains("0 times"), "{none}");
    assert_eq!(read(&root, LEVEL_PAGE), "# The courtyard\n\nA stone.\nAnother stone.\n", "a refusal writes nothing");

    // The game's page is changed by proposal only.
    let game = refusal_of(&edit(&mut app, GAME_PAGE, json!([{"find": "Warm light.", "replace": "Cold light."}])));
    assert!(game.contains("grimoire_style_propose") && game.contains("game"), "{game}");
    assert_eq!(read(&root, GAME_PAGE), GAME_TEXT);

    // A board and an area's rectangle have operations of their own.
    let board = refusal_of(&edit(&mut app, "design/levels/m/board/board.yaml", json!([{"find": "a", "replace": "b"}])));
    assert!(board.contains("board_set"), "{board}");
    let area = refusal_of(&edit(&mut app, "design/levels/m/areas/docks/area.yaml", json!([{"find": "a", "replace": "b"}])));
    assert!(area.contains("design_area_set"), "{area}");

    // A page the person has not allowed is not an agent's to change: it was never shown.
    std::fs::write(root.join(LEVEL_PAGE), "# The courtyard\n\nChanged by hand.\n").expect("rewritten");
    let withheld = refusal_of(&edit(&mut app, LEVEL_PAGE, json!([{"find": "hand", "replace": "foot"}])));
    assert!(withheld.contains("allow") && withheld.contains(LEVEL_PAGE), "{withheld}");
    assert_eq!(read(&root, LEVEL_PAGE), "# The courtyard\n\nChanged by hand.\n");
}

/// **A page may not outgrow the limits, or cite a picture that is not there.**
#[test]
fn an_edit_over_the_limits_or_citing_a_missing_picture_is_refused() {
    let (mut app, root, _store) = ready("style_edit_limits");
    let big = "x".repeat(bevy_grimoire_core::design::STYLE_FILE_MAX_BYTES);
    let over = refusal_of(&edit(&mut app, LEVEL_PAGE, json!([{"find": "Moss on the stones.", "replace": big}])));
    assert!(over.contains("bytes") && over.contains("32768"), "{over}");
    let picture = refusal_of(&edit(&mut app, LEVEL_PAGE, json!([{"find": "Moss on the stones.", "replace": "![moss](board/moss.png)"}])));
    assert!(picture.contains("board/moss.png") || picture.contains("moss.png"), "{picture}");
    assert_eq!(read(&root, LEVEL_PAGE), LEVEL_TEXT, "and nothing was written");
}

fn state_proposals(app: &mut App) -> Vec<Value> {
    let state = call(app, "grimoire/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    state["style_proposals"].as_array().unwrap_or_else(|| panic!("style_proposals is a list: {}", state["style_proposals"])).clone()
}

fn propose(app: &mut App, path: &str, find: &str, replace: &str) -> Result<Value, String> {
    call(app, "grimoire/style_propose", json!({"path": path, "edits": [{"find": find, "replace": replace}], "why": "the harbour reads cold"}))
}

fn unfold(app: &mut App, id: &str) {
    let entity = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(Entity, &WishOf), With<WishUnfold>>();
        query.iter(world).find(|(_, of)| of.0 == id).map(|(entity, _)| entity).unwrap_or_else(|| panic!("the AGENT block shows no row for `{id}`"))
    };
    app.world_mut().commands().trigger(Activate { entity });
    updates(app, 6);
}

fn press<T: Component>(app: &mut App, id: &str) {
    unfold(app, id);
    let entity = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &T, &WishOf)>();
        query.iter(world).find(|(_, _, of)| of.0 == id).map(|(entity, _, _)| entity).unwrap_or_else(|| panic!("the AGENT block shows no answer for `{id}`"))
    };
    app.world_mut().commands().trigger(Activate { entity });
    updates(app, 6);
}

/// **3. A proposal to the game's page waits; Keep writes it, Discard leaves it byte for byte, and Keep after an outside edit is refused.**
#[test]
fn a_proposal_for_the_games_style_is_kept_or_discarded_and_a_stale_one_is_refused() {
    let (mut app, root, _store) = ready("style_propose");
    let made = propose(&mut app, GAME_PAGE, "Warm light.", "Warm light, and long shadows.").unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(made["state"], "waiting", "{made}");
    assert_eq!(read(&root, GAME_PAGE), GAME_TEXT, "the call wrote nothing");
    let id = made["id"].as_str().expect("an id").to_owned();
    let rows = state_proposals(&mut app);
    assert_eq!((rows[0]["id"].as_str(), rows[0]["path"].as_str(), rows[0]["state"].as_str()), (Some(id.as_str()), Some(GAME_PAGE), Some("waiting")));

    // Keep writes it.
    press::<StyleKeep>(&mut app, &id);
    assert_eq!(read(&root, GAME_PAGE), GAME_TEXT.replace("Warm light.", "Warm light, and long shadows."), "Keep wrote the change");
    assert_eq!(state_proposals(&mut app)[0]["state"], "kept", "the agent reads `kept`");

    // **After a kept proposal the next `grimoire_style` serves the new text, with no trust question.**
    let served = call(&mut app, "grimoire/style", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert!(served["text"].as_str().is_some_and(|t| t.contains("long shadows")), "{served}");
    assert_eq!(served["withheld"], json!([]), "a kept change is allowed: {served}");

    // Discard leaves the file byte for byte.
    let kept = read(&root, GAME_PAGE);
    let second = propose(&mut app, GAME_PAGE, "Quiet streets.", "Loud streets.").unwrap_or_else(|e| panic!("{e}"));
    let second_id = second["id"].as_str().expect("an id").to_owned();
    press::<StyleDiscard>(&mut app, &second_id);
    assert_eq!(read(&root, GAME_PAGE), kept, "Discard wrote nothing");
    assert_eq!(state_proposals(&mut app)[1]["state"], "discarded");

    // **A proposal whose page changed after it was made is refused on Keep, naming the page.**
    let third = propose(&mut app, GAME_PAGE, "Quiet streets.", "Busy streets.").unwrap_or_else(|e| panic!("{e}"));
    let third_id = third["id"].as_str().expect("an id").to_owned();
    std::fs::write(root.join(GAME_PAGE), format!("{kept}Added by hand.\n")).expect("an outside edit");
    press::<StyleKeep>(&mut app, &third_id);
    assert_eq!(read(&root, GAME_PAGE), format!("{kept}Added by hand.\n"), "nothing was written over the outside edit");
    assert_eq!(state_proposals(&mut app)[2]["state"], "waiting", "a change that was not written is not an answer");
    let note = app.world().resource::<bevy_cauldron::style_proposals::StyleProposals>().queued[2].note.clone().unwrap_or_default();
    assert!(note.contains(GAME_PAGE) && note.contains("changed"), "the row says why and names the page: {note}");
}

/// **A proposal is refused at the door when it cannot apply, and a second for the same page waits for the first.**
#[test]
fn a_proposal_that_cannot_apply_is_refused_and_one_page_has_one_waiting() {
    let (mut app, root, _store) = ready("style_propose_refused");
    let nothing = propose(&mut app, GAME_PAGE, "A line the page does not say.", "x").expect_err("the find is not in the page");
    assert!(nothing.contains("0 times"), "{nothing}");
    let missing = propose(&mut app, "design/dialogue.md", "a", "b").expect_err("there is no such page");
    assert!(missing.contains("design/dialogue.md"), "{missing}");
    let first = propose(&mut app, GAME_PAGE, "Warm light.", "Warm, low light.").unwrap_or_else(|e| panic!("{e}"));
    let again = propose(&mut app, GAME_PAGE, "Quiet streets.", "Busy streets.").expect_err("one waiting proposal per page");
    assert!(again.contains(first["id"].as_str().expect("an id")), "{again}");
    assert_eq!(read(&root, GAME_PAGE), GAME_TEXT, "nothing was written");
    let long = "y".repeat(bevy_cauldron::style_proposals::STYLE_WHY_MAX_CHARS + 1);
    let why = call(&mut app, "grimoire/style_propose", json!({"path": LEVEL_PAGE, "edits": [{"find": "Moss on the stones.", "replace": "Moss."}], "why": long})).expect_err("a long why is refused");
    assert!(why.contains("`why`"), "{why}");
}

/// **5. What a page says is never in `grimoire_state`**: it names the page and the lines, not a word of its text.
#[test]
fn the_state_names_the_page_and_the_lines_and_never_its_text() {
    let (mut app, _root, _store) = ready("style_propose_state");
    propose(&mut app, GAME_PAGE, "Warm light.", "Warm light, and long shadows.").unwrap_or_else(|e| panic!("{e}"));
    let state = call(&mut app, "grimoire/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let said = state["style_proposals"].to_string();
    for word in ["Warm", "shadows", "Quiet streets"] {
        assert!(!said.contains(word), "`{word}` is a page's text, and the state carries none: {said}");
    }
    assert_eq!(state["style_proposals"][0]["lines"], json!([3, 3]));
}
