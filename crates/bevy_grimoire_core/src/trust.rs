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
pub const COMMAND_FILES: [&str; 3] = ["agent.ron", "play.ron", "AGENTS.md"];

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

    /// The button a person presses to start it again once it is allowed.
    pub fn verb(self) -> &'static str {
        match self {
            CommandKind::Notes => "Send",
            CommandKind::Play => "Play",
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
        let mut store: TrustStore =
            ron::from_str(&text).map_err(|e| format!("trust: {} does not read ({e}). Fix or delete it: nothing is trusted until it does.", file.display()))?;
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
        // **One grant per project**: a guide allowed twice is the newer text's grant and the older one is
        // kept too, so a person who edits back to an earlier wording is not asked again for a guide they
        // have already read.
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

/// **The SHA-256 of `bytes`, as lowercase hex.**
///
/// Here rather than as a dependency because `tests/engine_free.rs` keeps this crate's dependency list
/// closed, and a digest is arithmetic: sixty lines against a crate every game would then link. The risk
/// that buys is a digest that is subtly wrong, which would withhold a guide whose grant was made and read
/// as a trust bug — so the published vectors (FIPS 180-4 / NIST's `SHA256ShortMsg` set) are checked in
/// `tests/a_projects_guide_needs_the_persons_trust.rs`, including the 55-, 56- and 64-byte lengths where
/// the padding is the part worth getting wrong.
pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    // **The padding: a 1 bit, zeros up to 56 bytes into the block, then the length in bits as a u64.**
    let mut message = bytes.to_vec();
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let (blocks, _) = message.as_chunks::<64>();
    for block in blocks {
        let mut w = [0u32; 64];
        let (words, _) = block.as_chunks::<4>();
        for (i, word) in words.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for (i, k) in K.iter().enumerate() {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(*k).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for (slot, added) in h.iter_mut().zip(v) {
            *slot = slot.wrapping_add(added);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

/// **Has a person allowed this project's guide, as this text?**
pub fn guide_decision(store: &TrustStore, root: &Path, text: &str) -> Result<Decision, String> {
    store.guide_decision(root, text)
}

/// **A person allows this project's guide, exactly as it reads now.**
pub fn grant_guide(store: &mut TrustStore, root: &Path, text: &str) -> Result<(), String> {
    store.trust_guide(root, text)
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
