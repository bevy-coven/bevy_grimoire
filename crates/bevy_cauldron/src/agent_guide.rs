//! **The layers of an agent's guide the editor composes** (#347, AG-02; #348, AG-03).
//!
//! `bevy_grimoire_core::agent_guide` reads one file and refuses what it cannot serve whole; this module
//! knows **where** the two files are, which is the editor's knowledge and nobody else's: the person's
//! file is in their OS config directory, and the project's is inside the project it has open. So the MCP
//! server puts its own shipped layer first and reads no file itself, and `grimoire/agent_guide` returns
//! what this module read.
//!
//! **Both files are read on each call.** An agent asks at the start of a task, and a rule a person wrote
//! ten seconds ago is the one worth following; holding a copy would be a cache with no invalidation
//! anything can see.
//!
//! # The project's layer needs the person's trust
//!
//! A project cloned from somewhere else can carry a `.dark_factory/AGENTS.md` that tells an agent with a
//! shell to do anything, and an agent follows a guide it is given. So the project's layer is served only
//! when the person has allowed **that exact content** (`trust::GuideGrant`, the SHA-256 of the bytes) or
//! **the whole project** (`Scope::Project`). The person's own layer needs no trust: it is theirs.
//!
//! **Withheld, the layer becomes one line** saying the project has a guide nobody has allowed — never the
//! text, and never silence, because "this project has no guide" and "this project has a guide you cannot
//! see" are different facts an agent must be able to tell apart.
//!
//! **The question is asked in the confirm modal**, because a withheld guide blocks an agent's first task,
//! and the three answers mirror a command's: **Allow this guide**, **Trust this project**, **Not now**.
//! **"Not now" holds for that content for the rest of the session** — agents ask at the start of every
//! task, and a modal that came back each time would be one a person learns to dismiss. A changed file is
//! new content and asks again.
//!
//! **A damaged store withholds the guide and says so.** It is never read as "nothing is trusted" or as
//! "everything is", which is the rule `trust` already lives under.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_grimoire_core::agent_guide::{self, Guide, Layer, Source};
use bevy_grimoire_core::problem::Problem;
use bevy_grimoire_core::trust::{self, Decision, Scope, TrustStore};

use crate::confirm::{Asked, Choice, Confirm};
use crate::editor::EditorState;
use crate::project::Project;

/// **Where the person's guide is**, when the binary has said. A test points it at a file of its own, as
/// `settings::UserSettingsFile` does, so no test reads or writes the real one.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct UserGuideFile(pub PathBuf);

/// `<config_dir>/Grimoire/AGENTS.md`, or `None` on a platform with no config directory.
pub fn user_file() -> Option<PathBuf> {
    bevy_grimoire_core::user_dirs::config_dir().map(|dir| dir.join(agent_guide::USER_FILE))
}

/// `<root>/.dark_factory/AGENTS.md`.
pub fn project_file(root: &Path) -> PathBuf {
    root.join(agent_guide::PROJECT_PATH)
}

/// **What the editor serves an agent, and why.**
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Composed {
    /// The layers this person has allowed, the person's first.
    pub guide: Guide,
    /// The sentence that stands in the withheld project layer's place, when there is one.
    pub withheld: Option<String>,
    /// The project's guide as it read, kept for the question: the text a grant would be for.
    pub project_text: Option<String>,
    /// Why no decision could be made about the project's guide: a store that will not read, or a project
    /// root that will not resolve.
    pub trouble: Option<String>,
}

/// **The guide as the person has allowed it.** `trust_file` is the store (a test's own, so no test reads
/// the real one).
pub fn compose(project: Option<&Project>, user: Option<&UserGuideFile>, trust_file: Option<&Path>) -> Composed {
    let mut composed = Composed::default();
    if let Some(user) = user {
        match agent_guide::read_layer(Source::User, &user.0) {
            Ok(Some(text)) => {
                composed.guide.layers.push(Layer { source: Source::User, path: user.0.clone(), text });
            }
            Ok(None) => {}
            Err(problem) => composed.guide.problems.push(*problem),
        }
    }
    // **No project open is no project layer**, and not a problem: the Projects home is a screen an agent
    // may be on with nothing open.
    let Some(project) = project else { return composed };
    let path = project_file(&project.root);
    let text = match agent_guide::read_layer(Source::Project, &path) {
        Ok(None) => return composed,
        Ok(Some(text)) => text,
        // **A guide that cannot be served is a problem and not a question**: allowing it would not help, so
        // withholding it behind a modal would be asking a person to approve something unusable.
        Err(problem) => {
            composed.guide.problems.push(*problem);
            return composed;
        }
    };
    let withheld = trust::withheld_sentence(&project.root);
    let decision = match trust_file.map(TrustStore::load) {
        Some(Ok(store)) => match trust::guide_decision(&store, &project.root, &text) {
            Ok(decision) => decision,
            Err(e) => {
                composed.trouble = Some(e);
                composed.withheld = Some(withheld);
                return composed;
            }
        },
        Some(Err(e)) => {
            composed.trouble = Some(e);
            composed.withheld = Some(withheld);
            return composed;
        }
        // **No store named is no place to keep an answer**, so the question cannot be answered and the
        // layer is withheld with the reason rather than served on the strength of a missing file.
        None => {
            composed.trouble = Some(
                "there is nowhere to keep what you have allowed, so this project's guide cannot be allowed"
                    .to_owned(),
            );
            composed.withheld = Some(withheld);
            return composed;
        }
    };
    match decision {
        Decision::Trusted(_) => {
            composed.guide.layers.push(Layer { source: Source::Project, path, text });
        }
        Decision::Untrusted => {
            // **The layer is one line and the text stays out of the answer**, whether or not this person
            // has already said not now — the only difference is whether the question is raised again,
            // which is [`serve`]'s decision.
            composed.withheld = Some(withheld);
            composed.project_text = Some(text);
        }
    }
    composed
}

/// **A guide waiting for a person's answer.**
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuideRequest {
    pub root: PathBuf,
    pub text: String,
}

/// **Where the guide's question stands**, kept on the editor's state because the answer arrives in a
/// system rather than in the call that raised it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuideTrust {
    /// Raised by a `grimoire/agent_guide` call whose project layer was withheld; the next frame asks it.
    pub pending: Option<GuideRequest>,
    /// Asked, not answered.
    pub awaiting: Option<GuideRequest>,
    /// **What this person said "not now" to**, as `(project root, digest)`, for the rest of the session.
    /// A changed file is a new digest and asks again; the same text never asks twice.
    pub denied: BTreeSet<(PathBuf, String)>,
}

impl GuideTrust {
    /// Whether this content was already refused in this session.
    pub fn denied_at(&self, root: &Path, text: &str) -> bool {
        self.denied.contains(&(root.to_path_buf(), digest(text)))
    }
}

/// **The digest a grant is for**, so the session's memory and the store agree on identity.
pub fn digest(text: &str) -> String {
    trust::sha256_hex(text.as_bytes())
}

pub struct AgentGuidePlugin;

impl Plugin for AgentGuidePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (ask_the_person, take_the_answer).chain().after(crate::keys::Phase::Act),
        );
    }
}

/// **What the question shows of a guide**, capped so the card stays a card. A guide longer than this is
/// allowed by its first lines and its size, which is the honest thing a modal can offer.
pub const SHOWN_CHARS: usize = 1_200;

fn text_preview(text: &str) -> String {
    let total = text.chars().count();
    if total <= SHOWN_CHARS {
        return text.trim_end().to_owned();
    }
    let head: String = text.chars().take(SHOWN_CHARS).collect();
    format!("{}\n\n… and {} more characters.", head.trim_end(), total - SHOWN_CHARS)
}

/// Raises the guide question a withheld layer left, once no other question is up.
fn ask_the_person(state: Option<ResMut<EditorState>>, mut confirm: ResMut<Confirm>) {
    let Some(mut state) = state else { return };
    if state.guide.awaiting.is_some() || confirm.is_open() {
        return;
    }
    let Some(request) = state.guide.pending.take() else { return };
    // **The text is shown, because the text is what is being allowed.** A question that asked "allow this
    // project's guide?" over a filename would be asking a person to trust a file they have not read.
    let body = format!("{}\n\n---\n{}", project_file(&request.root).display(), text_preview(&request.text));
    confirm.ask_three(
        Asked::TrustGuide,
        "Give this project's guide to your agent?".to_owned(),
        body,
        "Allow this guide",
        "Trust this project",
        KeyCode::KeyP,
        "Not now",
    );
    state.guide.awaiting = Some(request);
}

/// Writes the person's answer down and says what happened.
fn take_the_answer(state: Option<ResMut<EditorState>>, mut confirm: ResMut<Confirm>) {
    let Some(mut state) = state else { return };
    let Some(choice) = confirm.answer_choice(Asked::TrustGuide) else { return };
    let Some(request) = state.guide.awaiting.take() else { return };
    if choice == Choice::No {
        state.guide.denied.insert((request.root.clone(), digest(&request.text)));
        state.status.note(
            "Not allowed, so this project's guide was not given to your agent. You can allow it on the project page."
                .to_owned(),
        );
        return;
    }
    let scope = if choice == Choice::Alt { Scope::Project } else { Scope::Command };
    let written = match state.trust.file.as_deref() {
        Some(path) => match TrustStore::load(path) {
            Ok(mut store) => {
                let answer = match scope {
                    Scope::Project => store.trust_project(&request.root),
                    _ => store.trust_guide(&request.root, &request.text),
                };
                answer.and_then(|()| store.save(path))
            }
            Err(e) => Err(e),
        },
        None => Err("there is nowhere to keep what you have allowed".to_owned()),
    };
    match written {
        Ok(()) => {
            let what = if scope == Scope::Project { "this project's guides" } else { "that guide" };
            state.status.note(format!("Allowed {what}. Ask your agent again and it will be given."));
        }
        Err(e) => state.status.problem(e),
    }
}

/// **Read the guide for an agent calling `grimoire/agent_guide`**: the allowed layers, the withheld line,
/// and the question when the person has not answered for this content yet.
pub fn serve(state: &mut EditorState, project: Option<&Project>, user: Option<&UserGuideFile>) -> Composed {
    let trust_file = state.trust.file.clone();
    let composed = compose(project, user, trust_file.as_deref());
    if let (Some(text), Some(project)) = (composed.project_text.as_ref(), project) {
        let already_asked = state
            .guide
            .awaiting
            .as_ref()
            .is_some_and(|a| a.root == project.root && a.text == *text);
        // **Not now is an answer**, so the same text is never asked for twice in one session. A changed
        // file is a different digest and is asked about again.
        if !already_asked && !state.guide.denied_at(&project.root, text) {
            state.guide.pending = Some(GuideRequest { root: project.root.clone(), text: text.clone() });
        }
    }
    composed
}

/// **The guide's `layers`, `withheld`, `problems` and `trouble`, as `grimoire/agent_guide` answers.**
pub fn json(composed: &Composed) -> serde_json::Value {
    let problems: Vec<serde_json::Value> = composed.guide.problems.iter().map(Problem::to_json).collect();
    serde_json::json!({
        "layers": composed
            .guide
            .layers
            .iter()
            .map(|layer| serde_json::json!({
                "source": layer.source.word(),
                "path": layer.path.display().to_string(),
                "text": layer.text,
            }))
            .collect::<Vec<_>>(),
        "withheld": composed.withheld,
        "problems": problems,
        "trouble": composed.trouble,
    })
}

/// **The files as they read, with no trust involved**, for a screen where the person is the one deciding.
pub fn read_untrusted(project: Option<&Project>, user: Option<&UserGuideFile>) -> Guide {
    let user_file = user.map(|f| f.0.clone());
    let project_file = project.map(|p| project_file(&p.root));
    agent_guide::read(user_file.as_deref(), project_file.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_projects_guide_is_the_dark_factory_folder_inside_the_project() {
        assert_eq!(project_file(Path::new("/a/project")), PathBuf::from("/a/project/.dark_factory/AGENTS.md"));
    }

    #[test]
    fn a_guide_with_no_project_and_no_file_is_empty_and_says_nothing() {
        let composed = compose(None, None, None);
        assert!(composed.guide.layers.is_empty());
        assert!(composed.guide.problems.is_empty());
        assert!(composed.withheld.is_none());
        assert_eq!(
            json(&composed),
            serde_json::json!({"layers": [], "withheld": null, "problems": [], "trouble": null})
        );
    }

    #[test]
    fn a_long_guide_is_shown_by_its_head_and_its_size() {
        let text = "x".repeat(SHOWN_CHARS + 40);
        let shown = text_preview(&text);
        assert!(shown.contains("40 more characters"), "{shown}");
        assert!(shown.chars().count() < text.chars().count());
    }
}
