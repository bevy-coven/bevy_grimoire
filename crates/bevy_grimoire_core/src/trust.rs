//! **What a person has allowed a project to run.**
//!
//! `agent.yaml` and `play.yaml` name programs, and the editor used to start whatever they named the moment
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
//!   on their own game, who would otherwise be asked after every edit of `play.yaml`.
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

/// The version of the store's file. **2** added guide grants ([`GuideGrant`]); a version 1 store is still
/// read, with no guides allowed, because the projects a person trusted yesterday are still trusted.
pub const TRUST_VERSION: u32 = 2;

/// The version this build still reads, so a store written before guide grants existed is not a refusal.
pub const TRUST_VERSION_READS_FROM: u32 = 1;

/// **The project files whose content names a program to start, or tells an agent what to do.** Nothing
/// may edit these as data: see `data_files::DataFiles::parse`.
///
/// `AGENTS.md` is here because `data_set` edits a registered file's values: a guide registered as a data
/// file would let an agent edit the very rules it is being asked to follow.
pub const COMMAND_FILES: [&str; 3] = ["agent.yaml", "play.yaml", "AGENTS.md"];

/// The store's file name, inside the editor's own folder of the user's data directory.
pub const STORE_FILE: &str = "trusted.yaml";

/// **Which command of a project.** The same words in two kinds are two commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandKind {
    /// The notes agent `agent.yaml` names, started by Send.
    Notes,
    /// The game `play.yaml` names, started by Play.
    Play,
    /// **The build `play.yaml` names** (GD-09, #371), run by Build before a change that needs Rust can
    /// be seen. A kind of its own, because a person allows the game and the rebuild separately: the
    /// game is what they watch, and the rebuild is a compiler.
    Build,
}

impl CommandKind {
    /// What a person calls it.
    pub fn noun(self) -> &'static str {
        match self {
            CommandKind::Notes => "notes agent",
            CommandKind::Play => "game",
            CommandKind::Build => "build",
        }
    }

    /// The button a person presses to start it again once it is allowed.
    pub fn verb(self) -> &'static str {
        match self {
            CommandKind::Notes => "Send",
            CommandKind::Play => "Play",
            CommandKind::Build => "Build",
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

/// **A person allowed one project's guide, as it read at that moment.**
///
/// The digest is of the file's **bytes**, so one changed byte is a different guide and asks again: a guide
/// an agent rewrote through its shell has not been allowed. A `Scope::Project` grant covers the guide
/// too, which is what trusting a project means.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuideGrant {
    pub project: PathBuf,
    /// The SHA-256 of the text, lowercase hex — see [`sha256_hex`].
    pub sha256: String,
}

/// **Everything this person has allowed.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustStore {
    version: u32,
    projects: Vec<PathBuf>,
    commands: Vec<Grant>,
    /// **The guides allowed, one per project.** Empty for a store written before this field existed, which
    /// is why it has a default: the projects that store trusted are still trusted.
    #[serde(default)]
    guides: Vec<GuideGrant>,
}

impl Default for TrustStore {
    /// A person who has trusted nothing.
    fn default() -> Self {
        TrustStore { version: TRUST_VERSION, projects: Vec::new(), commands: Vec::new(), guides: Vec::new() }
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
        let mut store: TrustStore = crate::yaml::from_str(&text, &file.display().to_string()).map_err(|p| {
            format!(
                "trust: {} does not read ({}). Fix or delete it: nothing is trusted until it does.",
                file.display(),
                p.line()
            )
        })?;
        if store.version < TRUST_VERSION_READS_FROM || store.version > TRUST_VERSION {
            return Err(format!(
                "trust: {} is version {}, and this build reads versions {TRUST_VERSION_READS_FROM} to {TRUST_VERSION}. Nothing is trusted until it does.",
                file.display(),
                store.version
            ));
        }
        // **A store from before guide grants reads as one with none**, and is written at this build's
        // version the next time a person allows anything — which is a migration, not a silent upgrade: the
        // projects it trusted are kept.
        store.version = TRUST_VERSION;
        Ok(store)
    }

    /// Writes the store all or nothing, creating its folder.
    pub fn save(&self, file: &Path) -> Result<(), String> {
        let text = crate::yaml::to_string(self)
            .map_err(|p| format!("trust: cannot write {}: {}", file.display(), p.line()))?;
        crate::yaml::save(file, &text)
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

    /// **Has a person allowed this project's guide, as this text?** An `Err` when the project folder
    /// cannot be resolved. The text's digest is what a [`GuideGrant`] holds, so any edit is a new question.
    pub fn guide_decision(&self, root: &Path, text: &str) -> Result<Decision, String> {
        let root = resolve(root)?;
        if self.projects.contains(&root) {
            return Ok(Decision::Trusted(Scope::Project));
        }
        let digest = sha256_hex(text.as_bytes());
        if self.guides.iter().any(|g| g.project == root && g.sha256 == digest) {
            return Ok(Decision::Trusted(Scope::Command));
        }
        Ok(Decision::Untrusted)
    }

    /// **A person allows this project's guide, exactly as it reads now.**
    pub fn trust_guide(&mut self, root: &Path, text: &str) -> Result<(), String> {
        let project = resolve(root)?;
        // **One grant per (project, text)**: an older grant is kept, so a person who edits back to a wording
        // they already allowed is not asked again for a guide they have read; a new text is a new entry.
        let grant = GuideGrant { project: project.clone(), sha256: sha256_hex(text.as_bytes()) };
        if !self.guides.contains(&grant) {
            self.guides.push(grant);
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

/// **The SHA-256 of `bytes`, as lowercase hex**, which is what a [`GuideGrant`] holds: whether a project's
/// guide is the exact text a person read. The published construction, with published vectors checked in
/// `tests/a_projects_guide_needs_the_persons_trust.rs` — the question is identity, not secrecy, and a
/// digest that was subtly wrong would withhold a guide whose grant was made and read as a trust bug.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// **What an agent is told when the project's guide is withheld.**
///
/// One line, because it stands where a layer would have been: the agent is not shown the text, and it is
/// told why and who can change that. A person reading it in the editor reads the same words.
pub fn withheld_sentence(root: &Path) -> String {
    format!(
        "{} has a guide at `.dark_factory/AGENTS.md` that no person has allowed, so it was not served. Ask them to allow it in the editor; an agent cannot.",
        root.display()
    )
}
