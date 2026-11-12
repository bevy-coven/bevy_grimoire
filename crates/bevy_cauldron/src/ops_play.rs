//! **Play the open map in the game, and stop it** (#97): `play` and `play_stop`.
//!
//! The game command comes from the project's `play.ron`, never from the editor's code, and the
//! editor never links the game: `play` starts that command as a child process with the open map and
//! the editor camera's position in its arguments and environment (`GRIMOIRE_PLAY_MAP`,
//! `GRIMOIRE_PLAY_CAMERA`, `GRIMOIRE_PLAY_ROOT`). The game is expected to carry `bevy_fairy`; an agent
//! then reads its state, runs its commands and captures frames through fairy's own MCP. `play` can
//! wait for `bevy_fairy/status` to say `ready: true`; `play_stop` ends the game it started: the
//! whole process group, because a game is usually started through a launcher (`cargo run`), and
//! killing only the launcher's pid left the game running as an orphan.
//! Neither changes a document, so neither has an undo.

use bevy_grimoire_core::play::PlayConfig;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::ops::{Ctx, Operation, Outcome, Refusal, Shown, Undo};

/// The games this process started, by project root: one per project.
static RUNNING: Mutex<Option<HashMap<PathBuf, Child>>> = Mutex::new(None);

fn with_table<T>(f: impl FnOnce(&mut HashMap<PathBuf, Child>) -> T) -> T {
    let mut guard = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

/// **How the game this editor last ran ended** (NV-09, #386): what the Game workspace says after the process is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exit {
    /// The process's exit code, or `None` when it was ended by a signal (a crash that never got to say a code).
    pub code: Option<i32>,
    /// The editor's own `play_stop` ended it, so this is not a crash whatever the code was.
    pub stopped: bool,
}

impl Exit {
    /// **A crash is the game ending without being asked to and without success.** Exit `0` on its own is the game
    /// closing its window, which needs nothing from a person; a signal or any other code does.
    pub fn crashed(&self) -> bool {
        !self.stopped && self.code != Some(0)
    }
}

/// How each project's game last ended, by project root. Cleared when `play` starts another.
static EXITS: Mutex<Option<HashMap<PathBuf, Exit>>> = Mutex::new(None);

fn with_exits<T>(f: impl FnOnce(&mut HashMap<PathBuf, Exit>) -> T) -> T {
    let mut guard = EXITS.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

/// **How the game this editor started for `root` last ended**, or `None` while it runs or before any run.
pub fn last_exit(root: &std::path::Path) -> Option<Exit> {
    with_exits(|t| t.get(root).copied())
}

/// **The most bytes the game's log file holds.** A game that prints for an hour must not fill a disk: past this the file is
/// cut back to nothing and the run goes on, with one line saying so.
pub const LOG_CAP_BYTES: u64 = 1024 * 1024;

/// The line written where the log was cut.
const LOG_CUT: &[u8] = b"[the game's log passed its size cap and was cut here]\n";

/// **Where the game's output goes**: one file per project in a directory of the user's own in the temporary directory, named by a
/// hash of the root, so the log of a project is never inside it (a project is a folder a person shares), two projects never share
/// one and another user's files are never followed into. `play` truncates it, so it holds the last run only; `play_stop` and
/// closing the project remove it ([`remove_log`]).
pub fn log_path(root: &std::path::Path) -> PathBuf {
    const BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let hash = root.to_string_lossy().bytes().fold(BASIS, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME));
    let user: String = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "user".to_owned())
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    std::env::temp_dir().join(format!("grimoire-game-logs-{user}")).join(format!("{hash:016x}.log"))
}

/// **Open the game's log for a new run**, or say why not. The directory is made private to the user (mode 700 where there are modes).
fn open_log(root: &std::path::Path) -> Result<std::fs::File, String> {
    let path = log_path(root);
    let cannot = |e: std::io::Error| format!("could not open the game's log file {}: {e}", path.display());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(cannot)?;
        #[cfg(unix)]
        std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700)).map_err(cannot)?;
    }
    std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(&path).map_err(cannot)
}

/// **Remove the game's log** — when the game is stopped and when the project closes. A file that is not there is already removed.
pub fn remove_log(root: &std::path::Path) {
    let path = log_path(root);
    if let Err(e) = std::fs::remove_file(&path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        bevy::log::warn!("could not remove the game's log {}: {e}", path.display());
    }
}

/// **Copy one of the game's pipes into the log**, cutting the file back past [`LOG_CAP_BYTES`].
fn pump(mut from: impl std::io::Read, log: std::sync::Arc<Mutex<(std::fs::File, u64)>>) {
    use std::io::{Seek, SeekFrom, Write};
    let mut buffer = [0_u8; 8192];
    loop {
        let read = match from.read(&mut buffer) {
            Ok(0) => return,
            Ok(n) => n,
            Err(e) => {
                bevy::log::warn!("the game's output could not be read: {e}");
                return;
            }
        };
        let mut guard = log.lock().unwrap_or_else(|e| e.into_inner());
        let (file, written) = &mut *guard;
        let wrote = (|| {
            if *written + read as u64 > LOG_CAP_BYTES {
                file.set_len(0)?;
                file.seek(SeekFrom::Start(0))?;
                file.write_all(LOG_CUT)?;
                *written = LOG_CUT.len() as u64;
            }
            file.write_all(&buffer[..read])?;
            *written += read as u64;
            Ok::<(), std::io::Error>(())
        })();
        if let Err(e) = wrote {
            bevy::log::warn!("the game's log could not be written: {e}");
            return;
        }
    }
}

/// The pid of the game this editor started for `root`, if it is still running. A game that has ended has its
/// [`Exit`] recorded here, the one place the child is reaped.
pub fn running_pid(root: &std::path::Path) -> Option<u32> {
    with_table(|t| match t.get_mut(root) {
        Some(child) => match child.try_wait() {
            Ok(None) => Some(child.id()),
            ended => {
                if let Ok(Some(status)) = ended {
                    with_exits(|e| e.insert(root.to_path_buf(), Exit { code: status.code(), stopped: false }));
                }
                t.remove(root);
                None
            }
        },
        None => None,
    })
}

/// Whether the game answering on `port` says `ready: true` to `bevy_fairy/status`.
pub fn fairy_ready(port: u16) -> bool {
    let agent: ureq::Agent =
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_millis(500))).http_status_as_error(false).build().into();
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": "bevy_fairy/status"});
    let Ok(mut resp) = agent.post(format!("http://127.0.0.1:{port}")).send_json(&body) else {
        return false;
    };
    let Ok(v) = resp.body_mut().read_json::<serde_json::Value>() else {
        return false;
    };
    v.pointer("/result/ready").and_then(|r| r.as_bool()) == Some(true)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayParams {
    /// The map to play. Default: the map open in the editor.
    #[serde(default)]
    pub map: Option<String>,
    /// Where the editor's camera is, `[x, y, z]`, handed to the game.
    #[serde(default)]
    pub camera: Option<[f32; 3]>,
    /// Seconds to wait for the game's fairy to say it is ready. `0`: return at once.
    #[serde(default)]
    pub wait_s: f64,
}

fn parse<T: serde::de::DeserializeOwned>(raw: &serde_json::Value) -> Result<T, Refusal> {
    serde_json::from_value(raw.clone())
        .map_err(|e| Refusal::new(format!("these are not parameters of this operation: {e}.")))
}

/// **Start the project's game on the open map.**
pub struct Play;

impl Play {
    fn prepare(ctx: &Ctx, params: &serde_json::Value) -> Result<(PathBuf, PlayConfig, String, PlayParams), Refusal> {
        let p: PlayParams = parse(params)?;
        let Some(root) = ctx.root_or_project() else {
            return Err(Refusal::new("no project is open, so there is no game to play."));
        };
        let map = p
            .map
            .clone()
            .or_else(|| ctx.open.as_ref().map(|o| o.map.name.clone()))
            .ok_or_else(|| Refusal::new("no map is open and none was named, so there is nothing to play."))?;
        if let Some(pid) = running_pid(&root) {
            return Err(Refusal::new(format!("the game is already running (pid {pid}). Stop it with `play_stop` first.")));
        }
        let config = PlayConfig::load(&root).map_err(Refusal::new)?;
        // **No person has to have pressed anything for a project's `play.ron` to be here**: a cloned
        // project brings its own. Nothing starts until a person has allowed this command or this project.
        if let Some(request) = crate::trusted::needs_answer(ctx.trust_file.as_deref(), &root, &config.spec()).map_err(Refusal::new)? {
            return Err(Refusal::new(bevy_grimoire_core::trust::TrustStore::untrusted_sentence(&request.root, &request.spec)));
        }
        Ok((root, config, map, p))
    }
}

/// **Would `play` have to ask a person first?** The Play button asks before it dispatches, so the
/// question is raised instead of only a refusal. `Ok(None)` when the game is allowed, or when there is no
/// `play.ron` to be allowed (the operation says so itself).
pub fn untrusted_request(ctx: &Ctx, root: &std::path::Path) -> Result<Option<crate::trusted::TrustRequest>, String> {
    match PlayConfig::load(root) {
        Ok(config) => crate::trusted::needs_answer(ctx.trust_file.as_deref(), root, &config.spec()),
        Err(_) => Ok(None),
    }
}

impl Operation for Play {
    fn name(&self) -> &'static str {
        "play"
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::Nowhere
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "map": {"type": "string", "description": "The map to play. Default: the open map."},
                "camera": {"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3,
                    "description": "The editor camera's position [x, y, z], handed to the game."},
                "wait_s": {"type": "number", "description": "Seconds to wait for bevy_fairy/status to say ready. 0 returns at once."}
            }
        })
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let (root, config, map, p) = Self::prepare(ctx, params)?;
        Ok(json!({"argv": config.argv(&root, &map, p.camera), "map": map}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let (root, config, map, p) = Self::prepare(ctx, params)?;
        let argv = config.argv(&root, &map, p.camera);
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..])
            .current_dir(config.dir(&root))
            .env("GRIMOIRE_PLAY_MAP", &map)
            .env("GRIMOIRE_PLAY_ROOT", &root);
        if let Some(c) = p.camera {
            cmd.env("GRIMOIRE_PLAY_CAMERA", format!("{},{},{}", c[0], c[1], c[2]));
        }
        // The child goes into `RUNNING`, where `running_pid` reaps it (`try_wait`) or `play_stop` kills and waits.
        // **Its own process group**, so `play_stop` can end what the launcher started as well as the launcher.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        // **Its output goes to the project's game log** ([`log_path`]), which the Game workspace reads: a game started from a
        // window has no terminal, and what it printed before it died is what says why. **A log that cannot be opened refuses
        // Play**, with the reason, rather than starting a game whose output is lost.
        let log = std::sync::Arc::new(Mutex::new((open_log(&root).map_err(|why| Refusal::new(format!("{why}. Nothing was started.")))?, 0_u64)));
        cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        #[allow(clippy::zombie_processes)]
        let mut child = cmd.spawn().map_err(|e| Refusal::new(format!("could not start `{}`: {e}. Nothing is running.", argv[0])))?;
        if let Some(out) = child.stdout.take() {
            let log = std::sync::Arc::clone(&log);
            std::thread::spawn(move || pump(out, log));
        }
        if let Some(err) = child.stderr.take() {
            std::thread::spawn(move || pump(err, log));
        }
        let pid = child.id();
        with_exits(|e| e.remove(&root));
        with_table(|t| t.insert(root.clone(), child));
        let deadline = Instant::now() + Duration::from_secs_f64(p.wait_s.max(0.0));
        let mut ready = fairy_ready(config.port);
        while !ready && Instant::now() < deadline && running_pid(&root).is_some() {
            std::thread::sleep(Duration::from_millis(100));
            ready = fairy_ready(config.port);
        }
        Ok(Outcome {
            summary: format!("playing {map} (pid {pid}{})", if ready { ", ready" } else { ", not ready yet" }),
            affected: 0,
            saved: false,
            inverse: None,
            after: Some(json!({"pid": pid, "map": map, "ready": ready, "port": config.port})),
        })
    }
}

/// **Stop the game this editor started**, by pid, and return to the editor.
pub struct PlayStop;

impl Operation for PlayStop {
    fn name(&self) -> &'static str {
        "play_stop"
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::Nowhere
    }
    fn writes(&self) -> &'static [&'static str] {
        &[]
    }
    fn undo(&self) -> Undo {
        Undo::None
    }
    fn schema(&self) -> serde_json::Value {
        json!({"type": "object", "additionalProperties": false, "properties": {}})
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let _: serde_json::Map<String, serde_json::Value> = parse(params)?;
        let Some(root) = ctx.root_or_project() else {
            return Err(Refusal::new("no project is open, so no game was started."));
        };
        match running_pid(&root) {
            Some(pid) => Ok(json!({"pid": pid})),
            None => Err(Refusal::new("no game started by this editor is running, so there is nothing to stop.")),
        }
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let planned = self.plan(ctx, params)?;
        let root = ctx.root_or_project().expect("plan found a root");
        let mut child = with_table(|t| t.remove(&root)).expect("plan found the child");
        let pid = child.id();
        // The group first (its id is the launcher's pid, from `process_group(0)` in `play`), then the
        // launcher itself, which also covers a platform with no process groups.
        #[cfg(unix)]
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{pid}")])
            .stderr(std::process::Stdio::null())
            .status();
        let killed = child.kill();
        let _ = child.wait();
        remove_log(&root);
        with_exits(|e| e.insert(root.clone(), Exit { code: None, stopped: true }));
        killed
            .or_else(|e| if e.kind() == std::io::ErrorKind::InvalidInput { Ok(()) } else { Err(e) })
            .map_err(|e| Refusal::new(format!("could not stop pid {pid}: {e}")))?;
        let _ = planned;
        Ok(Outcome {
            summary: format!("stopped the game (pid {pid})"),
            affected: 0,
            saved: false,
            inverse: None,
            after: Some(json!({"pid": pid})),
        })
    }
}

/// **One button of the PLAY section** (#97).
#[derive(bevy::prelude::Component, Clone, Copy, Debug, PartialEq)]
pub enum PlayButton {
    Play,
    Stop,
}

/// The PLAY section of the controls panel.
pub fn spawn_section(s: &mut bevy::prelude::ChildSpawnerCommands) {
    use crate::chrome::{GAP_TIGHT, Severity, button, section};
    use bevy::prelude::*;
    section(s, "PLAY");
    s.spawn(Node { column_gap: Val::Px(GAP_TIGHT), ..default() }).with_children(|r| {
        button(r, PlayButton::Play, "Play", Severity::Plain);
        button(r, PlayButton::Stop, "Stop", Severity::Plain);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The game's log is bounded**: a pipe that carries more than [`LOG_CAP_BYTES`] leaves a file under it, cut with the line that
    /// says so, and the last bytes written are the pipe's last.
    #[test]
    fn a_log_past_its_cap_is_cut_back_and_keeps_the_newest_output() {
        let path = std::env::temp_dir().join(format!("grimoire-log-cap-{}.log", std::process::id()));
        let file = std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(&path).unwrap_or_else(|e| panic!("{e}"));
        let log = std::sync::Arc::new(Mutex::new((file, 0_u64)));
        let chunk = vec![b'x'; 1000];
        let mut input = Vec::new();
        for _ in 0..(LOG_CAP_BYTES / 1000 + 50) {
            input.extend_from_slice(&chunk);
        }
        input.extend_from_slice(b"last line\n");
        pump(std::io::Cursor::new(input), log);
        let text = std::fs::read(&path).unwrap_or_else(|e| panic!("{e}"));
        let _ = std::fs::remove_file(&path);
        assert!((text.len() as u64) <= LOG_CAP_BYTES, "the log holds {} bytes, over its cap of {LOG_CAP_BYTES}", text.len());
        assert!(text.starts_with(LOG_CUT), "a cut log says where it was cut");
        assert!(text.ends_with(b"last line\n"), "the newest output is kept");
    }
}
