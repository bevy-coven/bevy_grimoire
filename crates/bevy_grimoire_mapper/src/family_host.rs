//! **The editor as host of the owner's family crates** (#96).
//!
//! Each family crate is an optional cargo feature of this crate, named after it. A project's
//! `family.ron` (`bevy_grimoire_core::family`) lists the ones it uses. A listed crate this build was
//! compiled without is a problem the author is told about, never a silent no-op.

use bevy::prelude::*;
use bevy_grimoire_core::family::{Family, FamilyWatch, SettingsChange};
use bevy_grimoire_core::live::KindVerdict;
#[cfg(any(feature = "bevy_elements", feature = "bevy_gooey"))]
use bevy_grimoire_core::reload_check;
use bevy_grimoire_core::reload_check::Refusal;
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

/// **What the crate's own type makes of a settings file** (#200): for the crates this build has the type
/// of (`bevy_elements`' [`ElementsSettings`](crate::environment::ElementsSettings), `bevy_gooey`'s skin)
/// the file is read as that type with [`reload_check::read`], so a refusal names the file and the field
/// it is wrong at (`family/bevy_elements.ron (clock)`), and the words returned with it say what stays.
/// The editor holds no schema of its own: the type is the crate's. Any other crate's file is held to
/// being RON, which [`FamilyWatch`] has already seen.
fn read_as(krate: &str, text: &str) -> Result<(), (Refusal, &'static str)> {
    #[cfg(feature = "bevy_elements")]
    if krate == crate::environment::CRATE {
        let file = settings_file(krate);
        return reload_check::read::<crate::environment::ElementsSettings>("family", &file, text).map(drop).map_err(|r| (r, "the previous environment stays"));
    }
    #[cfg(feature = "bevy_gooey")]
    if krate == crate::ui_skin::CRATE {
        let file = settings_file(krate);
        return reload_check::read::<bevy_gooey::GooeySkin>("family", &file, text).map(drop).map_err(|r| (r, "the previous skin stays"));
    }
    let _ = (krate, text);
    Ok(())
}

/// How often a refused settings file is compared with the disk, seconds.
const SETTLE_REFUSED_SECS: f64 = 0.5;

/// What [`reload_settings`] holds between frames. Opaque: a system's `Local` has to be nameable by
/// the system, nothing else uses it.
pub struct Held {
    root: PathBuf,
    watch: FamilyWatch,
    started: std::time::Instant,
    /// When refused files were last compared with the disk, in seconds since `started`.
    settled_at: f64,
    /// The text of each refused file as last decided, so the same bytes are decided once: the watch
    /// says a file is invalid again whenever another settings file changes.
    bad: std::collections::BTreeMap<String, String>,
}

/// **The settings files that were not valid RON when the project opened**, with their text and why.
/// [`FamilyWatch`] keeps only the ones that were, and says nothing about the rest until one of them
/// changes, so a file broken before the editor started would go unnamed.
fn unread_at_launch(root: &std::path::Path, good: &std::collections::BTreeMap<String, String>) -> Vec<(String, String, String)> {
    let Ok(entries) = std::fs::read_dir(bevy_grimoire_core::family::settings_dir(root)) else { return Vec::new() };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let (Some(stem), Some(ext), Some(name)) = (path.file_stem(), path.extension(), path.file_name()) else { continue };
        let krate = stem.to_string_lossy().into_owned();
        if ext != "ron" || name.to_string_lossy().starts_with('.') || good.contains_key(&krate) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        if let Err(e) = ron::from_str::<ron::Value>(&text) {
            out.push((krate, text, e.to_string()));
        }
    }
    out.sort();
    out
}

/// **Bring the refused files up to date with the disk.** [`FamilyWatch`] reports a change from the
/// settings in force, so a file the author puts back to exactly those (undoing a typo) is no change
/// to it, and a file that never read and is then deleted was never in force to be removed. Neither
/// would ever leave the verdicts, so a refused file is looked at twice a second: back to the settings
/// in force it is `applied` again, gone it is forgotten, and anything else is the watch's to report.
/// A crate whose type refused a file is the watch's too: it holds the text it took (valid RON) and the
/// settings in force are the last the type read, so the file's next edit is a change to it; the one thing
/// it cannot say is a return to that text after an unreadable edit in between, which is said here.
fn settle_refused(
    root: &std::path::Path,
    good: &std::collections::BTreeMap<String, String>,
    bad: &mut std::collections::BTreeMap<String, String>,
    current: &std::collections::BTreeMap<String, String>,
    state: &mut EditorState,
    reloads: &mut crate::live::EditorReloads,
) {
    let refused: Vec<String> = reloads
        .kinds()
        .iter()
        .filter(|k| k.kind == "family" && k.outcome == bevy_grimoire_core::live::Outcome::Refused)
        .map(|k| k.file.clone())
        .collect();
    let mut decided = Vec::new();
    for file in refused {
        let Some(krate) = file.strip_prefix("family/").and_then(|f| f.strip_suffix(".ron")) else { continue };
        match std::fs::read_to_string(root.join(&file)) {
            Err(_) => {
                bad.remove(krate);
                reloads.forget_file("family", &file);
            }
            // The settings in force are not what the watch holds: the type refused its text.
            Ok(text) if current.get(krate) != good.get(krate) => {
                if current.get(krate) == Some(&text)
                    && bad.get(krate) != Some(&text)
                    && let Err((refusal, stays)) = read_as(krate, &text)
                {
                    state.status.problem(format!("{}; {stays}", refusal.sentence()));
                    decided.push(KindVerdict::refused("family", refusal.file, refusal.field, refusal.reason));
                    bad.insert(krate.to_owned(), text);
                }
            }
            Ok(text) if good.get(krate) == Some(&text) => {
                bad.remove(krate);
                decided.push(KindVerdict::applied("family", file));
            }
            Ok(_) => {}
        }
    }
    if !decided.is_empty() {
        reloads.decide(decided);
    }
}

/// **Follow the settings files, and say what became of each edit** (#96, #200).
///
/// A file that reads replaces the crate's settings; one that is not valid RON is refused and the last
/// good ones stay ([`FamilyWatch`] holds them). A file that is RON but is not the crate's own type
/// (`read_as`: the elements settings, the gooey skin) is refused the same way, naming the file and the
/// field, and never reaches [`FamilySettings`], so what a crate applies is always a text its type reads.
/// Each poll's changes are one decision in [`crate::live::EditorReloads`], kind `family`, so
/// `grimoire_state.reloads` names the file and why, once per edit; a file that is gone is no longer
/// listed, and one that was already broken when the project opened is named from the start.
pub fn reload_settings(
    project: Option<Res<Project>>,
    mut settings: ResMut<FamilySettings>,
    mut state: ResMut<EditorState>,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut held: Local<Option<Held>>,
) {
    let Some(project) = project else { return };
    if held.as_ref().is_none_or(|h| h.root != project.root) {
        let fresh = FamilyWatch::new(&project.root);
        settings.text.clear();
        let mut verdicts = Vec::new();
        let mut bad = std::collections::BTreeMap::new();
        for (krate, text) in fresh.current() {
            match read_as(krate, text) {
                Ok(()) => {
                    settings.text.insert(krate.clone(), text.clone());
                    verdicts.push(KindVerdict::applied("family", settings_file(krate)));
                }
                Err((refusal, _)) => {
                    state.status.problem(format!("{}; its defaults apply", refusal.sentence()));
                    verdicts.push(KindVerdict::refused("family", refusal.file, refusal.field, refusal.reason));
                    bad.insert(krate.clone(), text.clone());
                }
            }
        }
        for (krate, text, error) in unread_at_launch(&project.root, fresh.current()) {
            state.status.problem(format!("{krate} settings are not valid RON, so its defaults apply: {error}"));
            let field = read_as(&krate, &text).err().and_then(|(r, _)| r.field);
            verdicts.push(KindVerdict::refused("family", settings_file(&krate), field, error));
            bad.insert(krate, text);
        }
        if let Some(reloads) = reloads.as_deref_mut() {
            // Another project's files are not this one's.
            reloads.forget(&["family"]);
            if !verdicts.is_empty() {
                reloads.decide(verdicts);
            }
        }
        *held = Some(Held { root: project.root.clone(), watch: fresh, started: std::time::Instant::now(), settled_at: 0.0, bad });
    }
    let Some(held) = held.as_mut() else { return };
    let now = held.started.elapsed().as_secs_f64();
    let (mut decided, mut gone) = (Vec::new(), Vec::new());
    for change in held.watch.poll(now) {
        match change {
            SettingsChange::Reloaded { krate, text } => {
                match read_as(&krate, &text) {
                    Ok(()) => {
                        state.status.note(format!("{krate} settings reloaded"));
                        held.bad.remove(&krate);
                        decided.push(KindVerdict::applied("family", settings_file(&krate)));
                        settings.text.insert(krate, text);
                    }
                    Err((refusal, stays)) => {
                        state.status.problem(format!("{}; {stays}", refusal.sentence()));
                        decided.push(KindVerdict::refused("family", refusal.file, refusal.field, refusal.reason));
                        held.bad.insert(krate.clone(), text);
                    }
                }
            }
            SettingsChange::Invalid { krate, error } => {
                // The same bytes are decided once, however often the watch says so.
                let text = std::fs::read_to_string(bevy_grimoire_core::family::settings_dir(&held.root).join(format!("{krate}.ron"))).ok();
                if text.is_some() && held.bad.get(&krate) == text.as_ref() {
                    continue;
                }
                state.status.problem(format!("{krate} settings are not valid RON, so the previous ones stay: {error}"));
                // Where the crate's own type stopped reading, when this build has it.
                let field = text.as_deref().and_then(|t| read_as(&krate, t).err()).and_then(|(r, _)| r.field);
                decided.push(KindVerdict::refused("family", settings_file(&krate), field, error));
                if let Some(text) = text {
                    held.bad.insert(krate, text);
                }
            }
            SettingsChange::Removed { krate } => {
                state.status.note(format!("{krate} settings removed; its defaults apply"));
                held.bad.remove(&krate);
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
        if now - held.settled_at >= SETTLE_REFUSED_SECS {
            held.settled_at = now;
            settle_refused(&held.root, &settings.text, &mut held.bad, held.watch.current(), &mut state, reloads);
        }
    }
}
