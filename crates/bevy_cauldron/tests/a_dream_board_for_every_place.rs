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

// ── A board that does not read, the header check over every board, and what `board_set` declares ─────────────────

#[test]
fn a_board_that_does_not_read_is_refused_and_never_overwritten_and_the_state_names_it() {
    let (mut app, mut ctx, root) = editor("board_unreadable");
    let file = root.join(board::file_of("design/levels/m"));
    std::fs::create_dir_all(file.parent().unwrap_or(&root)).unwrap();
    for (what, bytes) in [("malformed YAML", "pictures: [\n  - path: art/a.png\n"), ("another version", "version: 2\npictures: []\n")] {
        std::fs::write(&file, bytes).unwrap();
        for call in [
            json!({"action": "add", "path": "art/new.png", "caption": "the new one"}),
            json!({"action": "remove", "path": "art/a.png"}),
            json!({"action": "caption", "path": "art/a.png", "caption": "again"}),
        ] {
            let said = refusal(&mut ctx, call.clone());
            assert!(said.contains("board"), "{what}: {call}: the refusal names the board: {said}");
            assert_eq!(std::fs::read_to_string(&file).unwrap(), bytes, "{what}: {call}: a board that does not read is never overwritten");
        }
        let state = grimoire_state(&mut app);
        let level = state["board"]["places"].as_array().unwrap().iter().find(|p| p["place"] == "design/levels/m").unwrap_or_else(|| panic!("{state}")).clone();
        assert!(level["problem"].as_str().is_some_and(|p| !p.is_empty()), "{what}: grimoire_state.board names the problem: {level}");
        assert!(level.get("pictures").is_none(), "{what}: and lists no pictures from it: {level}");
    }
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
    b.extend(w.to_be_bytes());
    b.extend(h.to_be_bytes());
    b.extend([8, 6, 0, 0, 0]);
    b
}

fn pump(app: &mut App, secs: f64) {
    let start = std::time::Instant::now();
    while start.elapsed().as_secs_f64() < secs {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(40));
    }
}

fn verdict(app: &mut App, file: &str) -> Value {
    grimoire_state(app)["reloads"]["kinds"]
        .as_array()
        .and_then(|k| k.iter().find(|v| v["kind"] == "reference_images" && v["file"] == file).cloned())
        .unwrap_or(Value::Null)
}

/// **The header check runs over every board of the project** (#200, SG-03): a picture on a region's board, and on the game's, that
/// is no picture is refused naming the file and the field, as one on the level's board is.
#[test]
fn a_picture_on_any_boards_that_is_no_picture_is_named_by_the_header_check() {
    let (mut app, mut ctx, root) = editor("board_header_every");
    let on = [("design/regions/north", "art/harbour.png"), ("design", "art/game.png")];
    for (place, file) in on {
        std::fs::create_dir_all(root.join(file).parent().unwrap_or(&root)).unwrap();
        std::fs::write(root.join(file), png(8, 8)).unwrap();
        ops::dispatch(&mut ctx, "board_set", &json!({"action": "add", "place": place, "path": file, "caption": "the fog's colour"})).unwrap_or_else(|e| panic!("{}", e.0));
    }
    pump(&mut app, 1.5);
    for (_, file) in on {
        assert_eq!(verdict(&mut app, file), Value::Null, "a picture seen reading for the first time is not news: {file}");
    }
    for (place, file) in on {
        std::fs::write(root.join(file), b"a text where the picture was").unwrap();
        let mut seen = Value::Null;
        for _ in 0..60 {
            pump(&mut app, 0.1);
            seen = verdict(&mut app, file);
            if seen != Value::Null {
                break;
            }
        }
        assert_eq!(seen["outcome"], "refused", "{place}: {file}: {seen}");
        assert_eq!(seen["file"], file, "{seen}");
        assert_eq!(seen["field"], "signature", "{place}: the verdict names the header field: {seen}");
        assert_eq!(std::fs::read(root.join(file)).unwrap(), b"a text where the picture was", "the picture is the author's: nothing is written");
    }
}

fn stat(dir: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, (u64, std::time::SystemTime)> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                stack.push(e.path());
            } else if let Ok(modified) = meta.modified() {
                out.insert(e.path(), (meta.len(), modified));
            }
        }
    }
    out
}

/// **`board_set`'s declaration is true** (the reload contract's ratchet, for the one operation that copies a file in): a drop from outside the
/// project writes the board and the picture, and each is a kind the operation declares, wherever the place is.
#[test]
fn a_dropped_picture_dispatched_through_board_set_changes_only_the_kinds_it_declares() {
    use bevy_grimoire_core::reload::{self, Base};
    let (_app, mut ctx, root) = editor("board_kinds");
    let outside = std::env::temp_dir().join(format!("board_kinds_drop_{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    let dropped = outside.join("poster.png");
    std::fs::write(&dropped, b"a poster").unwrap();

    let declared = ops::find("board_set").expect("a registered operation").writes();
    let mut kinds: Vec<&str> = Vec::new();
    for (place, call) in [
        (None, json!({"action": "add", "path": dropped.to_str().unwrap(), "caption": "the poster's blue"})),
        (Some("design/regions/north"), json!({"action": "add", "place": "design/regions/north", "path": dropped.to_str().unwrap(), "caption": "its blue again"})),
        (Some("design"), json!({"action": "add", "place": "design", "path": dropped.to_str().unwrap(), "caption": "and for the game"})),
    ] {
        let before = stat(&root);
        ops::dispatch(&mut ctx, "board_set", &call).unwrap_or_else(|e| panic!("{}", e.0));
        let after = stat(&root);
        let mut changed: Vec<&std::path::PathBuf> = after.iter().filter(|(p, now)| before.get(*p) != Some(*now)).map(|(p, _)| p).collect();
        changed.extend(before.keys().filter(|p| !after.contains_key(*p)));
        assert!(changed.len() >= 2, "{place:?}: the board and the picture were written: {changed:?}");
        for path in changed {
            let rel = path.strip_prefix(&root).expect("under the project root").to_string_lossy().replace('\\', "/");
            let kind = match rel.strip_prefix("assets/grimoire/") {
                Some(under) => reload::kind_of(Base::Grimoire, under),
                None => reload::kind_of(Base::Root, &rel),
            }
            .unwrap_or_else(|| panic!("board_set changed `{rel}`, which is no kind in bevy_grimoire_core::reload::KINDS"));
            assert!(kind.id == "oplog" || declared.contains(&kind.id), "board_set changed `{rel}` (kind `{}`) and declares only {declared:?}", kind.id);
            kinds.push(kind.id);
        }
    }
    assert!(kinds.contains(&"board") && kinds.contains(&"reference_images"), "both kinds were exercised: {kinds:?}");
    std::fs::remove_dir_all(&outside).unwrap();
}
