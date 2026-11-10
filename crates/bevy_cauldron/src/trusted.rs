//! **The editor's half of project-command trust: the store's place, the question, and the answer.**
//!
//! `bevy_grimoire_core::trust` decides whether a person has allowed a project's `agent.yaml` or `play.yaml`
//! command; this module is where the editor asks that person and writes down what they said. The rules
//! are in the core module's docs. The two that shape this one:
//!
//! - **Only a person answers.** The grant is made here, from the confirm dialog's buttons and keys. It
//!   is deliberately not an operation in [`crate::ops::registry`], which is the vocabulary an agent
//!   shares with a person: an operation an agent could call is a trust an agent could give itself.
//!   `tests/a_project_command_needs_the_persons_trust.rs` checks that no operation is called `trust*`.
//! - **A refusal is where a person looks.** An untrusted command is refused on the status line and
//!   raises the question; an agent's `play` is refused with the same sentence, and the question is
//!   waiting for the person.
//!
//! The question has three answers (`Y` this command, `P` the whole project, `N` not now). After a grant
//! the person presses Send or Play again; nothing starts on its own, because a start the person did not
//! just ask for is the thing this module exists to prevent.

use bevy::prelude::*;
use bevy_grimoire_core::trust::{CommandSpec, Decision, Scope, TrustStore, STORE_FILE};
use std::path::{Path, PathBuf};

use crate::confirm::{Asked, Choice, Confirm};
use crate::editor::EditorState;

/// **Where the person's stores live**, when the program has said. The binary names the user's data
/// directory; a test names a scratch folder, so no test reads or writes the real `trusted.ron`.
///
/// **The override is the store's *file*, not its folder** ([`TrustState::file`]), and this is the folder a
/// `None` override resolves to — the one fact that used to be a hard-wired call to
/// `bevy_grimoire_core::user_dirs::data_dir()` inside [`store_path`], where a test could not reach it.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct StoreRoot(pub PathBuf);

/// **The person's store, resolved once**: the named file, else the [`StoreRoot`], else the user's data
/// directory (`None` is that directory, never "no store" — a real editor names no file).
pub fn resolve(root: Option<&StoreRoot>, over: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(file) = over {
        return Ok(file.to_path_buf());
    }
    match root {
        Some(root) => Ok(root.0.join(STORE_FILE)),
        None => bevy_grimoire_core::user_dirs::data_dir()
            .map(|d| d.join(STORE_FILE))
            .ok_or_else(|| "this machine has no user data directory, so there is nowhere to keep what you trust".to_owned()),
    }
}

/// **A command waiting for a person's answer.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustRequest {
    pub root: PathBuf,
    pub spec: CommandSpec,
}

/// **Where the question stands**, kept on the editor's state because Send and Play are plain functions
/// over it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustState {
    /// Raised by Send or Play; the next frame asks it.
    pub pending: Option<TrustRequest>,
    /// Asked, not answered.
    pub awaiting: Option<TrustRequest>,
    /// Where this person's store is. `None` is the user's data directory; a test points it at a file of
    /// its own so no test reads or writes the real one.
    pub file: Option<PathBuf>,
}

/// `<data_dir>/Grimoire/trusted.ron`.
pub fn store_path(over: Option<&Path>) -> Result<PathBuf, String> {
    resolve(None, over)
}

/// Reads the person's store through a [`StoreRoot`], so a test's scratch folder is where it looks.
pub fn load_at(root: Option<&StoreRoot>, over: Option<&Path>) -> Result<TrustStore, String> {
    TrustStore::load(&resolve(root, over)?)
}

/// Reads the person's store; a damaged one is an `Err` that names it.
pub fn load(over: Option<&Path>) -> Result<TrustStore, String> {
    TrustStore::load(&store_path(over)?)
}

/// **Is this command allowed?** `Ok(None)` when it is; `Ok(Some(request))` when a person must be asked.
pub fn needs_answer(over: Option<&Path>, root: &Path, spec: &CommandSpec) -> Result<Option<TrustRequest>, String> {
    match load(over)?.decision(root, spec)? {
        Decision::Trusted(_) => Ok(None),
        Decision::Untrusted => Ok(Some(TrustRequest { root: root.to_path_buf(), spec: spec.clone() })),
    }
}

/// **Write down what the person said.**
pub fn grant(over: Option<&Path>, request: &TrustRequest, scope: Scope) -> Result<(), String> {
    let file = store_path(over)?;
    let mut store = TrustStore::load(&file)?;
    match scope {
        Scope::Command => store.trust_command(&request.root, &request.spec)?,
        Scope::Project => store.trust_project(&request.root)?,
    }
    store.save(&file)
}

pub struct TrustPlugin;

impl Plugin for TrustPlugin {
    fn build(&self, app: &mut App) {
        // After `Phase::Act`: the answer is held until read, so nothing here needs to run before the verbs,
        // and running after the confirm plugin's key handling (which is before `Act`) means the answer
        // is there to take in the frame it was given. That the answering key is not also a verb is the
        // census's doing: an open question owns the keyboard (`editor::sense_context`).
        app.add_systems(Update, (ask_the_person, take_the_answer).chain().after(crate::keys::Phase::Act));
    }
}

/// Raises the question a Send or a Play left, once no other question is up.
fn ask_the_person(state: Option<ResMut<EditorState>>, mut confirm: ResMut<Confirm>) {
    let Some(mut state) = state else { return };
    if state.trust.awaiting.is_some() || confirm.is_open() {
        return;
    }
    let Some(request) = state.trust.pending.take() else { return };
    confirm.ask_three(
        Asked::TrustCommand,
        format!("Allow this project to start its {}?", request.spec.kind.noun()),
        format!("{}\nin {}", request.spec.display(), request.root.display()),
        "Allow this command",
        "Trust the whole project",
        KeyCode::KeyP,
        "Not now",
    );
    state.trust.awaiting = Some(request);
}

/// Writes the person's answer down and says what happened.
fn take_the_answer(state: Option<ResMut<EditorState>>, mut confirm: ResMut<Confirm>) {
    let Some(mut state) = state else { return };
    let Some(choice) = confirm.answer_choice(Asked::TrustCommand) else { return };
    let Some(request) = state.trust.awaiting.take() else { return };
    let scope = match choice {
        Choice::Yes => Scope::Command,
        Choice::Alt => Scope::Project,
        Choice::No => {
            state.status.note(format!("Not allowed, so the {} was not started.", request.spec.kind.noun()));
            return;
        }
    };
    match grant(state.trust.file.as_deref(), &request, scope) {
        Ok(()) => {
            let what = if scope == Scope::Project { "this project's commands" } else { "that command" };
            state.status.note(format!("Allowed {what}. Press {} again to start it.", request.spec.kind.verb()));
        }
        Err(e) => state.status.problem(e),
    }
}
