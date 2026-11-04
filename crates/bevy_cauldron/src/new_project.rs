//! **The new-project wizard on the launcher** (UI-12, #218): a name, a place and a template, and a Create that
//! stays disabled for as long as the form says why it cannot.
//!
//! The checks are [`check`], one function over the three fields, so the words under a field, the reason beside
//! Create and the refusal when Create is pressed anyway (a button that is only dimmed can still be activated by a
//! script) are the same sentence. Creating writes `assets/grimoire` through [`crate::catalog::create_project_with`]
//! and then opens the project the way a recent row does ([`crate::launcher::OpenRequest`]).
//!
//! # Templates
//!
//! `Empty` is a project with a vocabulary and no kit. `One empty kit` adds a first kit named after the project
//! ([`crate::catalog::create_kit`]) so the chooser has something to author in. A template that copies a kit of the
//! example project was the issue's first wording; it would copy the meshes the kit names too (90 files, tens of
//! megabytes) into every new project, so it is left until a project can reference the example's meshes in place.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy_gooey::{FieldProblem, TextHint, ValueChange, button, choice, heading, label, observe, panel, text_input};

use crate::launcher::{Launcher, OpenRequest};
use crate::screen::Screen;

/// The vocabulary every new project starts from, byte for byte as the example project keeps it.
pub(crate) const VOCAB: &[u8] = include_bytes!("../../../assets/grimoire/vocab.ron");

/// What a new project starts with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Template {
    #[default]
    Empty,
    OneKit,
}

impl Template {
    pub const ALL: [Template; 2] = [Template::Empty, Template::OneKit];

    pub fn label(self) -> &'static str {
        match self {
            Template::Empty => "Empty",
            Template::OneKit => "One empty kit",
        }
    }
}

/// The form's three fields.
#[derive(Clone, Debug, PartialEq)]
pub struct NewProject {
    pub name: String,
    /// The folder the project's own folder is made in.
    pub location: String,
    pub template: Template,
}

impl Default for NewProject {
    fn default() -> Self {
        let place = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."));
        NewProject { name: String::new(), location: place.to_string_lossy().into_owned(), template: Template::Empty }
    }
}

/// What is wrong with the form, by field. Both `None` is a form that can be created.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Problems {
    pub name: Option<String>,
    pub location: Option<String>,
    /// Said beside Create when the form is empty and nothing is wrong yet, so a disabled button always has its reason.
    pub missing: Option<String>,
}

impl Problems {
    pub fn none(&self) -> bool {
        self.name.is_none() && self.location.is_none() && self.missing.is_none()
    }

    /// The reason Create is disabled, in one sentence.
    pub fn reason(&self) -> Option<&str> {
        self.name.as_deref().or(self.location.as_deref()).or(self.missing.as_deref())
    }
}

/// The project's folder name: the name as the project's own snake_case.
fn folder_name(name: &str) -> String {
    bevy_grimoire_core::naming::to_snake_case(name)
}

/// Checks the form. An empty name is not an error on the field (nothing has been typed yet) but still keeps
/// Create disabled, with its reason.
pub fn check(form: &NewProject) -> Problems {
    let mut out = Problems::default();
    let location = Path::new(form.location.trim());
    if form.location.trim().is_empty() {
        out.location = Some("Say which folder the project goes in.".to_owned());
    } else if !location.is_dir() {
        out.location = Some(format!("{} is not a folder.", location.display()));
    } else if std::fs::metadata(location).is_ok_and(|m| m.permissions().readonly()) {
        out.location = Some(format!("{} cannot be written to.", location.display()));
    }
    if form.name.trim().is_empty() {
        out.missing = Some("Type a name for the project.".to_owned());
    } else {
        let folder = folder_name(&form.name);
        if folder.is_empty() {
            out.name = Some(format!(
                "`{}` leaves nothing usable as a name. Use lowercase letters, digits and single underscores, starting with a letter.",
                form.name.trim()
            ));
        } else if out.location.is_none() && location.join(&folder).exists() {
            out.name = Some(format!(
                "`{folder}` already exists in {}. Pick another name, or open the one that is there.",
                location.display()
            ));
        }
    }
    out
}

/// Makes the project the form describes and returns its folder. Refuses, with [`check`]'s sentence, a form that
/// is not ready.
pub fn create(form: &NewProject) -> Result<PathBuf, String> {
    let problems = check(form);
    if let Some(reason) = problems.reason() {
        return Err(reason.to_owned());
    }
    let dir = crate::catalog::create_project_with(Path::new(form.location.trim()), &form.name, VOCAB)?;
    if form.template == Template::OneKit
        && let Err(e) = crate::catalog::create_kit(&dir, &folder_name(&form.name))
    {
        return Err(format!("{} was made, but its first kit was not: {e}", dir.display()));
    }
    Ok(dir)
}

/// The form on screen, while there is one.
#[derive(Resource, Debug, Default)]
pub struct NewProjectForm(pub NewProject);

/// The card the form is drawn on.
#[derive(Component)]
pub struct FormCard;

/// The Name field.
#[derive(Component)]
pub struct NameField;

/// The Location field.
#[derive(Component)]
pub struct LocationField;

/// The Template choice.
#[derive(Component)]
pub struct TemplateChoice;

/// The Create button.
#[derive(Component)]
pub struct CreateButton;

/// The line that says why Create is disabled.
#[derive(Component)]
pub struct ReasonLine;

pub struct NewProjectPlugin;

impl Plugin for NewProjectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (open_the_form, spawn_the_card, show_the_form, close_the_form, swap_the_cards).chain().run_if(in_state(Screen::Launcher)),
        )
        .add_systems(OnExit(Screen::Launcher), |mut commands: Commands| commands.remove_resource::<NewProjectForm>());
    }
}

/// The launcher's "New project…" button asks for the form by setting [`Launcher::new_project_asked`]; the form is a
/// resource so that the screen's own state (`Launcher`) stays about the list.
fn open_the_form(mut commands: Commands, mut launcher: ResMut<Launcher>, form: Option<Res<NewProjectForm>>) {
    if launcher.new_project_asked && form.is_none() {
        commands.insert_resource(NewProjectForm::default());
    }
    launcher.new_project_asked = false;
}

fn spawn_the_card(
    mut commands: Commands,
    form: Option<Res<NewProjectForm>>,
    cards: Query<(), With<FormCard>>,
    root: Query<Entity, With<crate::launcher::HomeContent>>,
) {
    let Some(form) = form else { return };
    if !cards.is_empty() {
        return;
    }
    let Ok(root) = root.single() else { return };
    let start = form.0.clone();
    commands.entity(root).with_children(|root| {
        root.spawn((panel(), FormCard))
            .insert(Node { flex_direction: FlexDirection::Column, width: Val::Px(crate::launcher::CARD_WIDTH), max_width: Val::Percent(100.0), ..default() })
            .with_children(|card| {
            card.spawn(heading("New project"));
            card.spawn((
                text_input("Name", start.name.clone()),
                NameField,
                TextHint("project name".into()),
                observe(|change: On<ValueChange<String>>, mut form: ResMut<NewProjectForm>| form.0.name = change.value.clone()),
            ));
            card.spawn((
                text_input("Location", start.location.clone()),
                LocationField,
                observe(|change: On<ValueChange<String>>, mut form: ResMut<NewProjectForm>| form.0.location = change.value.clone()),
            ));
            card.spawn((
                choice("Template", Template::ALL.map(Template::label), 0),
                TemplateChoice,
                observe(|change: On<ValueChange<usize>>, mut form: ResMut<NewProjectForm>| {
                    form.0.template = Template::ALL.get(change.value).copied().unwrap_or_default();
                }),
            ));
            card.spawn((label(""), ReasonLine));
            card.spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(crate::chrome::GAP_GROUP), ..default() }).with_children(|row| {
                row.spawn((
                    button("Create"),
                    CreateButton,
                    observe(|_: On<bevy_gooey::Activate>, form: Res<NewProjectForm>, mut launcher: ResMut<Launcher>, mut out: MessageWriter<OpenRequest>| {
                        // Checked again here: a dimmed button can still be activated by a script.
                        match create(&form.0) {
                            Ok(dir) => {
                                launcher.problem = None;
                                launcher.new_project_done = true;
                                out.write(OpenRequest(dir));
                            }
                            Err(e) => launcher.problem = Some(e),
                        }
                    }),
                ));
                row.spawn((
                    button("Cancel"),
                    observe(|_: On<bevy_gooey::Activate>, mut launcher: ResMut<Launcher>| launcher.new_project_done = true),
                ));
            });
        });
    });
}

/// Puts [`check`]'s words on the fields and on Create, every time the form changes.
fn show_the_form(
    mut commands: Commands,
    form: Option<Res<NewProjectForm>>,
    name: Query<Entity, With<NameField>>,
    location: Query<Entity, With<LocationField>>,
    create: Query<Entity, With<CreateButton>>,
    mut reason: Query<&mut Text, With<ReasonLine>>,
) {
    let Some(form) = form.filter(|f| f.is_changed()) else { return };
    let problems = check(&form.0);
    let mark = |commands: &mut Commands, entity: Entity, problem: &Option<String>| match problem {
        Some(text) => {
            commands.entity(entity).insert(FieldProblem::error(text.clone()));
        }
        None => {
            commands.entity(entity).remove::<FieldProblem>();
        }
    };
    for e in &name {
        mark(&mut commands, e, &problems.name);
    }
    for e in &location {
        mark(&mut commands, e, &problems.location);
    }
    for e in &create {
        if problems.none() {
            commands.entity(e).remove::<InteractionDisabled>();
        } else {
            commands.entity(e).insert(InteractionDisabled);
        }
    }
    let said = problems.reason().unwrap_or_default().to_owned();
    for mut text in &mut reason {
        if text.0 != said {
            text.0 = said.clone();
        }
    }
}

/// Takes the card down when Create or Cancel was pressed.
fn close_the_form(mut commands: Commands, mut launcher: ResMut<Launcher>, cards: Query<Entity, With<FormCard>>) {
    if !launcher.new_project_done {
        return;
    }
    launcher.new_project_done = false;
    for card in &cards {
        commands.entity(card).despawn();
    }
    commands.remove_resource::<NewProjectForm>();
}

/// The launcher's list gives its place to the form while the form is open, so neither is cut off by the window.
fn swap_the_cards(form: Option<Res<NewProjectForm>>, mut cards: Query<&mut Node, With<crate::launcher::LauncherCard>>) {
    let want = if form.is_some() { Display::None } else { Display::Flex };
    for mut node in &mut cards {
        if node.display != want {
            node.display = want;
        }
    }
}
