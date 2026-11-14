//! **An agent cannot copy a file from outside the project onto a board** (#419, found by the #402 review).
//!
//! `board_set add` copies an absolute path outside the project into the place's `board/`: that is a person's drop
//! and stays. For an agent (over BRP or MCP) it would let a prompt pull any readable image under the user's home
//! into a repository, so an operation whose source is not the person refuses it, naming the path, and copies nothing.

use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy_cauldron::ops::{self, Applied, Ctx, Source};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_grimoire_core::board::{self, Board};
use serde_json::{Value, json};

use crate::fixtures;

fn editor(tag: &str, source: Source) -> (bevy::prelude::App, Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    let ctx = Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        session: tag.to_owned(),
        source,
        ..Ctx::default()
    };
    (app, ctx, root)
}

fn outside_picture(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("secret.png");
    std::fs::write(&file, b"a picture outside the project").unwrap();
    (dir, file)
}

fn add(path: &std::path::Path) -> Value {
    json!({"action": "add", "path": path.to_str().unwrap(), "caption": "the blue, not the type"})
}

#[test]
fn an_agent_refused_a_path_outside_the_project_copies_nothing() {
    let (_app, mut ctx, root) = editor("agent_board_outside", Source::Agent);
    let (dir, file) = outside_picture("agent_board_outside_src");
    let params = add(&file);

    let said = match ops::dispatch(&mut ctx, "board_set", &params) {
        Ok(Applied::Done { outcome, .. }) => panic!("an agent's add of {} was accepted: {}", file.display(), outcome.summary),
        Ok(Applied::Refused(e)) | Err(e) => e.0,
    };
    assert!(said.contains(file.to_str().unwrap()), "the refusal names the path: {said}");
    assert!(said.contains("Nothing was changed"), "{said}");
    assert!(!root.join("design/levels/m/board").exists(), "nothing was copied into the board folder");
    assert!(!board::places_with_a_board(&root).iter().any(|p| p == "design/levels/m"), "no board file was written");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_agent_may_still_name_a_picture_inside_the_project_by_either_path() {
    let (_app, mut ctx, root) = editor("agent_board_inside", Source::Agent);
    std::fs::create_dir_all(root.join("assets/shots")).unwrap();
    std::fs::write(root.join("assets/shots/store.png"), b"a shot").unwrap();
    std::fs::write(root.join("assets/shots/aisle.png"), b"another shot").unwrap();

    // Project-relative.
    ops::dispatch(&mut ctx, "board_set", &json!({"action": "add", "path": "assets/shots/store.png", "caption": "the shelf spacing"})).unwrap_or_else(|e| panic!("{}", e.0));
    // Absolute, but inside the project: referenced where it is, never copied.
    let absolute = root.join("assets/shots/aisle.png");
    ops::dispatch(&mut ctx, "board_set", &json!({"action": "add", "path": absolute.to_str().unwrap(), "caption": "the aisle width"})).unwrap_or_else(|e| panic!("{}", e.0));
    let saved = Board::of_place(&root, &board::level_place(&root, "m")).unwrap();
    let paths: Vec<&str> = saved.pictures.iter().map(|p| p.path.as_str()).collect();
    assert_eq!(paths, ["assets/shots/store.png", "assets/shots/aisle.png"], "both are referenced where they are");
    for name in ["store.png", "aisle.png"] {
        assert!(!root.join("design/levels/m/board").join(name).exists(), "{name}: an in-project path is not copied");
    }
}

#[test]
fn a_persons_drop_from_outside_the_project_is_still_copied_in() {
    let (_app, mut ctx, root) = editor("human_board_outside", Source::Human);
    let (dir, file) = outside_picture("human_board_outside_src");
    ops::dispatch(&mut ctx, "board_set", &add(&file)).unwrap_or_else(|e| panic!("{}", e.0));
    assert!(root.join("design/levels/m/board/secret.png").is_file(), "the drop was copied into the place's board/");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// **The same refusal over the door an agent really comes through**: `grimoire/operation_apply`, planned (`dry_run`) and applied.
fn over_brp(app: &mut bevy::prelude::App, params: &Value, dry_run: bool) -> Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("grimoire/operation_apply").cloned() else {
        panic!("grimoire/operation_apply is not registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(json!({"operation": "board_set", "parameters": params, "dry_run": dry_run})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"))
}

/// **An agent cannot ask the host which files exist**: a file that is there and one that is not, outside the project, are refused with the
/// one sentence (the path aside), planned and applied, and a path that climbs out of the project with `..` is no different.
#[test]
fn an_agent_over_brp_is_refused_the_same_whether_or_not_the_outside_file_exists() {
    let tag = "agent_board_brp";
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    let (dir, there) = outside_picture("agent_board_brp_src");
    let missing = dir.join("missing.png");
    let climbing = root.join("..").join(dir.file_name().unwrap()).join("secret.png");
    assert!(climbing.canonicalize().is_ok(), "the climbing path leads to a real file, so only the sentence can tell the cases apart");

    let mut sentences = Vec::new();
    for path in [&there, &missing, &climbing] {
        for dry_run in [true, false] {
            let answer = over_brp(&mut app, &add(path), dry_run);
            assert_eq!(answer["outcome"], "refused", "{path:?} dry_run={dry_run}: {answer}");
            let said = answer["refusal"].as_str().unwrap_or_else(|| panic!("no refusal in {answer}")).to_owned();
            assert!(said.contains("outside the project"), "{said}");
            sentences.push(said.replace(path.to_str().unwrap(), "<path>"));
        }
    }
    assert!(sentences.windows(2).all(|w| w[0] == w[1]), "one sentence for every case: {sentences:#?}");
    assert!(!root.join("design/levels/m/board").exists(), "nothing was copied or written");
    std::fs::remove_dir_all(&dir).unwrap();
}
