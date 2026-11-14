//! **Play the open map in the game, and stop it** (#97): `play` and `play_stop`.
//!
//! The game command comes from the project's `play.yaml`, never from the editor's code, and the
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

/// **The most bytes the game's log file holds.** A game that prints for an hour must not fill a disk: the editor cuts the file
/// back to a line saying so once it is past this ([`enforce_log_cap`]), and the run goes on.
pub const LOG_CAP_BYTES: u64 = 1024 * 1024;

/// The line written where the log was cut.
pub const LOG_CUT: &[u8] = b"[the game's log passed its size cap and was cut here]\n";

/// **Where the game's output goes**: one file per project in a directory of the user's own in the temporary directory, named by a
/// hash of the root, so the log of a project is never inside it (a project is a folder a person shares), two projects never share
/// one and another user cannot plant a link in the directory. `play` truncates it, so it holds the last run only; `play_stop`, closing
/// the project (when no game runs) and quitting the editor remove it ([`remove_log`]).
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

/// **Open the game's log at `path` for a new run**, or say why not. The directory is made private to the user (mode 700 where there
/// are modes). The file is created empty and **reopened for append**, so the game's own writes land at the end however the editor
/// cuts the file back.
pub fn open_log_at(path: &std::path::Path) -> Result<std::fs::File, String> {
    let cannot = |e: std::io::Error| format!("could not open the game's log file {}: {e}", path.display());
    if let Some(dir) = path.parent() {
        // **Only a directory made here is made private**: a log under an existing directory leaves that directory's mode alone.
        let existed = dir.exists();
        std::fs::create_dir_all(dir).map_err(cannot)?;
        #[cfg(unix)]
        if !existed {
            std::fs::set_permissions(dir, std::os::unix::fs::PermissionsExt::from_mode(0o700)).map_err(cannot)?;
        }
        #[cfg(not(unix))]
        let _ = existed;
    }
    std::fs::File::create(path).map_err(cannot)?;
    std::fs::OpenOptions::new().append(true).open(path).map_err(cannot)
}

/// **Remove the game's log** — when the game is stopped, when the project closes and when the editor quits. A file that is not
/// there is already removed. A game still running keeps writing to the removed file, which costs it nothing.
pub fn remove_log(root: &std::path::Path) {
    let path = log_path(root);
    if let Err(e) = std::fs::remove_file(&path)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        bevy::log::warn!("could not remove the game's log {}: {e}", path.display());
    }
}

/// **Cut the log at `path` back to [`LOG_CUT`]** when it is past [`LOG_CAP_BYTES`]. Answers whether it cut. The game holds the file
/// open for append, so what it writes next lands after the line.
pub fn enforce_log_cap_at(path: &std::path::Path) -> Result<bool, String> {
    use std::io::Write;
    let cannot = |e: std::io::Error| format!("{}: {e}", path.display());
    let length = std::fs::metadata(path).map_err(cannot)?.len();
    if length <= LOG_CAP_BYTES {
        return Ok(false);
    }
    let mut file = std::fs::OpenOptions::new().write(true).open(path).map_err(cannot)?;
    file.set_len(0).map_err(cannot)?;
    file.write_all(LOG_CUT).map_err(cannot)?;
    Ok(true)
}

/// **[`enforce_log_cap_at`] for a project's log.**
pub fn enforce_log_cap(root: &std::path::Path) -> Result<bool, String> {
    enforce_log_cap_at(&log_path(root))
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
        // **No person has to have pressed anything for a project's `play.yaml` to be here**: a cloned
        // project brings its own. Nothing starts until a person has allowed this command or this project.
        if let Some(request) = crate::trusted::needs_answer(ctx.trust_file.as_deref(), &root, &config.spec()).map_err(Refusal::new)? {
            return Err(Refusal::new(bevy_grimoire_core::trust::TrustStore::untrusted_sentence(&request.root, &request.spec)));
        }
        Ok((root, config, map, p))
    }
}

/// **Would `play` have to ask a person first?** The Play button asks before it dispatches, so the
/// question is raised instead of only a refusal. `Ok(None)` when the game is allowed, or when there is no
/// `play.yaml` to be allowed (the operation says so itself).
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
        // **Its output goes straight into the project's game log** ([`log_path`]), which the Game workspace reads: a game started
        // from a window has no terminal, and what it printed before it died is what says why. **A log that cannot be opened refuses
        // Play**, with the reason, rather than starting a game whose output is lost. **Not a pipe through the editor**: the game
        // writes to the file itself, so it never gets a broken pipe when the editor quits (the file is bounded by the editor
        // while it runs, [`enforce_log_cap`]).
        let file = open_log_at(&log_path(&root)).map_err(|why| Refusal::new(format!("{why}. Nothing was started.")))?;
        let again = file.try_clone().map_err(|e| Refusal::new(format!("could not open the game's log for its errors: {e}. Nothing was started.")))?;
        cmd.stdout(std::process::Stdio::from(file)).stderr(std::process::Stdio::from(again));
        #[allow(clippy::zombie_processes)]
        let child = cmd.spawn().map_err(|e| Refusal::new(format!("could not start `{}`: {e}. Nothing is running.", argv[0])))?;
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
    use crate::chrome::{GAP_TIGHT_STEPS, Severity, button, section};
    section(s, "PLAY");
    s.spawn(bevy_gooey::row(GAP_TIGHT_STEPS)).with_children(|r| {
        button(r, PlayButton::Play, "Play", Severity::Plain);
        button(r, PlayButton::Stop, "Stop", Severity::Plain);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The game's log is bounded**: a file past [`LOG_CAP_BYTES`] is cut back to the line that says so, and what the game appends
    /// next lands after it.
    #[test]
    fn a_log_past_its_cap_is_cut_back_and_the_game_keeps_appending() {
        let path = std::env::temp_dir().join(format!("grimoire-log-cap-{}.log", std::process::id()));
        let mut game = open_log_at(&path).unwrap_or_else(|e| panic!("{e}"));
        std::io::Write::write_all(&mut game, &vec![b'x'; usize::try_from(LOG_CAP_BYTES).unwrap_or(0) + 10]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(enforce_log_cap_at(&path), Ok(true), "a log past the cap is cut");
        std::io::Write::write_all(&mut game, b"last line\n").unwrap_or_else(|e| panic!("{e}"));
        let text = std::fs::read(&path).unwrap_or_else(|e| panic!("{e}"));
        let _ = std::fs::remove_file(&path);
        let again = enforce_log_cap_at(&path);
        assert!(text.starts_with(LOG_CUT), "a cut log says where it was cut");
        assert!(text.ends_with(b"last line\n"), "what the game writes after the cut is kept, after the line");
        assert_eq!(text.len(), LOG_CUT.len() + 10, "nothing of the old output is left");
        assert!(again.is_err(), "a log that is gone says so rather than pretending to be bounded");
    }

    /// **A log that cannot be opened is a refusal with its reason**: here the directory it would live in is a file.
    #[test]
    fn a_log_that_cannot_be_opened_says_why() {
        let blocker = std::env::temp_dir().join(format!("grimoire-log-blocker-{}", std::process::id()));
        std::fs::write(&blocker, b"in the way").unwrap_or_else(|e| panic!("{e}"));
        let opened = open_log_at(&blocker.join("game.log"));
        let _ = std::fs::remove_file(&blocker);
        let why = opened.err().unwrap_or_else(|| panic!("a log under a file opened"));
        assert!(why.contains("could not open the game's log file"), "{why}");
    }
}
