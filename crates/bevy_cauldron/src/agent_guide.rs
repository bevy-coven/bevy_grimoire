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
//! when the person has allowed **that exact content** (`trust::TextGrant`, the SHA-256 of the bytes) or
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
use bevy_grimoire_core::live::KindVerdict;
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

/// **The path the page shows**, project-relative: what a person types, and what `reload` classifies.
pub const PROJECT_PATH_SHOWN: &str = bevy_grimoire_core::agent_guide::PROJECT_PATH;

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

/// **The guide as the person has allowed it.** `trust_file` is the store's **override**: `None` is the
/// person's own `<data_dir>/Grimoire/trusted.yaml`, resolved by [`crate::trusted::store_path`], which is what
/// a real editor always has. A test names a file of its own so nothing reads or writes the real one.
pub fn compose(project: Option<&Project>, user: Option<&UserGuideFile>, store_root: Option<&crate::trusted::StoreRoot>, trust_file: Option<&Path>) -> Composed {
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
    // **`None` is the person's own store**, not "no store": `store_path` resolves it through the user's
    // data directory, and that is where a real editor keeps every grant. A `None` read as "nothing can be
    // allowed" would withhold every project's guide in the shipped app while every test passed.
    let decision = match crate::trusted::load_at(store_root, trust_file) {
        Ok(store) => match store.guide_decision(&project.root, &text) {
            Ok(decision) => decision,
            Err(e) => {
                composed.trouble = Some(e);
                composed.withheld = Some(withheld);
                return composed;
            }
        },
        Err(e) => {
            composed.trouble = Some(e);
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

/// **What the project page's Agent guide section says about the project's guide file.** Named for what it
/// describes: the layer at `PROJECT_PATH`, not the person's own file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuideState {
    /// The content is granted, or the project is trusted.
    #[default]
    Allowed,
    /// The content is not allowed: the page offers Allow this guide and Trust this project.
    Waiting,
    /// There is no file: the page offers Create guide.
    Missing,
}

impl GuideState {
    /// The word the status line shows, and the word the tests read.
    pub fn word(self) -> &'static str {
        match self {
            GuideState::Allowed => "allowed",
            GuideState::Waiting => "waiting for you",
            GuideState::Missing => "missing",
        }
    }
}

/// **A text waiting for a person's answer**: the project's guide, or one of its style files (#401). They share
/// one question, `Asked::TrustGuide`, and one store, as they share one grant type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuideRequest {
    pub root: PathBuf,
    /// The file the text is, relative to the project root: [`PROJECT_PATH_SHOWN`] for the guide, `design/style.md`
    /// for a style file. The grant is for this path and this content.
    pub path: String,
    pub text: String,
    /// **Whether the question offered "Allow this guide".** A guide too long to show is raised as a *note*,
    /// which has no affirmative at all — not the text (unread) and not `Scope::Project`, whose list is the
    /// one `TrustStore::decision` reads for `agent.yaml` and `play.yaml`. So a `Yes` can never reach a grant
    /// about a file the question did not show.
    pub offer_allow: bool,
}

/// **Where the guide's question stands**, kept on the editor's state because the answer arrives in a
/// system rather than in the call that raised it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuideTrust {
    /// **Raised by a `grimoire/agent_guide` or `grimoire/style` call that withheld a text; the frames after it ask
    /// them one at a time**, in the order they were raised. A style chain can withhold a file for each place.
    pub pending: Vec<GuideRequest>,
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

/// **How many characters wide the question's card shows a guide.** The card is a label that wraps at the
/// dialog's width; this is that width in characters, conservatively (the body text is proportional, and a
/// narrow guess under-counts only by making the bound stricter).
pub const SHOWN_COLUMNS: usize = 80;

/// **How many visual rows the question can show before the text runs off the card.**
///
/// The card has no height limit and no scroll, so a guide of forty short lines is a hundred and sixty
/// characters — under `SHOWN_CHARS` — with its last line thirty-nine rows below the fold, and `Y` would have
/// been the answer to it. A bound in rows is the one that matches the box.
pub const SHOWN_LINES: usize = 24;

fn text_preview(text: &str) -> String {
    let total = text.chars().count();
    if total <= SHOWN_CHARS {
        return text.trim_end().to_owned();
    }
    let head: String = text.chars().take(SHOWN_CHARS).collect();
    format!("{}\n\n… and {} more characters.", head.trim_end(), total - SHOWN_CHARS)
}

/// **Whether the question can show all of `text`.**
///
/// **Two bounds, and the rows are the one that matters.** `SHOWN_CHARS` counts characters; the card has no
/// height limit and no scroll, so forty short lines are a hundred and sixty characters with the last line
/// thirty-nine rows below the fold — and a character-only check would have offered **Allow this guide**
/// over it. A guide that fits neither bound is raised as a note, which has no affirmative at all.
pub fn fully_shown(text: &str) -> bool {
    let trimmed = text.trim_end();
    trimmed.chars().count() <= SHOWN_CHARS
        && bevy_grimoire_core::agent_guide::visual_rows(trimmed, SHOWN_COLUMNS) <= SHOWN_LINES
}

/// **What the question calls the text**: the project's guide, or a style file by its path.
fn what_it_is(request: &GuideRequest) -> String {
    if request.path == PROJECT_PATH_SHOWN { "guide".to_owned() } else { format!("style file `{}`", request.path) }
}

/// Raises the question a withheld text left, once no other question is up.
fn ask_the_person(state: Option<ResMut<EditorState>>, mut confirm: ResMut<Confirm>) {
    let Some(mut state) = state else { return };
    if state.guide.awaiting.is_some() || confirm.is_open() || state.guide.pending.is_empty() {
        return;
    }
    let request = state.guide.pending.remove(0);
    let what = what_it_is(&request);
    // **The text is shown, because the text is what is being allowed.** A question that asked "allow this
    // project's guide?" over a filename would be asking a person to trust a file they have not read.
    let body = format!("{}\n\n---\n{}", request.root.join(&request.path).display(), text_preview(&request.text));
    // **A guide too long to show is a note, with no grant at all.**
    //
    // The grants a question could offer are the text's (which the person has not been shown) and
    // `Scope::Project` — and `Scope::Project` is **the same list `TrustStore::decision` reads for
    // `agent.yaml` and `play.yaml`**, so a padded guide plus a habitual `Y` would trust the project's program
    // launches, which is a decision about a program nobody made. So there is no affirmative answer to give:
    // a note says what the file is and where to read it, and its one answer dismisses it.
    if !fully_shown(&request.text) {
        let mut request = request;
        request.offer_allow = false;
        confirm.note(
            Asked::TrustGuide,
            format!("This project's {what} is too long to show here"),
            format!(
                "{}\n\nIt is {} lines and {} characters, and this question can show {SHOWN_LINES} lines and \
                 {SHOWN_CHARS} characters. Read it in your own editor, at the path above, and shorten it to \
                 fit: a text too long to show cannot be allowed from here, and nothing has been granted.",
                request.root.join(&request.path).display(),
                bevy_grimoire_core::agent_guide::visual_rows(&request.text, SHOWN_COLUMNS),
                request.text.chars().count()
            ),
            "Close",
        );
        state.guide.awaiting = Some(request);
        return;
    }
    let mut request = request;
    request.offer_allow = true;
    confirm.ask_three(
        Asked::TrustGuide,
        format!("Give this project's {what} to your agent?"),
        body,
        "Allow this guide",
        "Trust this project",
        KeyCode::KeyP,
        "Not now",
    );
    state.guide.awaiting = Some(request);
}

/// Writes the person's answer down and says what happened.
fn take_the_answer(
    state: Option<ResMut<EditorState>>,
    mut confirm: ResMut<Confirm>,
    root: Option<Res<crate::trusted::StoreRoot>>,
) {
    let store_root = root.as_deref();
    let Some(mut state) = state else { return };
    let Some(choice) = confirm.answer_choice(Asked::TrustGuide) else { return };
    let Some(request) = state.guide.awaiting.take() else { return };
    // **A note grants nothing.** Its answer is a dismissal and its content is held for the session, the
    // same as Not now: there is no affirmative on a note at all, and nothing here reads one.
    if choice == Choice::No || !request.offer_allow {
        state.guide.denied.insert((request.root.clone(), digest(&request.text)));
        let what = what_it_is(&request);
        let again = if request.path == PROJECT_PATH_SHOWN { "You can allow it on the project page." } else { "Ask your agent again to be asked again." };
        state.status.note(format!("Not allowed, so this project's {what} was not given to your agent. {again}"));
        return;
    }
    // **The middle answer is Trust this project**; only a question that offered Allow can be answered with
    // the text, and [`ask_the_person`] is the one place that decides which question was asked.
    let scope = if choice == Choice::Alt { Scope::Project } else { Scope::Command };
    // **The person's own store when none is named** (`store_path`), so the answer is written where the
    // next `grimoire/agent_guide` looks for it.
    let written = crate::trusted::resolve(store_root, state.trust.file.as_deref())
        .and_then(|path| {
            let mut store = TrustStore::load(&path)?;
            match scope {
                Scope::Project => store.trust_project(&request.root),
                _ => store.trust_text(&request.root, &request.path, &request.text),
            }
            .and_then(|()| store.save(&path))
        });
    match written {
        Ok(()) => {
            let what = if scope == Scope::Project { "this project's guides and style".to_owned() } else { format!("that {}", what_it_is(&request)) };
            state.status.note(format!("Allowed {what}. Ask your agent again and it will be given."));
            // **Trusting the project answers every question about it**, so a style chain that withheld a file for
            // each place does not ask about the rest.
            if scope == Scope::Project {
                state.guide.pending.retain(|other| other.root != request.root);
            }
        }
        Err(e) => state.status.problem(e),
    }
}

/// **Read the guide for an agent calling `grimoire/agent_guide`**: the allowed layers, the withheld line,
/// and the question when the person has not answered for this content yet.
pub fn serve(
    state: &mut EditorState,
    project: Option<&Project>,
    user: Option<&UserGuideFile>,
    store_root: Option<&crate::trusted::StoreRoot>,
) -> Composed {
    let trust_file = state.trust.file.clone();
    let composed = compose(project, user, store_root, trust_file.as_deref());
    if let (Some(text), Some(project)) = (composed.project_text.as_ref(), project) {
        let already_asked = state
            .guide
            .awaiting
            .as_ref()
            .is_some_and(|a| a.root == project.root && a.text == *text);
        // **Not now is an answer**, so the same text is never asked for twice in one session. A changed
        // file is a different digest and is asked about again.
        if !already_asked {
            raise(state, &project.root, PROJECT_PATH_SHOWN, text);
        }
    }
    composed
}

/// **Raise the question about one withheld text**, unless it is already raised, being asked, or answered "not now" in
/// this session. `path` is the file relative to the project root.
pub fn raise(state: &mut EditorState, root: &Path, path: &str, text: &str) {
    let same = |r: &GuideRequest| r.root == root && r.path == path && r.text == text;
    if state.guide.denied_at(root, text) || state.guide.awaiting.as_ref().is_some_and(same) || state.guide.pending.iter().any(same) {
        return;
    }
    state.guide.pending.push(GuideRequest { root: root.to_path_buf(), path: path.to_owned(), text: text.to_owned(), offer_allow: true });
}

/// **The style of a place, as `grimoire/style` serves it** (SG-02, #401): the chain of the design tree's files with
/// every file the person has not allowed withheld, and the question raised for each.
///
/// The same store as the guide's (`trusted::load_at`), the same grants (`TextGrant`) and the same question
/// (`Asked::TrustGuide`). **A store that will not read withholds every file** and is reported in `trouble`, never
/// read as "nothing is trusted" or as "everything is".
pub fn serve_style(
    state: &mut EditorState,
    project: &Project,
    store_root: Option<&crate::trusted::StoreRoot>,
    request: &bevy_grimoire_core::style::Request,
) -> Result<(bevy_grimoire_core::style::Answer, Vec<(String, String)>), String> {
    let tree = bevy_grimoire_core::design::Tree::read(&project.root);
    let store = crate::trusted::load_at(store_root, state.trust.file.as_deref());
    let root = project.root.clone();
    // **The text that was checked is the text that is noted as served** (SG-09, #408 review): a page is recorded from the very string the trust decision was made
    // about, never read again from disk, so a write between the answer and the record cannot note text nobody was served.
    let mut checked: Vec<(String, String)> = Vec::new();
    let answer = bevy_grimoire_core::style::compose(&tree, request, &mut |path, text| {
        let decision = match &store {
            Ok(store) => store.text_decision(&root, path, text),
            Err(e) => Err(e.clone()),
        };
        if matches!(decision, Ok(bevy_grimoire_core::trust::Decision::Trusted(_))) {
            checked.push((path.to_owned(), text.to_owned()));
        }
        decision
    })?;
    for withheld in &answer.withheld {
        raise(state, &project.root, &withheld.path, &withheld.text);
    }
    let served = crate::style_followed::served_pages(&answer.files, &checked);
    Ok((answer, served))
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
                // **The heading, rendered once** (`bevy_grimoire_core::agent_guide::Layer::rendered`), so
                // the layer an agent reads has one spelling of `## This project's guide (.dark_factory/AGENTS.md)`
                // wherever it is added up: a second renderer in the server would be a second spelling.
                "rendered": layer.rendered(),
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

/// **What [`sync_guide_reload`] saw last**: the file and its stamp (modification time and size). Held in a
/// `Local`, so the editor gains no resource for a file it only watches.
#[derive(Default)]
pub struct GuideLooked {
    path: PathBuf,
    stamp: Option<(std::time::SystemTime, u64)>,
}

/// **Say what became of the project's guide each time it is edited** (#347): the registry's `guide` kind,
/// as `grimoire_state.reloads` lists it. The guide is read on every `grimoire/agent_guide` call, so an
/// edit is already live; this is the person's side of that, a refusal they can read before an agent asks.
///
/// A file that reads is `applied`; one the guide refuses (not UTF-8, over [`agent_guide::GUIDE_MAX_BYTES`],
/// not a regular file) is `refused` with the reason the problem carries, and an absent file has no verdict.
/// The file is read again only when its path, modification time or size moves, so one edit is one decision.
pub fn sync_guide_reload(
    project: Option<Res<Project>>,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut looked: Local<GuideLooked>,
) {
    let Some(project) = project else { return };
    let path = project_file(&project.root);
    let stamp = std::fs::metadata(&path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
    if looked.path == path && looked.stamp == stamp {
        return;
    }
    looked.path = path.clone();
    looked.stamp = stamp;
    let Some(reloads) = reloads.as_deref_mut() else { return };
    // Another project's verdict, or the verdict of a file since deleted, is not this file's.
    reloads.forget(&["guide"]);
    let verdict = match agent_guide::read_layer(Source::Project, &path) {
        Ok(None) => return,
        Ok(Some(_)) => KindVerdict::applied("guide", PROJECT_PATH_SHOWN),
        Err(problem) => KindVerdict::refused("guide", PROJECT_PATH_SHOWN, None, problem.why.clone().unwrap_or(problem.what)),
    };
    reloads.decide(vec![verdict]);
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
        let composed = compose(None, None, None, None);
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
