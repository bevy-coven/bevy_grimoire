//! **The editor as host of the owner's family crates** (#96).
//!
//! Each family crate is an optional cargo feature of this crate, named after it. A project's
//! `family.ron` (`bevy_grimoire_core::family`) lists the ones it uses. A listed crate this build was
//! compiled without is a problem the author is told about, never a silent no-op.

use bevy::prelude::*;
use bevy_grimoire_core::family::{Family, FamilyWatch, SettingsChange};
use bevy_grimoire_core::live::KindVerdict;
use std::path::PathBuf;

use crate::editor::EditorState;
use crate::project::Project;

/// Every crate this editor can host, whether or not this build has it. Add a name here in the
/// commit that adds the cargo feature.
pub const KNOWN: &[&str] = &["bevy_carnage", "bevy_elements", "bevy_flora", "bevy_gooey", "bevy_lights", "bevy_orca", "bevy_society", "bevy_speech_bubbles", "bevy_stigmergy"];

/// The family crates this build was compiled with: one entry per enabled cargo feature.
pub fn built() -> Vec<&'static str> {
    let mut on = Vec::new();
    if cfg!(feature = "bevy_carnage") {
        on.push("bevy_carnage");
    }
    if cfg!(feature = "bevy_elements") {
        on.push("bevy_elements");
    }
    if cfg!(feature = "bevy_flora") {
        on.push("bevy_flora");
    }
    if cfg!(feature = "bevy_gooey") {
        on.push("bevy_gooey");
    }
    if cfg!(feature = "bevy_lights") {
        on.push("bevy_lights");
    }
    if cfg!(feature = "bevy_orca") {
        on.push("bevy_orca");
    }
    if cfg!(feature = "bevy_society") {
        on.push("bevy_society");
    }
    if cfg!(feature = "bevy_speech_bubbles") {
        on.push("bevy_speech_bubbles");
    }
    if cfg!(feature = "bevy_stigmergy") {
        on.push("bevy_stigmergy");
    }
    on
}

/// **What is wrong with the project's `family.ron` for this build**: an unreadable file, an unknown
/// crate, or a crate whose feature is off. Empty when the project lists nothing or has no file.
pub fn problems_with(root: &std::path::Path, built: &[&str]) -> Vec<String> {
    match Family::load(root) {
        Ok(family) => family.problems(KNOWN, built),
        Err(e) => vec![e],
    }
}

/// Reports the project's family problems once per opened project root, on the status line where the
/// author looks.
pub fn report_problems(
    project: Option<Res<Project>>,
    mut state: ResMut<EditorState>,
    mut told: Local<Option<PathBuf>>,
) {
    let Some(project) = project else { return };
    if told.as_deref() == Some(project.root.as_path()) {
        return;
    }
    *told = Some(project.root.clone());
    for p in problems_with(&project.root, &built()) {
        state.status.problem(p);
    }
}

/// **Hot reload of family settings** (#96): a settings file that changes on disk, from `family_set`
/// or from a person's editor, is read once it settles and reported on the status line. The new text
/// is kept in [`FamilySettings`] for the crate's own plugin to apply; a file that does not parse is a
/// problem and the previous settings stay.
#[derive(Resource, Default, Debug)]
pub struct FamilySettings {
    /// Crate name → the last good settings text.
    pub text: std::collections::BTreeMap<String, String>,
}

/// The set [`reload_settings`] runs in, which a crate's apply system orders after. A set rather than
/// the system itself, because ordering against a system type panics once a schedule holds two of it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Reloaded;

/// **The file a crate's settings are in**, as the verdicts name it: under the game's root, which is
/// where a `family` kind's pattern starts.
fn settings_file(krate: &str) -> String {
    format!("family/{krate}.ron")
}

/// **Follow the settings files, and say what became of each edit** (#96, #200).
///
/// A file that reads replaces the crate's settings; one that is not valid RON is refused and the last
/// good ones stay ([`FamilyWatch`] holds them). Each poll's changes are one decision in
/// [`crate::live::EditorReloads`], kind `family`, so `grimoire_state.reloads` names the file and why,
/// once per edit; a file that is gone is no longer listed. What the crate's own type makes of a file
/// that is valid RON but the wrong shape is not decided here: the type lives in the crate.
pub fn reload_settings(
    project: Option<Res<Project>>,
    mut settings: ResMut<FamilySettings>,
    mut state: ResMut<EditorState>,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut watch: Local<Option<(PathBuf, FamilyWatch, std::time::Instant)>>,
) {
    let Some(project) = project else { return };
    if watch.as_ref().is_none_or(|(root, ..)| *root != project.root) {
        let fresh = FamilyWatch::new(&project.root);
        settings.text = fresh.current().clone();
        if let Some(reloads) = reloads.as_deref_mut() {
            // Another project's files are not this one's.
            reloads.forget(&["family"]);
            if !settings.text.is_empty() {
                reloads.decide(settings.text.keys().map(|k| KindVerdict::applied("family", settings_file(k))).collect());
            }
        }
        *watch = Some((project.root.clone(), fresh, std::time::Instant::now()));
    }
    let Some((_, w, started)) = watch.as_mut() else { return };
    let (mut decided, mut gone) = (Vec::new(), Vec::new());
    for change in w.poll(started.elapsed().as_secs_f64()) {
        match change {
            SettingsChange::Reloaded { krate, text } => {
                state.status.note(format!("{krate} settings reloaded"));
                decided.push(KindVerdict::applied("family", settings_file(&krate)));
                settings.text.insert(krate, text);
            }
            SettingsChange::Invalid { krate, error } => {
                state.status.problem(format!("{krate} settings are not valid RON, so the previous ones stay: {error}"));
                decided.push(KindVerdict::refused("family", settings_file(&krate), None, error));
            }
            SettingsChange::Removed { krate } => {
                state.status.note(format!("{krate} settings removed; its defaults apply"));
                gone.push(settings_file(&krate));
                settings.text.remove(&krate);
            }
        }
    }
    if let Some(reloads) = reloads.as_deref_mut() {
        for file in &gone {
            reloads.forget_file("family", file);
        }
        if !decided.is_empty() {
            reloads.decide(decided);
        }
    }
}
