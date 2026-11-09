//! `agent.ron`: the command Send starts for the notes (#94).
//!
//! A project names a coding agent here, and pressing Send in the editor starts it once, so a person
//! with no agent running still gets their notes worked. The editor holds no agent logic: it starts a
//! process, which connects over MCP like any hand-started agent. With no file, or no command, Send only
//! queues the notes. Engine-free: `std::process` only.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::trust::{CommandKind, CommandSpec, Decision, TrustStore};

pub const AGENT_VERSION: u32 = 1;

/// The model an agent is started with when the file does not name one.
pub const DEFAULT_MODEL: &str = "claude-sonnet-5-5";

/// What the started agent is asked. It names the guide, so the prompt stays short and the rules live in one place.
///
/// **`grimoire_guide` first, not a document** (#346, AG-01). The guide is shipped inside the MCP
/// server and returned by that tool, so the agent reads the text the server actually carries — the
/// same text `docs/mcp-tools.md` is rendered from — rather than a file that may not be there.
pub const DEFAULT_PROMPT: &str = "Work the sent notes on the open map through the editor's MCP server. \
Call grimoire_guide first: call grimoire_state, claim a sent note with note_claim, stay inside its area, \
ask before removing a person's piece, and finish with note_resolve.";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSettings {
    pub version: u32,
    /// The program to start. `None` means Send only queues the notes.
    #[serde(default)]
    pub command: Option<String>,
    /// Its arguments. `{model}`, `{prompt}` and `{project}` are replaced. Empty means `-p --model {model} {prompt}`, which is `omp`'s headless form.
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_model() -> String {
    DEFAULT_MODEL.to_owned()
}

impl Default for AgentSettings {
    fn default() -> Self {
        AgentSettings { version: AGENT_VERSION, command: None, args: Vec::new(), model: default_model() }
    }
}

/// `<project>/agent.ron`.
pub fn path_for_project(root: &Path) -> PathBuf {
    root.join("agent.ron")
}

impl AgentSettings {
    pub fn from_ron(text: &str) -> Result<AgentSettings, String> {
        let s: AgentSettings = ron::from_str(text).map_err(|e| format!("agent: parse: {e}"))?;
        if s.version != AGENT_VERSION {
            return Err(format!("agent: version {} is not the {AGENT_VERSION} this build reads", s.version));
        }
        if s.command.as_deref().is_some_and(|c| c.trim().is_empty()) {
            return Err("agent: `command` is empty; write a program or remove the line".to_owned());
        }
        Ok(s)
    }

    /// Reads the project's `agent.ron`; a project without one launches nothing.
    pub fn load(root: &Path) -> Result<AgentSettings, String> {
        let path = path_for_project(root);
        match std::fs::read_to_string(&path) {
            Ok(text) => AgentSettings::from_ron(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AgentSettings::default()),
            Err(e) => Err(format!("agent: {}: {e}", path.display())),
        }
    }

    /// The arguments with `{model}`, `{prompt}` and `{project}` filled in.
    pub fn argv(&self, root: &Path) -> Vec<String> {
        let template: Vec<String> = if self.args.is_empty() {
            ["-p", "--model", "{model}", "{prompt}"].map(str::to_owned).to_vec()
        } else {
            self.args.clone()
        };
        let project = root.to_string_lossy();
        template
            .iter()
            .map(|a| a.replace("{model}", &self.model).replace("{prompt}", DEFAULT_PROMPT).replace("{project}", &project))
            .collect()
    }

    /// The command as a person reads and trusts it, or `None` when no command is set. It is the template
    /// (`{model}` and `{prompt}` unfilled), so trust follows what the file says, and it is the form the
    /// default arguments take when the file names none.
    pub fn spec(&self) -> Option<CommandSpec> {
        let program = self.command.clone()?;
        let args = if self.args.is_empty() {
            ["-p", "--model", "{model}", "{prompt}"].map(str::to_owned).to_vec()
        } else {
            self.args.clone()
        };
        Some(CommandSpec { kind: CommandKind::Notes, program, args, cwd: None })
    }

    /// **Start the agent, once, if a person has allowed it.** `Ok(None)` when no command is set. A command
    /// no person has allowed is [`LaunchError::Untrusted`] and nothing is started. The child runs in the
    /// project folder with `BEVY_GRIMOIRE_PROJECT` set; a thread reaps it when it ends. A program that
    /// cannot start is [`LaunchError::Failed`] naming it, so the editor reports it rather than dropping
    /// the send silently.
    pub fn launch(&self, root: &Path, trust: &TrustStore) -> Result<Option<u32>, LaunchError> {
        let Some(spec) = self.spec() else { return Ok(None) };
        match trust.decision(root, &spec).map_err(LaunchError::Failed)? {
            Decision::Trusted(_) => {}
            Decision::Untrusted => return Err(LaunchError::Untrusted(spec)),
        }
        let program = &spec.program;
        let mut child = Command::new(program)
            .args(self.argv(root))
            .current_dir(root)
            .env("BEVY_GRIMOIRE_PROJECT", root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| LaunchError::Failed(format!("could not start the notes agent `{program}`: {e}")))?;
        let pid = child.id();
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(Some(pid))
    }
}

/// **Why a launch did not start anything.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchError {
    /// No person has allowed this command in this project. The editor asks; nothing was started.
    Untrusted(CommandSpec),
    /// It was allowed and still could not start, or the trust could not be decided.
    Failed(String),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaunchError::Untrusted(spec) => write!(f, "the {} `{}` is not allowed yet", spec.kind.noun(), spec.display()),
            LaunchError::Failed(why) => f.write_str(why),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_with_no_command_is_the_default_and_a_blank_command_is_refused() {
        let s = AgentSettings::from_ron("(version: 1)").unwrap();
        assert_eq!(s, AgentSettings::default());
        assert!(AgentSettings::from_ron("(version: 1, command: Some(\" \"))").is_err());
        assert!(AgentSettings::from_ron("(version: 2)").is_err());
    }

    #[test]
    fn placeholders_are_filled_and_the_default_form_is_omps() {
        let s = AgentSettings::from_ron("(version: 1, command: Some(\"omp\"), model: \"m1\")").unwrap();
        let argv = s.argv(Path::new("/p"));
        assert_eq!(&argv[..3], ["-p", "--model", "m1"]);
        assert!(argv[3].contains("note_resolve"));
        let s = AgentSettings::from_ron("(version: 1, command: Some(\"x\"), args: [\"{project}\"])").unwrap();
        assert_eq!(s.argv(Path::new("/p")), ["/p"]);
    }
}
