//! **`play` starts the project's game on the open map, and `play_stop` ends it by pid** (#97).

use bevy_cauldron::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

/// A fixture game: a python process that answers `bevy_fairy/status` with `ready: true` on `port`
/// and writes the map and camera it was started with beside itself.
const GAME: &str = r#"
import http.server, json, os, sys
open(os.path.join(os.environ["GRIMOIRE_PLAY_ROOT"], "started.txt"), "w").write(
    sys.argv[1] + " " + os.environ.get("GRIMOIRE_PLAY_CAMERA", "") + " " + sys.argv[2])
class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers.get("content-length", 0)))
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"ready": True}}).encode()
        self.send_response(200); self.send_header("content-length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", int(sys.argv[3])), H).serve_forever()
"#;

fn alive(pid: u64) -> bool {
    std::process::Command::new("kill").args(["-0", &pid.to_string()]).stderr(std::process::Stdio::null()).status().unwrap().success()
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn ctx(tag: &str, port: u16) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("town");
    std::fs::write(root.join("game.py"), GAME).unwrap();
    std::fs::write(
        root.join("play.ron"),
        format!(r#"(version: 1, command: ["{py}", "game.py", "{{map}}", "{{camera}}", "{port}"], port: {port})"#, py = fixtures_python()),
    )
    .unwrap();
    (Ctx { root: Some(root.clone()), session: "test".to_owned(), ..Ctx::default() }, root)
}

/// The interpreter the fixture game runs on: the system's `/usr/bin/python3` where there is one.
/// Homebrew's Python 3.15.2 on bmb never answered the fixture's POST, so this test panicked before
/// `play_stop` and left the game running (#121; owner, 2026-10-30).
fn fixtures_python() -> &'static str {
    if std::path::Path::new("/usr/bin/python3").exists() { "/usr/bin/python3" } else { "python3" }
}

#[test]
fn play_starts_the_game_on_the_map_until_fairy_is_ready_and_stop_ends_it_by_pid() {
    let port = free_port();
    let (mut c, root) = ctx(&format!("play{}", std::process::id()), port);
    let applied = ops::dispatch(&mut c, "play", &json!({"map": "town", "camera": [1.0, 2.0, 3.0], "wait_s": 20.0})).unwrap();
    let Applied::Done { outcome, .. } = applied else { panic!("play was refused") };
    let after = outcome.after.unwrap();
    let pid = after["pid"].as_u64().unwrap();
    let _cleanup = KillGroupOnDrop(pid);
    assert_eq!(after["ready"], true, "fairy's status answered ready: {}", outcome.summary);
    assert!(alive(pid));
    assert_eq!(std::fs::read_to_string(root.join("started.txt")).unwrap(), "town 1,2,3 1,2,3");
    let again = ops::dispatch(&mut c, "play", &json!({"map": "town"}));
    assert!(matches!(again, Err(_) | Ok(Applied::Refused(_))), "a second Play while one runs is refused");
    let Applied::Done { outcome, .. } = ops::dispatch(&mut c, "play_stop", &json!({})).unwrap() else { panic!("stop refused") };
    assert_eq!(outcome.after.unwrap()["pid"].as_u64().unwrap(), pid);
    assert!(!alive(pid), "the process Play started is gone");
    assert!(matches!(ops::dispatch(&mut c, "play_stop", &json!({})), Err(_) | Ok(Applied::Refused(_))));
}

/// **A failing test still ends the game it started.** Before this, a panic between `play` and
/// `play_stop` left the game running. The orphan held the runner's output pipe open, so an agent's
/// `cargo test | …` or `git push` (the gate) never returned. Measured 2026-10-30 on big_mac and bmb:
/// eight orphans across both machines. `play` puts the game in its own process group, whose id is
/// its pid.
pub(crate) struct KillGroupOnDrop(pub(crate) u64);

impl Drop for KillGroupOnDrop {
    fn drop(&mut self) {
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &format!("-{}", self.0)])
            .stderr(std::process::Stdio::null())
            .status();
    }
}

#[test]
fn a_project_with_no_play_file_says_how_to_write_one() {
    let root = fixtures::Fixture::new(&format!("noplay{}", std::process::id())).descriptor("site/wall", "alpha").build("town");
    let mut c = Ctx { root: Some(root), session: "test".to_owned(), ..Ctx::default() };
    let e = match ops::dispatch(&mut c, "play", &json!({"map": "town"})) {
        Err(e) | Ok(Applied::Refused(e)) => e.0,
        Ok(_) => panic!("played without a play.ron"),
    };
    assert!(e.contains("play.ron") && e.contains("command"), "{e}");
}
