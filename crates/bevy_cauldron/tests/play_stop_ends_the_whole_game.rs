//! **`play_stop` ends the game, not only the process `play` started** (#97).
//!
//! A game is usually started through a launcher: `play.ron`'s own example is `cargo run -- {map}`.
//! Killing only the launcher's pid left the game running with no parent, still answering on its
//! port. This test's launcher is `sh -c "… & wait"`. (Correction, 2026-10-30: the orphaned
//! `game.py` processes found on bmb were not this. `play_starts_and_stops_the_game` panicked
//! before `play_stop`, because Homebrew's Python 3.15 fixture server never answered; see #121.)

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::ops_play::fairy_ready;
use serde_json::json;
use std::time::{Duration, Instant};

use crate::fixtures;

/// The game: answers `bevy_fairy/status` with `ready: true` on the port in its first argument.
const GAME: &str = r#"
import http.server, json, sys
class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers.get("content-length", 0)))
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"ready": True}}).encode()
        self.send_response(200); self.send_header("content-length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
"#;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn stop_ends_a_game_started_through_a_launcher() {
    let port = free_port();
    let root = fixtures::Fixture::new(&format!("play_launcher{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("town");
    std::fs::write(root.join("game.py"), GAME).unwrap();
    // A launcher that starts the game as its child and waits on it, as `cargo run` does.
    std::fs::write(
        root.join("play.ron"),
        format!(r#"(version: 1, command: ["sh", "-c", "{} game.py {port} & wait"], port: {port})"#, fixtures_python()),
    )
    .unwrap();
    let mut c = Ctx { root: Some(root), session: "test".to_owned(), ..Ctx::default() };

    let Applied::Done { outcome, .. } =
        ops::dispatch(&mut c, "play", &json!({"map": "town", "wait_s": 20.0})).unwrap()
    else {
        panic!("play was refused")
    };
    let after = outcome.after.unwrap();
    let _cleanup = crate::play_starts_and_stops_the_game::KillGroupOnDrop(after["pid"].as_u64().unwrap());
    assert_eq!(after["ready"], true, "the game behind the launcher answered: {}", outcome.summary);

    let Applied::Done { .. } = ops::dispatch(&mut c, "play_stop", &json!({})).unwrap() else { panic!("stop refused") };
    let deadline = Instant::now() + Duration::from_secs(5);
    while fairy_ready(port) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        !fairy_ready(port),
        "the game still answers on port {port} after play_stop: the launcher died and its child, the game, did not"
    );
}

/// The interpreter the fixture game runs on: the system's `/usr/bin/python3` where there is one.
/// Homebrew's Python 3.15.2 on bmb never answered the fixture's POST (#121).
fn fixtures_python() -> &'static str {
    if std::path::Path::new("/usr/bin/python3").exists() { "/usr/bin/python3" } else { "python3" }
}
