//! **The agent a terminal tab starts, and the power-on self test that proves it is connected.**
//!
//! The person picks a harness in Settings, Terminal (`terminal_agent`). The terminal then shows an
//! `Invoke Agent` button, because a session costs tokens to warm up and a person who is only looking
//! around pays none; its press opens an agent tab that types that harness into its login shell, already pointed at the editor's
//! MCP server (`bevy_cauldron_mcp`), and four checks are shown on a card over the terminal as they
//! pass:
//!
//! | check | passes when |
//! |---|---|
//! | Tools server | the server binary is beside the editor and its config was written ([`plan`]) |
//! | Agent started | the launch line was typed into the shell ([`AgentPost::started`]) |
//! | Agent connected | the server told the editor its client finished the MCP handshake (`grimoire/agent_hello`) |
//! | Agent self test | the agent ran `grimoire_self_test` and its three reads came back (`grimoire/self_test_result`) |
//!
//! **The tab is matched by a token.** [`plan`] mints one per launch and puts it in the server's
//! environment (`CAULDRON_AGENT_TOKEN`); both BRP methods carry it back, so two agent tabs never read
//! each other's results, and a server nobody started from a tab (token `null`) changes no card.
//!
//! **The line is typed, not exec'd.** The shell is the person's login shell, so its PATH and rc files
//! apply (`~/.local/bin/omp` is on a login PATH only), and when the harness exits the person is back
//! at a prompt in the same tab.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::chrome;
use crate::settings::TerminalAgent;
use crate::terminal::Terminal;

/// **What an agent started by the terminal is asked first.** It names the tool and asks for nothing
/// else, so the self test is one turn. It holds no `'`, so [`shell_quote`] leaves it as written.
pub const SELF_TEST_PROMPT: &str = "You are running in the terminal of Cauldron, a world editor. Run the Cauldron power-on self test: call the MCP tool named grimoire_self_test (your harness may prefix it with the server name cauldron) exactly once, look at the picture it returns, and reply in one short paragraph saying what you see on the screen and that you are ready. Do not call any other tool.";

/// The name the editor's MCP server has in every config written here.
pub const MCP_SERVER_NAME: &str = "cauldron";
/// The MCP server's binary, beside the editor's own.
pub const MCP_BINARY: &str = "bevy_cauldron_mcp";
/// How long a started agent has to finish its MCP handshake.
pub const AGENT_HELLO_TIMEOUT_S: f64 = 60.0;
/// How long a connected agent has to run the self test: a model turn, plus a person answering an
/// approval prompt if the harness asks one.
pub const SELF_TEST_TIMEOUT_S: f64 = 180.0;
/// How long the card stays after every check passed.
pub const CARD_LINGER_S: f64 = 4.0;
/// How long the card stays after a check failed, or after the status chip was clicked.
pub const FAILED_LINGER_S: f64 = 12.0;
/// A shell that has printed nothing for this long is at its prompt, so the launch line is typed.
pub const LAUNCH_QUIET_MS: u64 = 300;
/// The launch line is typed this long after the tab opened, quiet or not.
pub const LAUNCH_CAP_MS: u64 = 5_000;

/// The config every harness can read, `$CAULDRON_MCP_CONFIG`, under [`AgentSetup::files`].
const MCP_CONFIG_FILE: &str = "mcp.json";
/// omp's extension folder under [`AgentSetup::files`]: omp discovers `.mcp.json` beside an extension
/// it was given with `-e <dir>`, and omp has no flag naming an MCP config.
const OMP_EXTENSION_DIR: &str = "omp";
const OMP_MCP_FILE: &str = ".mcp.json";
/// The extension's entry, which omp resolves a folder to.
const OMP_EXTENSION_ENTRY: &str = "index.ts";
/// **The extension written beside the MCP config on every launch.** It tells the editor which omp session its tab is in
/// (`grimoire/agent_session`, with the tab's token), when the session starts and when the person switches to another
/// inside omp, and that the agent left its session when omp shuts down. That is how the editor can resume the session the
/// next time it opens the project ([`crate::agent_resume`]); omp has no flag that prints its session id, and the id is the
/// extension's to read (`ctx.sessionManager.getSessionId()`). A report the editor cannot take is said in omp's own UI.
pub const OMP_EXTENSION_SOURCE: &str = r#"// Written by Cauldron on every agent launch. An omp extension whose folder carries the editor's MCP server (.mcp.json), and which tells the editor which omp session its tab is in, so the editor can resume that session the next time it opens this project.
const port = process.env.BEVY_BRP_PORT;
const token = process.env.CAULDRON_AGENT_TOKEN;

async function tell(session: string | null): Promise<string | null> {
	if (!port || !token) return "BEVY_BRP_PORT or CAULDRON_AGENT_TOKEN is not set";
	try {
		const reply = await fetch(`http://127.0.0.1:${port}/`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "grimoire/agent_session", params: { token, session } }),
			signal: AbortSignal.timeout(1000),
		});
		const body: any = await reply.json();
		return body.error ? String(body.error.message ?? body.error) : null;
	} catch (e) {
		return String(e);
	}
}

export default function (pi: any) {
	const report = async (_event: unknown, ctx: any) => {
		const failed = await tell(ctx.sessionManager.getSessionId());
		if (failed) ctx.ui?.notify?.(`Cauldron was not told this session's id, so it cannot resume it: ${failed}`, "warning");
	};
	pi.on("session_start", report);
	pi.on("session_switch", report);
	pi.on("session_shutdown", async () => {
		await tell(null);
	});
}
"#;

/// **The four checks, in the order the card lists them.**
pub const CHECK_LABELS: [&str; 4] = ["Tools server", "Agent started", "Agent connected", "Agent self test"];
const SERVER: usize = 0;
const STARTED: usize = 1;
const CONNECTED: usize = 2;
const SELF_TEST: usize = 3;

/// **What an agent tab needs from the running editor.** Inserted by `main`, never by a test harness,
/// as `settings::UserSettingsFile` is: a test inserts its own.
#[derive(Resource, Clone, Debug)]
pub struct AgentSetup {
    /// The port the editor's remote protocol listens on; `None` in a build without the `debugger`
    /// feature, where no agent can reach the editor.
    pub port: Option<u16>,
    /// The MCP server binary.
    pub server: PathBuf,
    /// Where the configs a launch writes go: `<data_dir>/agents`.
    pub files: PathBuf,
}

/// The folder under the per-user data folder an agent launch writes its configs to.
const AGENTS_DIR: &str = "agents";

/// **The running binary's [`AgentSetup`]**: the remote protocol's port, the MCP server beside this
/// executable, and `<data_dir>/agents`. Called by `main` only, as `update::insert_for_the_running_app`
/// is, so no test hands an agent the real data folder. Nothing is inserted when the executable or the
/// data folder cannot be found, and an agent tab then says so on its card.
pub fn insert_for_the_running_binary(app: &mut App) {
    let Some(exe_dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(Path::to_path_buf)) else {
        return;
    };
    let Some(data) = bevy_grimoire_core::user_dirs::data_dir() else {
        return;
    };
    #[cfg(feature = "debugger")]
    let port = Some(crate::harness::brp_port());
    #[cfg(not(feature = "debugger"))]
    let port = None;
    app.insert_resource(AgentSetup { port, server: exe_dir.join(MCP_BINARY), files: data.join(AGENTS_DIR) });
}

/// **The agent tab's ink, from the running skin** (UI-02, #210, slice 6a).
///
/// The self-test card and the bar's status chip drew four `chrome.rs` colours — the heading, a check
/// that passed, one that failed, one still waiting — read straight from the constants. That made the
/// tab the last panel a project's `family/bevy_gooey.yaml` could not reach: a high-contrast theme or a
/// parchment one left the card in the editor's darks and its greens.
///
/// The shape is [`crate::editor::StatusInk`]'s and [`crate::tiles::ListInk`]'s: one expression the
/// spawners and the tests both read, so an assertion here is an assertion about what the card paints.
///
/// **The fallback is the shipped palette**, never a second one: a headless harness that never ran
/// `EditorUiPlugin` has no `GooeySkin`, and it must get exactly the colours the editor has always had.
/// `Ink::muted` and `Ink::labeled` are the skin's own named tones (`bevy_gooey`'s `GooeySkin::tones`), because
/// no field of the skin means "excluded" or "judged"; a skin that carries neither falls back to the
/// constant, so the editor's own look is unchanged by construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgentInk {
    /// The card's heading — the harness's name and what the card is.
    pub read: Color,
    /// A check still waiting, or skipped on purpose.
    pub muted: Color,
    /// A check that passed.
    pub good: Color,
    /// A check that failed, and the chip that says the test failed.
    pub danger: Color,
}

impl AgentInk {
    /// From the running skin when there is one, the shipped palette when there is not.
    pub fn of(skin: Option<&bevy_gooey::GooeySkin>) -> AgentInk {
        let Some(skin) = skin else {
            return AgentInk { read: chrome::ink().text, muted: chrome::ink().muted, good: chrome::ink().labeled, danger: chrome::ink().danger };
        };
        AgentInk {
            read: skin.text,
            muted: skin.tone(MUTED_TONE).unwrap_or(chrome::ink().muted),
            good: skin.tone(LABELED_TONE).unwrap_or(chrome::ink().labeled),
            danger: skin.danger,
        }
    }
}

/// The skin's tone for "excluded on purpose", the role behind `chrome::ink().muted`.
const MUTED_TONE: &str = "muted";
/// The skin's tone for "judged and ready", the role behind `chrome::ink().labeled`.
const LABELED_TONE: &str = "labeled";

/// One check's state.
#[derive(Clone, Debug, PartialEq)]
pub enum CheckStatus {
    Pending,
    Passed(String),
    Failed(String),
    Skipped(String),
}

impl CheckStatus {
    /// **The state a check's row lights its LED in** (#421): a check that passed is live, one that failed is failed, one that was
    /// skipped is off, and one still waiting is busy. The row carries no word for it: the LED keeps its accessible state name.
    pub fn led(&self) -> bevy_gooey::LedState {
        match self {
            CheckStatus::Pending => bevy_gooey::LedState::Busy,
            CheckStatus::Passed(_) => bevy_gooey::LedState::Live,
            CheckStatus::Failed(_) => bevy_gooey::LedState::Failed,
            CheckStatus::Skipped(_) => bevy_gooey::LedState::Off,
        }
    }

    /// The row's ink, from the running skin's [`AgentInk`]. The no-skin fallback is the shipped palette,
    /// so every caller that has no skin reads what it always read.
    pub fn ink(&self, ink: &AgentInk) -> Color {
        match self {
            CheckStatus::Passed(_) => ink.good,
            CheckStatus::Failed(_) => ink.danger,
            CheckStatus::Pending | CheckStatus::Skipped(_) => ink.muted,
        }
    }

    /// What the check found, or `waiting`.
    pub fn detail(&self) -> &str {
        match self {
            CheckStatus::Pending => "waiting",
            CheckStatus::Passed(d) | CheckStatus::Failed(d) | CheckStatus::Skipped(d) => d,
        }
    }
}

/// **One agent tab's self test**: its harness, its token, and the four checks. Every change bumps
/// [`AgentPost::revision`], which is what the pane redraws on.
#[derive(Clone, Debug)]
pub struct AgentPost {
    pub harness: TerminalAgent,
    pub token: String,
    pub checks: [CheckStatus; 4],
    /// The word the shell is typed: `omp`, or a custom command's program.
    pub first_word: String,
    pub launched_s: f64,
    pub hello_s: Option<f64>,
    pub settled_s: Option<f64>,
    /// The card is shown until then, whatever the checks say: a click on the status chip.
    pub shown_until_s: Option<f64>,
    /// How many tools the server offered its client.
    pub tools: Option<usize>,
    pub revision: u32,
    /// **The project the tab was started for**, which keys its resume record ([`crate::agent_resume`]); `None` when no
    /// project was open.
    pub project: Option<PathBuf>,
    /// **The omp session the agent says it is in now** (`grimoire/agent_session`): `None` before it says, and again once it
    /// has left its session.
    pub session: Option<String>,
    /// The session this launch was told to resume, when it was a resume.
    pub resuming: Option<String>,
    /// `session` changed since the resume record was last settled ([`keep_the_resume_record`]).
    session_changed: bool,
    /// A resumed agent never connected, so the session the record named cannot be resumed: the record is struck out.
    resume_failed: bool,
}

impl AgentPost {
    fn new(harness: TerminalAgent, token: String, first_word: String, launched_s: f64) -> AgentPost {
        AgentPost {
            harness,
            token,
            checks: [CheckStatus::Pending, CheckStatus::Pending, CheckStatus::Pending, CheckStatus::Pending],
            first_word,
            launched_s,
            hello_s: None,
            settled_s: None,
            shown_until_s: None,
            tools: None,
            revision: 0,
            project: None,
            session: None,
            resuming: None,
            session_changed: false,
            resume_failed: false,
        }
    }

    /// Every check from `from` on is skipped, for `why`.
    fn skip_from(&mut self, from: usize, why: &str) {
        for check in &mut self.checks[from..] {
            *check = CheckStatus::Skipped(why.to_owned());
        }
        self.revision += 1;
    }

    /// **The launch line reached the shell.**
    pub fn started(&mut self) {
        self.checks[STARTED] = CheckStatus::Passed(format!("typed {} into the shell", self.first_word));
        self.revision += 1;
    }

    /// **The launch line never reached the shell**, for `why`: the shell did not start, or went away
    /// before it was typed.
    pub fn not_started(&mut self, why: String) {
        self.checks[STARTED] = CheckStatus::Failed(why);
        self.skip_from(CONNECTED, "nothing was started");
    }

    /// **The server said its client finished the handshake.** A new hello after a check failed or was
    /// skipped (the person started the agent again in the same tab) gives the self test a new window.
    pub fn hello(&mut self, client: Option<(String, String)>, tools: usize, now_s: f64) {
        self.checks[CONNECTED] = CheckStatus::Passed(match client {
            Some((name, version)) => format!("{name} {version} · {tools} tools"),
            None => format!("an MCP client · {tools} tools"),
        });
        if self.resuming.is_none() && !matches!(self.checks[SELF_TEST], CheckStatus::Passed(_)) {
            self.checks[SELF_TEST] = CheckStatus::Pending;
            self.settled_s = None;
        }
        self.hello_s = Some(now_s);
        self.tools = Some(tools);
        self.revision += 1;
    }

    /// **The self test's results**, as `(name, ok, detail)`: every detail when all passed, the first
    /// failure otherwise.
    pub fn self_test(&mut self, checks: &[(String, bool, String)]) {
        self.checks[SELF_TEST] = match checks.iter().find(|(_, ok, _)| !ok) {
            Some((name, _, detail)) => CheckStatus::Failed(format!("{name}: {detail}")),
            None => CheckStatus::Passed(checks.iter().map(|(_, _, d)| d.as_str()).collect::<Vec<_>>().join(" · ")),
        };
        self.revision += 1;
    }

    /// **The agent says which session it is in** (`grimoire/agent_session`), or `None` once it has left its session
    /// (omp shut down). The resume record follows it ([`keep_the_resume_record`]).
    pub fn session_reported(&mut self, session: Option<String>) {
        if self.session != session {
            self.session = session;
            self.session_changed = true;
        }
    }

    /// **The timeouts, and the moment the checks settled.**
    pub fn tick(&mut self, now_s: f64) {
        if self.checks[CONNECTED] == CheckStatus::Pending
            && matches!(self.checks[STARTED], CheckStatus::Passed(_))
            && now_s - self.launched_s > AGENT_HELLO_TIMEOUT_S
        {
            self.checks[CONNECTED] = CheckStatus::Failed(format!(
                "{} did not reach the editor in {AGENT_HELLO_TIMEOUT_S} s. If the terminal says command not found, install it or choose another agent in Settings, Terminal.",
                self.harness.name()
            ));
            self.checks[SELF_TEST] = CheckStatus::Skipped("needs a connected agent".to_owned());
            self.resume_failed = self.resuming.is_some();
            self.revision += 1;
        }
        if self.checks[SELF_TEST] == CheckStatus::Pending
            && let Some(hello) = self.hello_s
            && now_s - hello > SELF_TEST_TIMEOUT_S
        {
            self.checks[SELF_TEST] = match self.harness {
                TerminalAgent::OhMyPi => CheckStatus::Failed(format!(
                    "the agent did not run grimoire_self_test in {SELF_TEST_TIMEOUT_S} s; answer it in the terminal if it asked something, then click the agent chip to try again"
                )),
                TerminalAgent::Custom | TerminalAgent::Off => CheckStatus::Skipped(
                    "a custom command gets the self test only when it passes \"$CAULDRON_SELF_TEST_PROMPT\" to its agent".to_owned(),
                ),
            };
            self.revision += 1;
        }
        if self.settled_s.is_none() && !self.pending() {
            self.settled_s = Some(now_s);
            self.revision += 1;
        }
    }

    fn pending(&self) -> bool {
        self.checks.contains(&CheckStatus::Pending)
    }

    fn failed(&self) -> bool {
        self.checks.iter().any(|c| matches!(c, CheckStatus::Failed(_)))
    }

    /// **Is the card on screen?** While anything is pending; then for a short linger, longer when
    /// something failed; and while a click on the status chip asked for it.
    pub fn card_shown(&self, now_s: f64) -> bool {
        if self.shown_until_s.is_some_and(|until| until > now_s) || self.pending() {
            return true;
        }
        let linger = if self.failed() { FAILED_LINGER_S } else { CARD_LINGER_S };
        self.settled_s.is_none_or(|settled| now_s < settled + linger)
    }

    /// **The bar's status chip**: its words and its ink.
    pub fn chip(&self) -> (String, Color) {
        (self.chip_words(), self.chip_ink(&AgentInk::of(None)))
    }

    /// **The chip's ink from the running skin** (UI-02, #210, slice 6a): the same state the words say,
    /// drawn in the skin's own role. [`AgentPost::chip`] is this with the shipped palette, so the
    /// editor's own look is unchanged by construction and a project's skin reaches the bar.
    pub fn chip_ink(&self, ink: &AgentInk) -> Color {
        if self.failed() {
            ink.danger
        } else if self.pending() {
            ink.muted
        } else {
            ink.good
        }
    }

    /// The chip's words, one sentence per state. **The one writer**: the terminal pane's
    /// `fill_the_bar` puts these on the chip and nothing rewrites them, so what a person reads and
    /// what [`AgentPost::chip`] answers are one string rather than two that agree.
    pub fn chip_words(&self) -> String {
        let name = self.harness.name();
        if self.failed() {
            format!("{name}: self test failed")
        } else if self.pending() {
            format!("{name}: checking…")
        } else {
            format!("{name} connected · {} tools", self.tools.unwrap_or(0))
        }
    }

    /// **The first check that failed, in a sentence** (`Agent connected: omp did not reach the editor…`), or `None` when none has.
    /// A failure's reason belongs on the card and in the LED's tooltip, never in a chip of the bar (#423).
    pub fn failure(&self) -> Option<String> {
        self.checks.iter().zip(CHECK_LABELS).find_map(|(check, label)| match check {
            CheckStatus::Failed(why) => Some(format!("{label}: {why}")),
            _ => None,
        })
    }

    /// **What the bar's LED says when it is hovered**: the state in a sentence, the reason when a check failed, and what a click does.
    pub fn led_tooltip(&self) -> String {
        let state = match self.failure() {
            Some(why) => format!("{} ({why})", self.chip_words()),
            None => self.chip_words(),
        };
        format!("{state}. Click to show the self test again")
    }

    /// **The agent's LED** (feedback-1794230412409: *"where are the LEDs?"*): what the status chip says in words, in light. A
    /// failed check is [`bevy_gooey::LedState::Failed`], a check still waiting is [`bevy_gooey::LedState::Busy`], and every check
    /// passed or skipped is [`bevy_gooey::LedState::Live`]; the order is the chip's, so the two never disagree. `bevy_gooey` draws
    /// each state with its own silhouette and its word beside the dot, so the light is never the only signal.
    pub fn led(&self) -> bevy_gooey::LedState {
        if self.failed() {
            bevy_gooey::LedState::Failed
        } else if self.pending() {
            bevy_gooey::LedState::Busy
        } else {
            bevy_gooey::LedState::Live
        }
    }

    /// **A click on the status chip**: the card comes back for a while, and an omp self test that
    /// failed after its agent connected is asked again. `true` when the caller is to type
    /// [`SELF_TEST_PROMPT`] into the tab, which is at omp's prompt.
    pub fn rerun(&mut self, now_s: f64) -> bool {
        self.shown_until_s = Some(now_s + FAILED_LINGER_S);
        self.revision += 1;
        if self.harness == TerminalAgent::OhMyPi
            && matches!(self.checks[CONNECTED], CheckStatus::Passed(_))
            && matches!(self.checks[SELF_TEST], CheckStatus::Failed(_))
        {
            self.checks[SELF_TEST] = CheckStatus::Pending;
            self.hello_s = Some(now_s);
            self.settled_s = None;
            return true;
        }
        false
    }
}

/// **`s` as one POSIX shell word**: in single quotes, which a shell takes literally, with each `'`
/// written the POSIX way (`'\''`).
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// **What a new agent tab types, the environment its shell gets, and its self-test record.**
pub struct AgentLaunch {
    /// The launch line, with its `\r`; empty when nothing can be started (the post says why).
    pub line: Vec<u8>,
    pub env: Vec<(String, String)>,
    pub post: AgentPost,
}

/// **The MCP config a person's agent is handed**, with `args` extra on the server line.
///
/// **One builder, because there is one way an agent is connected.** The terminal tab writes this and an
/// agent the benchmark launches reads it (`grimoire/agent_setup`, BM-02), so a change to how an agent
/// finds the editor cannot leave one of them measuring or using a stale path. `token` is the tab's
/// (`CAULDRON_AGENT_TOKEN`), which is how the editor tells two agents' work apart.
pub fn mcp_config(
    server: &Path,
    port: u16,
    token: &str,
    project: Option<&Path>,
    args: &[String],
) -> Value {
    let mut base: Vec<String> = project
        .map_or_else(Vec::new, |p| vec!["--project".to_owned(), p.to_string_lossy().into_owned()]);
    base.extend(args.iter().cloned());
    json!({
        "mcpServers": {
            MCP_SERVER_NAME: {
                "type": "stdio",
                "command": server.to_string_lossy(),
                "args": base,
                "env": {"BEVY_BRP_PORT": port.to_string(), "CAULDRON_AGENT_TOKEN": token},
            },
        },
    })
}

/// **Plan an agent tab**: write the MCP configs, and decide the line and the environment.
///
/// Check 0 fails, and nothing is typed, when the editor has no agent setup, no remote protocol, no
/// server binary beside it, or a config could not be written. omp is given its config as an extension
/// folder (`omp -e <dir>`); a custom command reads `$CAULDRON_MCP_CONFIG` or `$CAULDRON_MCP` itself.
pub fn plan(
    harness: TerminalAgent,
    command: Option<&str>,
    setup: Option<&AgentSetup>,
    project: Option<&Path>,
    now_s: f64,
) -> AgentLaunch {
    plan_resuming(harness, command, setup, project, None, now_s)
}

/// **[`plan`], for an omp tab that picks a remembered session back up** (`omp --resume <id>`): no self-test prompt is
/// typed, because the session already has its history and a person has not asked for a turn, and the self test is skipped
/// with that said. `resume` names the session; anything but an omp harness, or an id that is not a session id, starts
/// nothing and says so on the card.
pub fn plan_resuming(
    harness: TerminalAgent,
    command: Option<&str>,
    setup: Option<&AgentSetup>,
    project: Option<&Path>,
    resume: Option<&str>,
    now_s: f64,
) -> AgentLaunch {
    let token = format!(
        "{:x}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())
    );
    let command = command.map(str::trim).filter(|c| !c.is_empty());
    let first_word = match harness {
        TerminalAgent::Custom => command.and_then(|c| c.split_whitespace().next()).unwrap_or(harness.name()),
        TerminalAgent::OhMyPi | TerminalAgent::Off => harness.name(),
    };
    let mut post = AgentPost::new(harness, token.clone(), first_word.to_owned(), now_s);
    post.project = project.map(Path::to_path_buf);
    post.resuming = resume.map(str::to_owned);
    let nothing = |mut post: AgentPost, why: String| {
        post.checks[SERVER] = CheckStatus::Failed(why);
        post.skip_from(STARTED, "nothing was started");
        AgentLaunch { line: Vec::new(), env: Vec::new(), post }
    };
    if harness == TerminalAgent::Off {
        return nothing(post, "no terminal agent is chosen: pick one in Settings, Terminal".to_owned());
    }
    if let Some(id) = resume {
        if harness != TerminalAgent::OhMyPi {
            return nothing(post, "only an omp session can be resumed".to_owned());
        }
        if !crate::agent_resume::is_session_id(id) {
            return nothing(post, format!("{id:?} is not a session id, so nothing was resumed"));
        }
    }
    let Some(setup) = setup else {
        return nothing(post, "this editor was started without an agent setup, so it cannot hand an agent its tools".to_owned());
    };
    let Some(port) = setup.port else {
        return nothing(post, "this build has no remote protocol (the debugger feature), so no agent can reach the editor".to_owned());
    };
    if !setup.server.is_file() {
        return nothing(
            post,
            format!("the tools server is not at {}: build it with cargo build -p {MCP_BINARY}", setup.server.display()),
        );
    }
    let config = mcp_config(&setup.server, port, &token, project, &[]);
    let config_text = match serde_json::to_string_pretty(&config) {
        Ok(text) => text + "\n",
        Err(e) => return nothing(post, format!("the MCP config did not serialise: {e}")),
    };
    let config_file = setup.files.join(MCP_CONFIG_FILE);
    let omp_dir = setup.files.join(OMP_EXTENSION_DIR);
    let mut writes = vec![(config_file.clone(), config_text.clone())];
    if harness == TerminalAgent::OhMyPi {
        writes.push((omp_dir.join(OMP_MCP_FILE), config_text));
        writes.push((omp_dir.join(OMP_EXTENSION_ENTRY), OMP_EXTENSION_SOURCE.to_owned()));
    }
    // **Written on every launch**: a harness reads its config when it starts, so the newest token is
    // the one the starting agent needs. Atomically, so a harness starting in another tab never reads
    // half a config.
    for (path, text) in &writes {
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| bevy_grimoire_core::atomic_file::write(path, text));
        if let Err(e) = written {
            return nothing(post, format!("{}: {e}", path.display()));
        }
    }
    post.checks[SERVER] = CheckStatus::Passed(format!("{MCP_BINARY} on port {port}"));
    let line = match harness {
        TerminalAgent::OhMyPi => match resume {
            Some(session) => {
                post.checks[SELF_TEST] = CheckStatus::Skipped("a resumed session is not asked its self test again".to_owned());
                format!("omp -e {} --resume {}\r", shell_quote(&omp_dir.to_string_lossy()), shell_quote(session))
            }
            None => format!("omp -e {} {}\r", shell_quote(&omp_dir.to_string_lossy()), shell_quote(SELF_TEST_PROMPT)),
        },
        TerminalAgent::Custom | TerminalAgent::Off => match command {
            Some(command) => format!("{command}\r"),
            None => {
                post.checks[STARTED] =
                    CheckStatus::Failed("Custom command is not set: add terminal_agent_command to settings.yaml".to_owned());
                post.skip_from(CONNECTED, "nothing was started");
                String::new()
            }
        },
    };
    let env = vec![
        ("BEVY_BRP_PORT".to_owned(), port.to_string()),
        ("CAULDRON_MCP".to_owned(), setup.server.to_string_lossy().into_owned()),
        ("CAULDRON_MCP_CONFIG".to_owned(), config_file.to_string_lossy().into_owned()),
        ("CAULDRON_AGENT_TOKEN".to_owned(), token),
        ("CAULDRON_SELF_TEST_PROMPT".to_owned(), SELF_TEST_PROMPT.to_owned()),
    ];
    AgentLaunch { line: line.into_bytes(), env, post }
}

/// `grimoire/agent_hello`'s body.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Hello {
    token: Option<String>,
    client: Option<Client>,
    tools: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Client {
    name: String,
    version: String,
}

/// `grimoire/self_test_result`'s body.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelfTestResult {
    token: Option<String>,
    checks: Vec<SelfTestCheck>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelfTestCheck {
    name: String,
    ok: bool,
    detail: String,
}

/// **An MCP server Cauldron's terminal started says its agent connected.** `{"known": true}` when
/// the token is a tab's, `false` for any other token or none.
pub fn agent_hello(terminal: &mut Terminal, params: &Value, now_s: f64) -> Result<Value, String> {
    let hello: Hello = serde_json::from_value(params.clone())
        .map_err(|e| format!("agent_hello takes token, client {{name, version}} and tools: {e}"))?;
    let post = hello.token.as_deref().and_then(|token| terminal.agent_with_token_mut(token));
    let Some(post) = post else {
        return Ok(json!({"known": false}));
    };
    post.hello(hello.client.map(|c| (c.name, c.version)), hello.tools, now_s);
    Ok(json!({"known": true}))
}

/// **`grimoire_self_test`'s results, for the tab whose token they carry.**
pub fn self_test_result(terminal: &mut Terminal, params: &Value) -> Result<Value, String> {
    const SHAPE: &str = "self_test_result takes token and checks: [{name, ok, detail}]";
    let result: SelfTestResult = serde_json::from_value(params.clone()).map_err(|e| format!("{SHAPE}: {e}"))?;
    if result.checks.is_empty() {
        return Err(format!("{SHAPE}: checks is empty, so there is nothing to show"));
    }
    let post = result.token.as_deref().and_then(|token| terminal.agent_with_token_mut(token));
    let Some(post) = post else {
        return Ok(json!({"known": false}));
    };
    let checks: Vec<(String, bool, String)> = result.checks.into_iter().map(|c| (c.name, c.ok, c.detail)).collect();
    post.self_test(&checks);
    Ok(json!({"known": true}))
}

/// `grimoire/agent_session`'s body.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionReport {
    token: Option<String>,
    session: Option<String>,
}

/// **An omp extension a terminal tab started says which session it is in**, or that it left its session (`session: null`),
/// for the tab whose token it carries. `{"known": true}` when the token is a tab's. The id is typed into a shell on the
/// next launch, so one that is not a session id is refused here.
pub fn agent_session(terminal: &mut Terminal, params: &Value) -> Result<Value, String> {
    const SHAPE: &str = "agent_session takes token and session (a session id, or null once the agent left its session)";
    let report: SessionReport = serde_json::from_value(params.clone()).map_err(|e| format!("{SHAPE}: {e}"))?;
    if let Some(id) = &report.session
        && !crate::agent_resume::is_session_id(id)
    {
        return Err(format!("{SHAPE}: {id:?} is not a session id"));
    }
    let post = report.token.as_deref().and_then(|token| terminal.agent_with_token_mut(token));
    let Some(post) = post else {
        return Ok(json!({"known": false}));
    };
    post.session_reported(report.session);
    Ok(json!({"known": true}))
}

/// **Open an agent tab** of the chosen harness, `cols` by `rows`, where the next shell starts, its
/// server told the open project: what the `Invoke Agent` button and the `Agent` chip both do.
pub fn open_agent_tab(
    terminal: &mut Terminal,
    cols: u16,
    rows: u16,
    setup: Option<&AgentSetup>,
    root: Option<&crate::project_assets::ProjectRoot>,
    now_s: f64,
) -> usize {
    open_agent_tab_resuming(terminal, cols, rows, setup, root, None, now_s)
}

/// **[`open_agent_tab`] for an omp session that was running when the editor closed**, which picks that session back up
/// (`omp --resume <session>`): what the pane does by itself at a project that has a resume record.
pub fn open_agent_tab_resuming(
    terminal: &mut Terminal,
    cols: u16,
    rows: u16,
    setup: Option<&AgentSetup>,
    root: Option<&crate::project_assets::ProjectRoot>,
    resume: Option<&str>,
    now_s: f64,
) -> usize {
    let project = root.and_then(crate::project_assets::ProjectRoot::current);
    let launch = plan_resuming(terminal.agent(), terminal.agent_command(), setup, project.as_deref(), resume, now_s);
    let cwd = terminal.next_cwd();
    terminal.spawn_agent(cols, rows, &cwd, launch)
}

/// The agent tabs' timeouts, each frame.
pub struct TerminalAgentPlugin;

impl Plugin for TerminalAgentPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (tick_the_agents, keep_the_resume_record));
    }
}

/// **Run every post's timeouts**, marking the terminal changed only when one moved: the pane redraws
/// on a revision, and a `Terminal` changed every frame would tell every reader it changed.
fn tick_the_agents(mut terminal: ResMut<Terminal>, time: Res<Time<Real>>) {
    let now = time.elapsed_secs_f64();
    let mut moved = false;
    for post in terminal.bypass_change_detection().agent_posts_mut() {
        let before = post.revision;
        post.tick(now);
        moved |= post.revision != before;
    }
    if moved {
        terminal.set_changed();
    }
}

/// **The record of what is running in each project follows what the agents say** ([`crate::agent_resume`]).
///
/// A project's record is the newest session an open agent tab of it holds, or nothing: written when a tab reports a
/// session, struck out when its agent leaves the session, when its tab is closed ([`Terminal::take_ended_agents`]: the
/// person closed it, or typed `exit` at its shell), and when a resumed agent never connected. **Nothing here runs when
/// the editor quits**, so a record that is still there at the next launch is an agent that was running when it closed.
/// A record that cannot be written or read is said in the pane's refusal, not dropped.
fn keep_the_resume_record(mut terminal: ResMut<Terminal>, setup: Option<Res<AgentSetup>>) {
    let Some(setup) = setup else {
        return;
    };
    let quiet = terminal.bypass_change_detection();
    let mut projects = quiet.take_ended_agents();
    for post in quiet.agent_posts_mut() {
        let failed = std::mem::take(&mut post.resume_failed);
        if failed {
            post.session = None;
        }
        if (std::mem::take(&mut post.session_changed) || failed)
            && let Some(project) = &post.project
        {
            projects.push(project.clone());
        }
    }
    projects.sort();
    projects.dedup();
    let mut refused = Vec::new();
    for project in projects {
        let session = quiet
            .agent_posts_mut()
            .filter(|post| post.project.as_deref() == Some(project.as_path()))
            .filter_map(|post| post.session.clone())
            .last();
        let settled = match &session {
            Some(session) => crate::agent_resume::remember(&setup.files, &project, TerminalAgent::OhMyPi.name(), session),
            None => crate::agent_resume::forget(&setup.files, &project),
        };
        if let Err(why) = settled {
            refused.push(format!("the agent's resume record for {} could not be kept: {why}", project.display()));
        }
    }
    for why in refused {
        terminal.refuse(why);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A setup in a fresh temporary folder, with a file standing in for the server binary.
    fn setup(tag: &str) -> AgentSetup {
        let root = std::env::temp_dir().join(format!("terminal_agent_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
        let server = root.join(MCP_BINARY);
        std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
        AgentSetup { port: Some(15799), server, files: root.join("agents") }
    }

    #[test]
    fn an_omp_plan_types_omp_with_its_extension_and_writes_the_servers_config() {
        let setup = setup("omp");
        let launch = plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(Path::new("/tmp/p")), 0.0);
        let omp = setup.files.join("omp");
        assert_eq!(
            String::from_utf8_lossy(&launch.line),
            format!("omp -e '{}' '{SELF_TEST_PROMPT}'\r", omp.display())
        );
        let text = std::fs::read_to_string(omp.join(".mcp.json")).unwrap_or_else(|e| panic!("{e}"));
        let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
        let server = &config["mcpServers"]["cauldron"];
        assert_eq!(server["args"], json!(["--project", "/tmp/p"]));
        assert_eq!(server["env"]["BEVY_BRP_PORT"], "15799");
        assert_eq!(server["env"]["CAULDRON_AGENT_TOKEN"], launch.post.token.as_str());
        assert!(omp.join("index.ts").is_file(), "omp resolves an extension folder to its index.ts");
        assert_eq!(launch.post.checks[0], CheckStatus::Passed("bevy_cauldron_mcp on port 15799".to_owned()));
        assert!(launch.env.contains(&("CAULDRON_AGENT_TOKEN".to_owned(), launch.post.token.clone())));
    }

    #[test]
    fn a_missing_server_starts_nothing_and_says_where_it_looked() {
        let mut setup = setup("missing");
        setup.server = setup.files.join("not_here");
        let launch = plan(TerminalAgent::OhMyPi, None, Some(&setup), None, 0.0);
        assert!(
            matches!(&launch.post.checks[0], CheckStatus::Failed(why) if why.contains("is not at")),
            "{:?}",
            launch.post.checks[0]
        );
        for check in &launch.post.checks[1..] {
            assert!(matches!(check, CheckStatus::Skipped(_)), "{check:?}");
        }
        assert!(launch.line.is_empty(), "nothing is typed when there is no server");
    }

    #[test]
    fn a_custom_agent_with_no_command_says_which_key_to_set() {
        let setup = setup("custom");
        let launch = plan(TerminalAgent::Custom, None, Some(&setup), None, 0.0);
        assert!(
            matches!(&launch.post.checks[1], CheckStatus::Failed(why) if why.contains("terminal_agent_command")),
            "{:?}",
            launch.post.checks[1]
        );
        assert!(launch.line.is_empty());
    }

    #[test]
    fn a_quoted_word_survives_its_own_apostrophe_and_the_prompt_needs_none() {
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert!(!SELF_TEST_PROMPT.contains('\''), "the prompt is typed in single quotes as written");
    }

    fn started_post(harness: TerminalAgent) -> AgentPost {
        let mut post = AgentPost::new(harness, "t".to_owned(), harness.name().to_owned(), 0.0);
        post.checks[0] = CheckStatus::Passed("server".to_owned());
        post.started();
        post
    }

    #[test]
    fn an_agent_that_never_connects_fails_after_a_minute_and_skips_its_test() {
        let mut post = started_post(TerminalAgent::OhMyPi);
        post.tick(AGENT_HELLO_TIMEOUT_S - 1.0);
        assert_eq!(post.checks[2], CheckStatus::Pending, "inside the minute it is still waited for");
        post.tick(AGENT_HELLO_TIMEOUT_S + 1.0);
        assert!(matches!(post.checks[2], CheckStatus::Failed(_)), "{:?}", post.checks[2]);
        assert!(matches!(post.checks[3], CheckStatus::Skipped(_)), "{:?}", post.checks[3]);
        assert_eq!(post.settled_s, Some(AGENT_HELLO_TIMEOUT_S + 1.0), "nothing pending is settled");
    }

    #[test]
    fn a_connected_agent_that_never_tests_fails_for_omp_and_is_skipped_for_a_custom_command() {
        for (harness, failed) in [(TerminalAgent::OhMyPi, true), (TerminalAgent::Custom, false)] {
            let mut post = started_post(harness);
            post.hello(None, 22, 10.0);
            post.tick(10.0 + SELF_TEST_TIMEOUT_S - 1.0);
            assert_eq!(post.checks[3], CheckStatus::Pending);
            assert_eq!(post.settled_s, None);
            post.tick(10.0 + SELF_TEST_TIMEOUT_S + 1.0);
            assert_eq!(matches!(post.checks[3], CheckStatus::Failed(_)), failed, "{harness:?}: {:?}", post.checks[3]);
            assert_eq!(matches!(post.checks[3], CheckStatus::Skipped(_)), !failed, "{harness:?}: {:?}", post.checks[3]);
            assert!(post.settled_s.is_some());
        }
    }

    #[test]
    fn the_card_lingers_longer_after_a_failure_and_a_click_reruns_a_failed_omp_test() {
        let mut post = started_post(TerminalAgent::OhMyPi);
        post.hello(Some(("omp".to_owned(), "18".to_owned())), 22, 1.0);
        post.self_test(&[("state".to_owned(), true, "a".to_owned()), ("picture".to_owned(), false, "none".to_owned())]);
        assert_eq!(post.checks[3], CheckStatus::Failed("picture: none".to_owned()));
        post.tick(2.0);
        assert!(post.card_shown(2.0 + CARD_LINGER_S + 1.0), "a failure stays past the passing linger");
        assert!(!post.card_shown(2.0 + FAILED_LINGER_S + 1.0));
        assert!(post.rerun(100.0), "a failed omp test after a connection is asked again");
        assert_eq!(post.checks[3], CheckStatus::Pending);
        post.self_test(&[("state".to_owned(), true, "a".to_owned()), ("picture".to_owned(), true, "b".to_owned())]);
        assert_eq!(post.checks[3], CheckStatus::Passed("a · b".to_owned()));
        assert!(!post.rerun(102.0), "a passing test is not asked again");
    }

    /// **A resumed omp session is picked back up and not asked its self test again** (`feedback-1794148425925`, oracle
    /// `spec`): the line is `omp --resume <session>` with the extension, with no prompt typed.
    #[test]
    fn a_resumed_omp_plan_types_resume_with_the_session_and_no_self_test_prompt() {
        let setup = setup("resume");
        let session = "01a1bb82-fdc4-7343-83c6-b6dce13deffa";
        let launch = plan_resuming(TerminalAgent::OhMyPi, None, Some(&setup), Some(Path::new("/tmp/p")), Some(session), 0.0);
        let omp = setup.files.join("omp");
        assert_eq!(String::from_utf8_lossy(&launch.line), format!("omp -e '{}' --resume '{session}'\r", omp.display()));
        assert!(!String::from_utf8_lossy(&launch.line).contains("self test"), "no prompt is typed into a resumed session");
        assert!(matches!(launch.post.checks[3], CheckStatus::Skipped(_)), "{:?}", launch.post.checks[3]);
        assert_eq!(launch.post.project.as_deref(), Some(Path::new("/tmp/p")));
        assert_eq!(launch.post.resuming.as_deref(), Some(session));
        let mut post = launch.post;
        post.started();
        post.hello(None, 22, 5.0);
        assert!(matches!(post.checks[3], CheckStatus::Skipped(_)), "a connection does not ask a resumed session for its self test");
    }

    /// **An id that is typed into a shell is checked, and only omp resumes.**
    #[test]
    fn a_resume_of_a_string_that_is_not_a_session_or_of_a_harness_that_is_not_omp_starts_nothing() {
        let setup = setup("resume_refused");
        let bad = plan_resuming(TerminalAgent::OhMyPi, None, Some(&setup), None, Some("x; rm -rf ~"), 0.0);
        assert!(bad.line.is_empty(), "a session id with a shell in it is never typed");
        assert!(matches!(&bad.post.checks[0], CheckStatus::Failed(why) if why.contains("not a session id")), "{:?}", bad.post.checks[0]);
        let other = plan_resuming(TerminalAgent::Custom, Some("echo hi"), Some(&setup), None, Some("0197-abc"), 0.0);
        assert!(other.line.is_empty());
        assert!(matches!(&other.post.checks[0], CheckStatus::Failed(why) if why.contains("only an omp session")), "{:?}", other.post.checks[0]);
    }

    /// **A resumed agent that never connects strikes its own record**: the session cannot be resumed, so the next launch does
    /// not try it again.
    #[test]
    fn a_resumed_agent_that_never_connects_asks_for_its_record_to_go_and_a_fresh_one_does_not() {
        let setup = setup("resume_fails");
        for (resume, struck) in [(Some("0197-abc"), true), (None, false)] {
            let launch = plan_resuming(TerminalAgent::OhMyPi, None, Some(&setup), Some(Path::new("/tmp/p")), resume, 0.0);
            let mut post = launch.post;
            post.started();
            post.tick(AGENT_HELLO_TIMEOUT_S + 1.0);
            assert_eq!(post.resume_failed, struck, "resume {resume:?}");
        }
    }

    /// **The agent's session report names a tab by its token, and only a session id is taken** (it is typed into a shell next
    /// launch).
    #[test]
    fn a_session_report_for_no_tab_is_unknown_and_one_that_is_not_an_id_is_refused() {
        let mut terminal = Terminal::default();
        let stranger = agent_session(&mut terminal, &json!({"token": "nobody", "session": "0197-abc"}));
        assert_eq!(stranger, Ok(json!({"known": false})));
        let no_token = agent_session(&mut terminal, &json!({"token": null, "session": null}));
        assert_eq!(no_token, Ok(json!({"known": false})));
        assert!(agent_session(&mut terminal, &json!({"token": "t", "session": "a b; c"})).is_err(), "a string with a shell in it is refused");
        assert!(agent_session(&mut terminal, &json!({"token": "t", "session": "0197-abc", "extra": 1})).is_err(), "an unknown field is refused");
    }

    /// **The LED answers as the status chip does** (`feedback-1794230412409`, oracle `spec`): busy while a check waits,
    /// failed once one failed, live when nothing waits and nothing failed, and the chip's own words say the same in every state.
    #[test]
    fn the_led_is_busy_while_a_check_waits_failed_on_a_failure_and_live_when_none_waits_and_the_chip_agrees() {
        use bevy_gooey::LedState;
        let mut post = started_post(TerminalAgent::OhMyPi);
        assert_eq!((post.led(), post.chip().0.as_str()), (LedState::Busy, "omp: checking…"), "connected and self test still wait");
        post.hello(None, 22, 1.0);
        assert_eq!(post.led(), LedState::Busy, "connected, and the self test still waits");
        post.self_test(&[("state".to_owned(), true, "a".to_owned()), ("picture".to_owned(), true, "b".to_owned())]);
        assert_eq!((post.led(), post.chip().0.as_str()), (LedState::Live, "omp connected · 22 tools"));
        post.self_test(&[("picture".to_owned(), false, "empty".to_owned())]);
        assert_eq!((post.led(), post.chip().0.as_str()), (LedState::Failed, "omp: self test failed"));
    }

    /// **A failure outranks a check that still waits**, as the chip's words do: the agent that never connected has failed, not
    /// "busy".
    #[test]
    fn a_failed_check_is_failed_even_while_another_still_waits() {
        let mut post = started_post(TerminalAgent::OhMyPi);
        post.checks[2] = CheckStatus::Failed("never reached the editor".to_owned());
        post.checks[3] = CheckStatus::Pending;
        assert_eq!(post.led(), bevy_gooey::LedState::Failed);
    }

    /// **A skipped self test is not a reason to stay busy**: a custom command that passes no prompt is connected and its test
    /// is skipped, which the chip calls connected.
    #[test]
    fn a_skipped_self_test_leaves_the_led_live() {
        let mut post = started_post(TerminalAgent::Custom);
        post.hello(None, 3, 10.0);
        post.tick(10.0 + SELF_TEST_TIMEOUT_S + 1.0);
        assert!(matches!(post.checks[3], CheckStatus::Skipped(_)), "{:?}", post.checks[3]);
        assert_eq!(post.led(), bevy_gooey::LedState::Live);
    }
}
