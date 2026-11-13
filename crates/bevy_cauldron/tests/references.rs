//! **`board_set` keeps a place's dream board as project data** (#124; SG-03, #402): an add, a re-caption, a removal and an
//! undo each reach the level's `board/board.yaml`, are one log line each, and a picture is named by its path.
//!
//! Owner exception (2026-11-13): this file tested `reference_set` and `maps/<map>.references.yaml`, which the board replaced; each
//! check it made is kept on the operation that took its place (the `r1` ids went with it).

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_grimoire_core::board::{self, Board};
use serde_json::json;

use crate::fixtures;

fn id_of(a: Applied) -> String {
    match a {
        Applied::Done { operation_id, .. } => operation_id,
        Applied::Refused(e) => panic!("refused: {}", e.0),
    }
}

/// The lines of the project's operation log: what "one log line" counts.
fn log_lines(ctx: &Ctx) -> usize {
    let dir = &ctx.project.as_ref().expect("a project").project_dir;
    std::fs::read_to_string(dir.join("oplog.jsonl")).map_or(0, |t| t.lines().count())
}

fn open_level(tag: &str) -> (bevy::prelude::App, Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let open = app.world().resource::<OpenMap>().clone();
    let project = app.world().resource::<Project>().clone();
    let file = project.root.join(board::file_of(&board::level_place(&project.root, &open.map.name)));
    let ctx = Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() };
    (app, ctx, file)
}

#[test]
fn a_board_keeps_two_pictures_in_order_and_each_step_is_one_log_line_and_one_undo() {
    let (_app, mut ctx, file) = open_level(&format!("board_steps_{}", std::process::id()));
    assert!(file.ends_with("design/levels/m/board/board.yaml"), "the level's board is the open map's: {}", file.display());
    let step = |ctx: &mut Ctx, p: serde_json::Value| -> String {
        let before = log_lines(ctx);
        let id = id_of(ops::dispatch(ctx, "board_set", &p).unwrap_or_else(|e| panic!("refused: {}", e.0)));
        assert_eq!(log_lines(ctx), before + 1, "{p} is one log line");
        id
    };
    step(&mut ctx, json!({"action": "add", "path": "refs/court.png", "caption": "how the roofs step down the hill"}));
    step(&mut ctx, json!({"action": "add", "path": "refs/fog.jpg", "caption": "the fog's colour, not its density"}));
    step(&mut ctx, json!({"action": "caption", "path": "refs/court.png", "caption": "the columns, not the steps"}));
    let removal = step(&mut ctx, json!({"action": "remove", "path": "refs/court.png"}));
    assert_eq!(Board::load(&file).unwrap().pictures.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), ["refs/fog.jpg"]);

    // Undo the removal: the picture is back where it stood, with its new caption, and the undo is one log line too.
    let before = log_lines(&ctx);
    ops::undo(&mut ctx, Some(&removal)).unwrap_or_else(|e| panic!("undo refused: {}", e.0));
    assert_eq!(log_lines(&ctx), before + 1);
    let saved = Board::load(&file).unwrap();
    assert_eq!(saved.pictures.iter().map(|e| e.path.as_str()).collect::<Vec<_>>(), ["refs/court.png", "refs/fog.jpg"]);
    assert_eq!(saved.pictures[0].caption, "the columns, not the steps");
}

#[test]
fn a_picture_is_refused_twice_outside_the_project_or_without_saying_what_to_take_and_nothing_is_written() {
    let (_app, mut ctx, file) = open_level(&format!("board_refused_{}", std::process::id()));
    let add = |ctx: &mut Ctx, p: &str, caption: &str| ops::dispatch(ctx, "board_set", &json!({"action": "add", "path": p, "caption": caption}));
    let first = id_of(add(&mut ctx, "refs/court.png", "like this").unwrap());
    // The same picture twice, a path outside the project, a non-image and an empty caption are refused, nothing written.
    for (bad, caption) in [("refs/court.png", "again"), ("../x.png", "x"), ("notes.txt", "x"), ("refs/empty.png", "  ")] {
        let refused = !matches!(add(&mut ctx, bad, caption), Ok(Applied::Done { .. }));
        assert!(refused, "{bad} ({caption:?}) was accepted");
    }
    assert_eq!(Board::load(&file).unwrap().pictures.len(), 1);

    // Undo the add: no picture is left, and a new one is the first again.
    ops::undo(&mut ctx, Some(&first)).unwrap();
    assert!(!file.exists() || Board::load(&file).unwrap().pictures.is_empty());
    add(&mut ctx, "refs/other.jpg", "the lintel").unwrap();
    assert_eq!(Board::load(&file).unwrap().pictures[0].path, "refs/other.jpg");
}

#[test]
fn undoing_a_removal_puts_the_picture_back_where_it_stood_and_a_move_reorders() {
    let (_app, mut ctx, file) = open_level(&format!("board_undo_{}", std::process::id()));
    for p in ["a.png", "b.png", "c.png"] {
        ops::dispatch(&mut ctx, "board_set", &json!({"action": "add", "path": p, "caption": format!("what to take from {p}")})).unwrap();
    }
    let removal = id_of(ops::dispatch(&mut ctx, "board_set", &json!({"action": "remove", "path": "b.png"})).unwrap());
    ops::undo(&mut ctx, Some(&removal)).unwrap_or_else(|e| panic!("undo refused: {}", e.0));
    let paths = |f: &std::path::Path| Board::load(f).unwrap().pictures.iter().map(|e| e.path.clone()).collect::<Vec<_>>();
    assert_eq!(paths(&file), ["a.png", "b.png", "c.png"]);
    ops::dispatch(&mut ctx, "board_set", &json!({"action": "move", "path": "c.png", "at": 0})).unwrap();
    assert_eq!(paths(&file), ["c.png", "a.png", "b.png"]);
}
