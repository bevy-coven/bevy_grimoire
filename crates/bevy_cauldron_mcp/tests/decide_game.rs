//! **`grimoire_decide` on the running game** (#114): a frame through the game's `bevy_fairy` and its state
//! over BRP, asked of the same checks and questions as the editor's. A stand-in game answers fairy's
//! methods with a frame captured from fairy's own fixture game (`tests/fixtures/game_frame.png`); a stand-in
//! for the recorded model answers the question. No live model, no game process.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use bevy_grimoire_decide::{Answer, Checks, Error, Question, Request, Response, Transport};
use bevy_cauldron_mcp::decide::{DecideParams, GameParams, StateCall, decide_with, fetch_game, load_checks, needs_game};
use serde_json::{Value, json};

fn frame() -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game_frame.png")).expect("the fixture frame")
}

/// A game that answers `bevy_fairy/screenshot` with an id, `bevy_fairy/capture` with the frame, and any other
/// method with `state`. `calls` records every method it was asked.
struct FakeGame {
    port: u16,
    calls: Arc<Mutex<Vec<(String, Value)>>>,
}

fn serve(state: Value) -> FakeGame {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    let port = listener.local_addr().unwrap().port();
    let calls: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let seen = calls.clone();
    let png = base64::engine::general_purpose::STANDARD.encode(frame());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut raw = Vec::new();
            let mut buf = [0u8; 4096];
            let body = loop {
                let n = s.read(&mut buf).unwrap_or(0);
                raw.extend_from_slice(&buf[..n]);
                if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                        .unwrap_or(0);
                    if raw.len() >= end + 4 + len {
                        break raw[end + 4..end + 4 + len].to_vec();
                    }
                }
                if n == 0 {
                    break Vec::new();
                }
            };
            let Ok(req) = serde_json::from_slice::<Value>(&body) else { continue };
            let method = req["method"].as_str().unwrap_or_default().to_owned();
            seen.lock().unwrap().push((method.clone(), req["params"].clone()));
            let result = match method.as_str() {
                "bevy_fairy/screenshot" => json!({"id": 1}),
                "bevy_fairy/capture" => json!({"status": "done", "png_base64": png}),
                _ => state.clone(),
            };
            let out = json!({"jsonrpc": "2.0", "id": req["id"], "result": result}).to_string();
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
                out.len()
            );
        }
    });
    FakeGame { port, calls }
}

/// The recorded model: P(yes) 0.9 when the one picture it is sent is the game's frame, 0.05 otherwise. It
/// keeps the request, so a test reads what the model was told.
struct Judge {
    asked: Mutex<Vec<Request>>,
}
impl Transport for Judge {
    fn endpoint(&self) -> String {
        "judge".into()
    }
    fn send(&self, r: &Request) -> Result<Response, Error> {
        self.asked.lock().unwrap().push(r.clone());
        let png = frame();
        let is_the_frame = r.images.len() == 1
            && r.images[0]
                .split_once("base64,")
                .and_then(|(_, b)| base64::engine::general_purpose::STANDARD.decode(b).ok())
                .is_some_and(|b| b == png);
        let p = if is_the_frame { 0.9 } else { 0.05 };
        let answers = r
            .questions
            .iter()
            .map(|(id, q)| match q {
                Question::Noul { .. } => (id.clone(), Answer::Noul { noul: p }),
                _ => unreachable!("only noul questions are asked here"),
            })
            .collect::<BTreeMap<_, _>>();
        Ok(Response { model: None, answers, usage: None })
    }
}

fn judge() -> Judge {
    Judge { asked: Mutex::new(Vec::new()) }
}

/// A project whose own `checks.ron` carries a check about the game, and whose `play.ron` names the port.
fn game_project(tag: &str, port: u16) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-decide-game-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets/grimoire")).unwrap();
    std::fs::write(
        dir.join("assets/grimoire/checks.ron"),
        r#"(checks: [(name: "hud_readable", question: "Is the HUD hard to read?", images: ["game"], facts: true, threshold: 0.6)])"#,
    )
    .unwrap();
    std::fs::write(dir.join("play.ron"), format!(r#"(version: 1, command: ["game"], port: {port})"#)).unwrap();
    dir
}

#[tokio::test(flavor = "multi_thread")]
async fn a_named_check_runs_on_a_frame_of_the_running_game_and_the_game_state_is_its_facts() {
    let game = serve(json!({"player": {"x": 3.0, "stuck": true}}));
    let dir = game_project("check", game.port);
    // The agent names only the check: it asks for the image `game`, so the server reads the game on play.ron's port.
    let p = DecideParams { check: Some("hud_readable".into()), ..Default::default() };
    let checks = load_checks(&dir).unwrap();
    assert!(needs_game(&p, &checks), "a check that asks for the image `game` reads the game");

    let ask = GameParams { port: None, state: Some(StateCall { method: "world.query".into(), params: Some(json!({"data": {}})) }) };
    let evidence = fetch_game(&dir, &ask).await.unwrap();
    assert_eq!(evidence.port, game.port);

    let model = judge();
    let out = decide_with(&p, &dir, None, Some(&evidence), &model).unwrap();
    assert_eq!(out["finding"]["p_yes"], 0.9, "the model was sent the game's own frame: {out}");
    assert_eq!(out["finding"]["found"], true);
    assert_eq!(out["message"], "hud_readable 0.90 > 0.60");
    assert_eq!(out["game"], json!({"port": game.port, "state": true}));
    let told = &model.asked.lock().unwrap()[0].state;
    assert!(told.contains(r#""player":{"stuck":true,"x":3.0}"#), "the game's state is the check's facts: {told}");

    let calls = game.calls.lock().unwrap();
    let methods: Vec<&str> = calls.iter().map(|(m, _)| m.as_str()).collect();
    assert_eq!(methods[0], "bevy_fairy/screenshot");
    assert!(methods.contains(&"bevy_fairy/capture") && methods.contains(&"world.query"), "{methods:?}");
    assert_eq!(calls[0].1, json!({}), "the game's own capture source is used, not the editor's `offscreen`");
    assert_eq!(calls.iter().find(|(m, _)| m == "world.query").unwrap().1, json!({"data": {}}));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn ad_hoc_questions_are_asked_of_the_game_frame_with_its_state_in_the_request() {
    let game = serve(json!({"player": {"stuck": true}}));
    let dir = game_project("adhoc", game.port);
    let p = DecideParams {
        questions: Some([("stuck".to_owned(), json!({"type": "noul", "instructions": "Is the player stuck against a wall?"}))].into()),
        state: Some("A fixture game.".into()),
        game: Some(GameParams { port: Some(game.port), state: Some(StateCall { method: "world.query".into(), params: None }) }),
        ..Default::default()
    };
    assert!(needs_game(&p, &load_checks(&dir).unwrap()));
    let evidence = fetch_game(&dir, p.game.as_ref().unwrap()).await.unwrap();
    let model = judge();
    let out = decide_with(&p, &dir, None, Some(&evidence), &model).unwrap();
    assert_eq!(out["answers"]["stuck"]["noul"], 0.9);
    let told = model.asked.lock().unwrap()[0].state.clone();
    assert!(told.starts_with("A fixture game.") && told.contains(r#""player":{"stuck":true}"#), "{told}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_game_check_is_the_games_own_and_the_editor_has_none() {
    assert!(Checks::builtin().get("hud_readable").is_none(), "no editor check is about a game's HUD");
    let dir = game_project("own", 1);
    let checks = load_checks(&dir).unwrap();
    let check = checks.get("hud_readable").expect("the game's own checks.ron defines it");
    assert_eq!(check.images, ["game"]);
    assert!(checks.get("floating").is_some(), "the editor's checks are still there beside it");
    // The editor-side asks without a game get a refusal naming the missing frame, never an answer from less.
    let p = DecideParams { check: Some("hud_readable".into()), facts: Some(json!({})), ..Default::default() };
    let err = decide_with(&p, &dir, None, None, &judge()).unwrap_err();
    assert!(err.contains("needs game"), "{err}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_frame_given_as_a_file_is_not_replaced_by_reading_the_game() {
    let checks = Checks::from_ron(r#"(checks: [(name: "g", question: "Q?", images: ["game"], threshold: 0.5)])"#).unwrap();
    let p = DecideParams { check: Some("g".into()), ..Default::default() };
    assert!(needs_game(&p, &checks));
    let given = DecideParams {
        check: Some("g".into()),
        images: vec![bevy_cauldron_mcp::decide::NamedImage { name: "game".into(), path: "game.png".into() }],
        ..Default::default()
    };
    assert!(!needs_game(&given, &checks));
    assert!(!needs_game(&DecideParams::default(), &checks));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_game_that_is_not_running_is_named_with_its_port_and_never_the_editor() {
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let dir = game_project("down", port);
    let err = fetch_game(&dir, &GameParams::default()).await.unwrap_err();
    assert!(err.contains(&format!("the running game on port {port}")), "{err}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_game_state_beside_facts_that_use_its_key_is_refused_before_the_model_is_asked() {
    let game = serve(json!({"a": 1}));
    let dir = game_project("clash", game.port);
    let p = DecideParams {
        check: Some("hud_readable".into()),
        facts: Some(json!({"game": "mine"})),
        game: Some(GameParams { port: None, state: Some(StateCall { method: "world.query".into(), params: None }) }),
        ..Default::default()
    };
    let evidence = fetch_game(&dir, p.game.as_ref().unwrap()).await.unwrap();
    let model = judge();
    let err = decide_with(&p, &dir, None, Some(&evidence), &model).unwrap_err();
    assert!(err.contains("facts.game"), "{err}");
    assert!(model.asked.lock().unwrap().is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
