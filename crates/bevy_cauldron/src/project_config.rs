//! **The project configuration screen: which family crates the project uses, with the verdict of the
//! file that says so** (UI-74, #252).
//!
//! Opened from the settings screen's *Project configuration* button (and by [`OpenProjectConfig`]) in a
//! project that is open. It lists every crate the editor can host (`family_host::KNOWN`), one switch each,
//! on while the project's `family.ron` names it. A switch rewrites `family.ron` atomically with the crate
//! added or taken out, every other crate in the order the file has them, and then reads the file back
//! through the reload contract's own check (`reload_check::check`, kind `family_list`) and records the
//! verdict where `grimoire_state.reloads` and the reload panel read it. The screen shows that verdict
//! under the switches: *applied*, or the refusal with the file and the field.
//!
//! # Validation, in the words of `Family::problems`
//!
//! A crate the project lists that this build was compiled without is a problem the file can carry (the
//! game may use it; the editor cannot host its panel), so it is shown on the crate's own line, in the
//! words the status line uses (`family_host::problems_with`), and the switch still works. A `family.ron`
//! that does not read is shown and **never written over**: the switches wait until it reads.
//!
//! The kits the project binds are listed under the switches, the one being authored marked; editing a kit
//! and the policy's exclusions go through their operations (`kit_create`, `kit_set_authoring`,
//! `pack_set_excluded`) and are not on this screen yet.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_grimoire_core::family::{self, Family};
use bevy_grimoire_core::live::KindVerdict;
use bevy_grimoire_core::reload_check::{self, Check};
use bevy_gooey::{Activate, Cancel, button, dialog, heading, label, menu, observe, panel, toggle, ValueChange};

use crate::family_host;
use crate::live::EditorReloads;
use crate::project::Project;

/// The reload-contract kind `family.ron` is.
pub const KIND: &str = "family_list";

/// The file's name as a verdict names it.
pub const FILE: &str = "family.ron";

/// Ask for the project configuration screen.
#[derive(Message, Clone, Copy, Debug, Default)]
pub struct OpenProjectConfig;

/// The open screen, and how many writes it has made (so it is built again at once after one).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct ProjectConfigDialog {
    pub writes: u32,
    /// A write the screen could not make.
    pub problem: Option<String>,
}

/// What the lines were last built from.
#[derive(Component, Default)]
struct Built(Option<Snapshot>);

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    dialog: ProjectConfigDialog,
    family: Result<Vec<String>, String>,
    verdict: Option<String>,
}

/// The column the switches, the verdict and the kits are built into.
#[derive(Component)]
struct Lines;

#[derive(SystemParam)]
struct Writer<'w, 's> {
    dialog: Query<'w, 's, &'static mut ProjectConfigDialog>,
    project: Option<Res<'w, Project>>,
    reloads: Option<ResMut<'w, EditorReloads>>,
}

impl Writer<'_, '_> {
    /// Lists `krate` in `family.ron` when `on`, takes it out otherwise, and records what the reload contract
    /// makes of the file it wrote.
    fn set(&mut self, krate: &str, on: bool) {
        let Some(project) = self.project.as_deref() else { return };
        let outcome = write_family(&project.root, krate, on);
        if let Some(reloads) = self.reloads.as_deref_mut() {
            reloads.decide(vec![verdict_of(&project.root)]);
        }
        if let Ok(mut dialog) = self.dialog.single_mut() {
            dialog.problem = outcome.err();
            dialog.writes += 1;
        }
    }
}

/// **Rewrites `<root>/family.ron` with `krate` listed or not**, atomically; a file that does not read is
/// refused and left as it is. Other crates keep the file's order and a crate is never listed twice.
pub fn write_family(root: &std::path::Path, krate: &str, on: bool) -> Result<(), String> {
    let mut family = Family::load(root).map_err(|e| format!("{e}; not changed"))?;
    let listed = family.crates.iter().any(|c| c == krate);
    match (on, listed) {
        (true, false) => family.crates.push(krate.to_owned()),
        (false, true) => family.crates.retain(|c| c != krate),
        _ => return Ok(()),
    }
    let text = ron::ser::to_string_pretty(&family, ron::ser::PrettyConfig::default()).map_err(|e| e.to_string())?;
    bevy_grimoire_core::atomic_file::write(&family::path_for_project(root), text).map_err(|e| format!("{FILE}: {e}"))
}

/// **The reload contract's verdict on the project's `family.ron`** as it is on disk now: applied when it
/// reads, the refusal (file and field) when it does not.
pub fn verdict_of(root: &std::path::Path) -> KindVerdict {
    let path = family::path_for_project(root);
    let Some(kind) = bevy_grimoire_core::reload::kind_by_id(KIND) else {
        return KindVerdict::refused(KIND, FILE, None, "the reload contract has no family_list kind");
    };
    match std::fs::read_to_string(&path) {
        // No file is the empty list: a project that uses no family crate.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => KindVerdict::applied(KIND, FILE),
        Err(e) => KindVerdict::refused(KIND, FILE, None, e.to_string()),
        Ok(text) => match reload_check::check(kind, FILE, &text) {
            Check::Refused(r) => KindVerdict::refused(KIND, FILE, r.field, r.reason),
            _ => KindVerdict::applied(KIND, FILE),
        },
    }
}

pub struct ProjectConfigPlugin;

impl Plugin for ProjectConfigPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<OpenProjectConfig>().add_systems(Update, (open, refresh).chain());
    }
}

fn open(
    mut asks: MessageReader<OpenProjectConfig>,
    existing: Query<(), With<ProjectConfigDialog>>,
    project: Option<Res<Project>>,
    mut commands: Commands,
) {
    if asks.read().count() == 0 || !existing.is_empty() || project.is_none() {
        return;
    }
    commands
        .spawn((dialog(), Name::new("project configuration"), ProjectConfigDialog::default(), Built::default()))
        .observe(|cancel: On<Cancel>, mut commands: Commands| {
            commands.entity(cancel.entity).despawn();
        })
        .with_children(|dialog| {
            dialog
                .spawn(panel())
                .insert(Node { flex_direction: FlexDirection::Column, width: Val::Px(crate::launcher::CARD_WIDTH), max_width: Val::Percent(100.0), ..default() })
                .with_children(|card| {
                    card.spawn(heading("Project configuration"));
                    card.spawn((Node { flex_direction: FlexDirection::Column, ..default() }, Lines));
                    card.spawn(menu()).with_children(|actions| {
                        actions.spawn((
                            button("Close"),
                            observe(|_: On<Activate>, dialogs: Query<Entity, With<ProjectConfigDialog>>, mut commands: Commands| {
                                for dialog in &dialogs {
                                    commands.entity(dialog).despawn();
                                }
                            }),
                        ));
                    });
                });
        });
}

fn refresh(
    project: Option<Res<Project>>,
    reloads: Option<Res<EditorReloads>>,
    mut dialogs: Query<(Ref<ProjectConfigDialog>, &mut Built)>,
    lines: Query<Entity, With<Lines>>,
    mut commands: Commands,
) {
    let (Ok((dialog, mut built)), Some(project)) = (dialogs.single_mut(), project) else { return };
    let family = Family::load(&project.root).map(|f| f.crates);
    let verdict = reloads
        .as_deref()
        .and_then(|r| r.kinds().iter().find(|k| k.kind == KIND && k.file == FILE))
        .map(|v| match v.outcome {
            bevy_grimoire_core::live::Outcome::Applied => v.outcome.word().to_owned(),
            _ => format!("{}: {}", v.outcome.word(), v.sentence()),
        });
    let snapshot = Snapshot { dialog: (*dialog).clone(), family: family.clone(), verdict: verdict.clone() };
    if built.0.as_ref() == Some(&snapshot) {
        return;
    }
    let Ok(column) = lines.single() else { return };
    commands.entity(column).despawn_children();
    let built_in = family_host::built();
    let problems = family_host::problems_with(&project.root, &built_in);
    commands.entity(column).with_children(|lines| {
        lines.spawn(heading("Family crates"));
        lines.spawn(label("A project lists the crates it uses in family.ron; a switch adds or takes one out."));
        match &family {
            Err(why) => {
                lines.spawn(label(format!("{why}. Fix the file by hand: nothing is written until it reads.")));
            }
            Ok(listed) => {
                for krate in family_host::KNOWN {
                    let on = listed.iter().any(|c| c == krate);
                    let name = (*krate).to_owned();
                    lines.spawn((
                        toggle(*krate, on),
                        observe(move |c: On<ValueChange<bool>>, mut w: Writer| w.set(&name, c.value)),
                    ));
                    if let Some(problem) = problems.iter().find(|p| p.contains(krate)) {
                        lines.spawn(label(problem.clone()));
                    } else if !built_in.contains(krate) {
                        lines.spawn(label(format!("{krate} is not built into this editor; rebuild with --features {krate} to host it.")));
                    }
                }
            }
        }
        lines.spawn(label(match &verdict {
            Some(said) => format!("family.ron: {said}"),
            None => "family.ron: not changed from here yet".to_owned(),
        }));
        if let Some(problem) = &dialog.problem {
            lines.spawn(label(problem.clone()));
        }
        lines.spawn(heading("Kits"));
        for kit in project.kits.iter() {
            let dir = kit.dir.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            let mark = if kit.dir == project.grimoire_dir { " (authoring)" } else { "" };
            lines.spawn(label(format!("{dir}: provides {}{mark}", kit.namespace)));
        }
    });
    built.0 = Some(snapshot);
}
