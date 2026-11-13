//! **A dream board for every place** (SG-03, #402): the pictures that show how a place should feel, in `<place>/board/board.yaml`,
//! changed only through `board_set`, and read back in `grimoire_state.board`.
//!
//! What is proved here, beside the re-pointed `references.rs`, `a_dropped_image_becomes_a_reference.rs` and
//! `a_reference_image_that_does_not_read_is_named.rs`: a place other than the level's has a board of its own; a restricted
//! picture is never put where a project ships; a picture missing on this machine stays on the board and says so; and a project
//! that still holds a `maps/<map>.references.ron` (or `.yaml`) gets the level's board, with the same captions in the same order,
//! through the migration — and `grimoire_state.board` lists them.

use bevy::prelude::*;
use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_grimoire_core::board::{self, Board};
use serde_json::{Value, json};

use crate::fixtures;

fn editor(tag: &str) -> (App, Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let app = opened(&root);
    let ctx = Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        session: tag.to_owned(),
        ..Ctx::default()
    };
    (app, ctx, root)
}

fn opened(root: &std::path::Path) -> App {
    let mut app = bevy_cauldron::harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn grimoire_state(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/state").cloned() else {
        panic!("grimoire/state is not registered");
    };
    app.world_mut().run_system_with(id, Some(json!({}))).unwrap_or_else(|e| panic!("{e:?}")).unwrap_or_else(|e| panic!("{e:?}"))
}

fn refusal(ctx: &mut Ctx, params: Value) -> String {
    match ops::dispatch(ctx, "board_set", &params) {
        Ok(Applied::Done { outcome, .. }) => panic!("{params} was accepted: {}", outcome.summary),
        Ok(Applied::Refused(e)) | Err(e) => e.0,
    }
}

#[test]
fn a_restricted_picture_is_refused_under_assets_and_a_dropped_one_is_never_copied_there() {
    let (_app, mut ctx, root) = editor("board_restricted");
    let said = refusal(&mut ctx, json!({"action": "add", "path": "assets/shots/store.png", "caption": "the shelf spacing", "restricted": true}));
    assert!(said.contains("restricted") && said.contains("assets"), "{said}");
    assert!(!board::places_with_a_board(&root).iter().any(|p| p == "design/levels/m"), "a refusal writes no board");

    // A restricted picture from outside the project is copied into the place's board/, which is not under assets/.
    let outside = std::env::temp_dir().join(format!("board_restricted_drop_{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    let poster = outside.join("poster.png");
    std::fs::write(&poster, b"a film poster").unwrap();
    ops::dispatch(
        &mut ctx,
        "board_set",
        &json!({"action": "add", "path": poster.to_str().unwrap(), "caption": "the poster's blue, not its type", "restricted": true, "creator": "a studio", "licence": "all rights reserved"}),
    )
    .unwrap_or_else(|e| panic!("{}", e.0));
    let level = board::level_place(&root, "m");
    let saved = Board::of_place(&root, &level).unwrap();
    assert_eq!(saved.pictures[0].path, "design/levels/m/board/poster.png");
    assert!(saved.pictures[0].restricted && saved.pictures[0].creator.as_deref() == Some("a studio"));
    assert!(!board::ships(&saved.pictures[0].path), "a restricted picture is not where the project ships");
    assert!(root.join("design/levels/m/board/poster.png").is_file());
    assert!(!root.join("assets/poster.png").exists() && !root.join("assets/grimoire/poster.png").exists());
    std::fs::remove_dir_all(&outside).unwrap();
}

#[test]
fn every_place_has_a_board_of_its_own_and_a_missing_picture_stays_listed() {
    let (mut app, mut ctx, root) = editor("board_places");
    ops::dispatch(&mut ctx, "board_set", &json!({"action": "add", "place": "design/regions/north", "path": "art/harbour.jpg", "caption": "the fog's colour, not its density"}))
        .unwrap_or_else(|e| panic!("{}", e.0));
    ops::dispatch(&mut ctx, "board_set", &json!({"action": "add", "place": "design", "path": "art/game.png", "caption": "the weight of the shadows"})).unwrap_or_else(|e| panic!("{}", e.0));
    assert!(root.join("design/regions/north/board/board.yaml").is_file());
    assert!(root.join("design/board/board.yaml").is_file());
    assert!(refusal(&mut ctx, json!({"action": "add", "place": "assets/grimoire", "path": "a.png", "caption": "x"})).contains("not a place"));

    let state = grimoire_state(&mut app);
    let places = state["board"]["places"].as_array().expect("grimoire_state.board.places");
    let place = |name: &str| places.iter().find(|p| p["place"] == name).unwrap_or_else(|| panic!("{name} is not in {state}"));
    let harbour = &place("design/regions/north")["pictures"][0];
    assert_eq!(harbour["caption"], "the fog's colour, not its density");
    assert_eq!(harbour["missing"], true, "a picture that is not on this machine is listed with `missing: true`");
    assert_eq!(state["board"]["level"], "design/levels/m");
    assert!(state["board"].get("references").is_none() && state.get("references").is_none(), "`references` is gone from the state");
    assert_eq!(Board::of_place(&root, "design/regions/north").unwrap().pictures.len(), 1, "a missing picture is never removed");
}

#[test]
fn a_references_file_becomes_the_levels_board_with_the_same_captions_in_the_same_order() {
    let tag = format!("board_migrates_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let old = root.join("assets/grimoire/maps/m.references.ron");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(
        &old,
        "(version: 1, refs: [(id: \"r1\", path: \"art/court.png\", caption: \"how the roofs step down the hill\"), (id: \"r2\", path: \"art/fog.jpg\", caption: \"the fog's colour, not its density\")], issued: 2)",
    )
    .unwrap();
    assert!(Project::open(&root, None).is_err(), "a project holding a references.ron is refused until it is migrated");

    let report = bevy_grimoire_migrate::convert_project(&root).unwrap_or_else(|p| panic!("{}", p.line()));
    assert_eq!(report.converted.len(), 1, "{:?}", report.converted);
    assert_eq!(report.converted[0].yaml, "design/levels/m/board/board.yaml");
    assert!(!old.exists(), "the old file leaves the project");
    assert!(root.join(".dark_factory/migrated-ron/assets/grimoire/maps/m.references.ron").is_file(), "and goes where every other migrated file goes");

    let saved = Board::of_place(&root, "design/levels/m").unwrap();
    assert_eq!(
        saved.pictures.iter().map(|e| (e.path.as_str(), e.caption.as_str())).collect::<Vec<_>>(),
        [("art/court.png", "how the roofs step down the hill"), ("art/fog.jpg", "the fog's colour, not its density")]
    );

    let mut app = opened(&root);
    let state = grimoire_state(&mut app);
    let level = state["board"]["places"].as_array().unwrap().iter().find(|p| p["place"] == "design/levels/m").unwrap_or_else(|| panic!("{state}")).clone();
    let captions: Vec<&str> = level["pictures"].as_array().unwrap().iter().map(|p| p["caption"].as_str().unwrap()).collect();
    assert_eq!(captions, ["how the roofs step down the hill", "the fog's colour, not its density"]);
}
