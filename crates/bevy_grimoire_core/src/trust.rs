//! **What a person has allowed a project to run.**
//!
//! `agent.ron` and `play.ron` name programs, and the editor used to start whatever they named the moment
//! a person pressed Send or Play. A project cloned from somewhere else could therefore run its author's
//! command on a single key press (the review of 2026-11-02, risk 1; the same hole as an editor opening an
//! untrusted workspace). Nothing here starts a program. This module only answers one question, "has a
//! person allowed this command in this project?", and keeps the answer in the **person's own file**.
//!
//! # Two ways to allow, and why both
//!
//! - **The exact command** ([`Scope::Command`]): the program, its arguments as written, its working
//!   directory and which command it is. An edit to any of them is a different command and asks again,
//!   which is what stops a command planted *after* the person looked, by an agent that can write files
//!   or by a hot-reloaded edit.
//! - **The whole project** ([`Scope::Project`]): every command of that directory. For a person iterating
//!   on their own game, who would otherwise be asked after every edit of `play.ron`.
//!
//! # The rules
//!
//! - **The store is outside the project.** A project cannot ship its own trust: the file lives in the
//!   user's data directory (the caller supplies the path; this crate does not look for one).
//! - **A grant is for a project path, resolved.** The root is canonicalised, so a symlink to a trusted
//!   project is that project and a look-alike directory is not. A root that cannot be resolved is
//!   refused, not guessed.
//! - **A damaged store is refused.** It is never read as "nothing is trusted" or as "everything is": the
//!   person is told which file is wrong. A missing file is the one empty case, because that is a person
//!   who has trusted nothing yet.
//! - **A grant is made by a person, in the editor.** It is deliberately not an operation in the
//!   editor's registry, which is the vocabulary an agent shares with a person: an agent that could call
//!   it could trust itself.
//!
//! Engine-free: `std` and `serde` only.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The version of the store's file.
pub const TRUST_VERSION: u32 = 1;

/// The store's file name, inside the editor's own folder of the user's data directory.
pub const STORE_FILE: &str = "trusted.ron";

/// **Which command of a project.** The same words in two kinds are two commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandKind {
    /// The notes agent `agent.ron` names, started by Send.
    Notes,
    /// The game `play.ron` names, started by Play.
    Play,
}

impl CommandKind {
    /// What a person calls it.
    pub fn noun(self) -> &'static str {
        match self {
            CommandKind::Notes => "notes agent",
            CommandKind::Play => "game",
        }
    }
}

/// **A command as the project's file writes it**, before `{map}`, `{model}` and the like are filled in:
/// trust follows the template a person read, not one run's values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub kind: CommandKind,
    pub program: String,
    pub args: Vec<String>,
    /// The working directory relative to the project root, when the file names one.
    pub cwd: Option<String>,
}

impl CommandSpec {
    /// The command as a person reads it: `cargo run -- {map}`.
    pub fn display(&self) -> String {
        let mut words = vec![self.program.as_str()];
        words.extend(self.args.iter().map(String::as_str));
        let line = words.join(" ");
        match &self.cwd {
            Some(dir) => format!("{line} (in {dir})"),
            None => line,
        }
    }
}

/// How far a grant reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Command,
    Project,
}

/// **The answer.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Trusted(Scope),
    Untrusted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    project: PathBuf,
    command: CommandSpec,
}

/// **Everything this person has allowed.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustStore {
    version: u32,
    projects: Vec<PathBuf>,
    commands: Vec<Grant>,
}

impl Default for TrustStore {
    /// A person who has trusted nothing.
    fn default() -> Self {
        TrustStore { version: TRUST_VERSION, projects: Vec::new(), commands: Vec::new() }
    }
}

fn resolve(root: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(root)
        .map_err(|e| format!("the project folder {} cannot be resolved, so nothing about it can be trusted: {e}", root.display()))
}

impl TrustStore {
    /// Reads the store. A file that does not exist is an empty store; a file that does not read is an
    /// `Err` naming it, because guessing would either lock a person out or trust everything.
    pub fn load(file: &Path) -> Result<TrustStore, String> {
        let text = match std::fs::read_to_string(file) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(TrustStore::default()),
            Err(e) => return Err(format!("trust: {}: {e}", file.display())),
        };
        let store: TrustStore =
            ron::from_str(&text).map_err(|e| format!("trust: {} does not read ({e}). Fix or delete it: nothing is trusted until it does.", file.display()))?;
        if store.version != TRUST_VERSION {
            return Err(format!(
                "trust: {} is version {}, and this build reads version {TRUST_VERSION}. Nothing is trusted until it does.",
                file.display(),
                store.version
            ));
        }
        Ok(store)
    }

    /// Writes the store all or nothing, creating its folder.
    pub fn save(&self, file: &Path) -> Result<(), String> {
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("trust: cannot write {}: {e}", file.display()))?;
        crate::atomic_file::write(file, text).map_err(|e| format!("trust: {}: {e}", file.display()))
    }

    /// **Has a person allowed this command in this project?** An `Err` when the project folder cannot
    /// be resolved.
    pub fn decision(&self, root: &Path, command: &CommandSpec) -> Result<Decision, String> {
        let root = resolve(root)?;
        if self.projects.contains(&root) {
            return Ok(Decision::Trusted(Scope::Project));
        }
        if self.commands.iter().any(|g| g.project == root && g.command == *command) {
            return Ok(Decision::Trusted(Scope::Command));
        }
        Ok(Decision::Untrusted)
    }

    /// A person allows every command of this project.
    pub fn trust_project(&mut self, root: &Path) -> Result<(), String> {
        let root = resolve(root)?;
        if !self.projects.contains(&root) {
            self.projects.push(root);
        }
        Ok(())
    }

    /// A person allows this one command, exactly, in this project.
    pub fn trust_command(&mut self, root: &Path, command: &CommandSpec) -> Result<(), String> {
        let project = resolve(root)?;
        let grant = Grant { project, command: command.clone() };
        if !self.commands.contains(&grant) {
            self.commands.push(grant);
        }
        Ok(())
    }

    /// **What to say when a command is not allowed**: what it is, where it came from and who can allow it.
    pub fn untrusted_sentence(root: &Path, command: &CommandSpec) -> String {
        format!(
            "{} wants to start the {} `{}`, and no person has allowed that. Nothing was started. A person can allow this command or the whole project in the editor; an agent cannot.",
            root.display(),
            command.kind.noun(),
            command.display()
        )
    }
}
