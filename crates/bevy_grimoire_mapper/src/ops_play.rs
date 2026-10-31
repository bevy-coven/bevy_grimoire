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

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

/// The games this process started, by project root: one per project.
static RUNNING: Mutex<Option<HashMap<PathBuf, Child>>> = Mutex::new(None);

fn with_table<T>(f: impl FnOnce(&mut HashMap<PathBuf, Child>) -> T) -> T {
    let mut guard = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

/// The pid of the game this editor started for `root`, if it is still running.
pub fn running_pid(root: &std::path::Path) -> Option<u32> {
    with_table(|t| match t.get_mut(root) {
        Some(child) => match child.try_wait() {
            Ok(None) => Some(child.id()),
            _ => {
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
        Ok((root, config, map, p))
    }
}

impl Operation for Play {
    fn name(&self) -> &'static str {
        "play"
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
        #[allow(clippy::zombie_processes)]
        let child = cmd.spawn().map_err(|e| Refusal::new(format!("could not start `{}`: {e}. Nothing is running.", argv[0])))?;
        let pid = child.id();
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
