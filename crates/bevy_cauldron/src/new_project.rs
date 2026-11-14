//! **The new-project wizard on the Projects home** (UI-12, #218; #297): a name, a place and a template, and a Create that
//! stays disabled for as long as the form says why it cannot.
//!
//! The checks are [`check`], one function over the three fields, so the words under a field, the reason beside
//! Create and the refusal when Create is pressed anyway (a button that is only dimmed can still be activated by a
//! script) are the same sentence. Creating writes `assets/grimoire` through [`crate::catalog::create_project_with`],
//! makes the template's kit and its first map, and opens that map the way the example is opened
//! ([`crate::launcher::OpenMapRequest`]): a new project lands in the editor with something to place.
//!
//! # Templates
//!
//! * **Starter** copies the Prototype Kit (CC0, with its licence) out of the example project into the new project,
//!   binds one kit, `prototype`, as the authoring kit with the kit's rows, and makes `first_map`.
//! * **Empty** is one kit, `main`, bound as the authoring kit with no pieces, and `first_map`; the editor's palette
//!   then offers Add meshes… ([`crate::add_meshes`]).
//!
//! # The place
//!
//! The Location starts at `Cauldron Projects` in the person's documents folder, made when the first project is
//! created there. It is never filled from anything else, and Choose… asks the system's own chooser.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::input_focus::AutoFocus;
use bevy_gooey::{Cancel, FieldProblem, TextHint, TextInputValue, ValueChange, button, choice, dialog, heading, label, observe, text_input};

use crate::launcher::{Launcher, OpenMapRequest};
use crate::picker::{PickFor, PickRequest, Picked};
use crate::screen::Screen;
use crate::ui::submit_button;

/// The vocabulary every new project starts from, byte for byte as the example project keeps it.
pub(crate) const VOCAB: &[u8] = include_bytes!("../../../assets/grimoire/vocab.yaml");

/// What a new project starts with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Template {
    #[default]
    Starter,
    Empty,
}

/// The kit a Starter project binds, and the one an Empty project binds.
const STARTER_KIT: &str = "prototype";
const EMPTY_KIT: &str = "main";

/// The map every new project opens on, and its size in metres: width, depth, height.
pub const FIRST_MAP: &str = "first_map";
const FIRST_MAP_METRES: [f32; 3] = [10.0, 10.0, 4.0];

/// Where the Prototype Kit's meshes are in the example project (and in the new one, which keeps the path so each row's
/// `mesh` names the same file), and the file holding the kit's rows.
const PROTOTYPE_MESHES: &str = "assets/kenney_prototype-kit";
const PROTOTYPE_ROWS: &str = "assets/grimoire/library_kenney_from_manifests.yaml";

/// What a Starter kit's library says about itself, in place of the example's "generated" note.
const PROTOTYPE_NOTE: &str = "Kenney's Prototype Kit (CC0, its licence is beside the meshes). Add your own meshes from the library; they land in this kit.";

/// What the folder line says before a name is typed, so the form does not jump when the first letter makes it appear.
const FOLDER_UNNAMED: &str = "Folder: named after the project";

/// The folder, inside the person's documents, that new projects go in by default.
const DEFAULT_PLACE: &str = "Cauldron Projects";

impl Template {
    pub const ALL: [Template; 2] = [Template::Starter, Template::Empty];

    pub fn label(self) -> &'static str {
        match self {
            Template::Starter => "Starter",
            Template::Empty => "Empty",
        }
    }

    /// The line under the choice, for the selected template.
    pub fn describe(self) -> &'static str {
        match self {
            Template::Starter => "Prototype pieces you can place right away",
            Template::Empty => "No pieces yet: add your own .glb meshes",
        }
    }

    /// The kit the template binds as the authoring kit.
    fn kit(self) -> &'static str {
        match self {
            Template::Starter => STARTER_KIT,
            Template::Empty => EMPTY_KIT,
        }
    }
}

/// Where new projects go unless a person says otherwise: `Cauldron Projects` in the documents folder, or in the home folder
/// of a system that has no documents folder.
pub fn default_location() -> PathBuf {
    dirs::document_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from(".")).join(DEFAULT_PLACE)
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
        NewProject { name: String::new(), location: default_location().to_string_lossy().into_owned(), template: Template::default() }
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
    } else if !location.is_dir() && location != default_location() {
        // The default place is made by Create, so its not being there yet is not a problem.
        out.location = Some(format!("{} is not a folder.", location.display()));
    } else if location.is_dir() && std::fs::metadata(location).is_ok_and(|m| m.permissions().readonly()) {
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

/// Makes the project the form describes — its folder, its template's kit and `first_map` — and returns the folder.
/// Refuses, with [`check`]'s sentence, a form that is not ready. A project that could not be finished is taken away again,
/// so the same name can be tried once the cause is gone.
pub fn create(form: &NewProject) -> Result<PathBuf, String> {
    let problems = check(form);
    if let Some(reason) = problems.reason() {
        return Err(reason.to_owned());
    }
    let place = Path::new(form.location.trim());
    std::fs::create_dir_all(place).map_err(|e| format!("cannot make {}: {e}", place.display()))?;
    let dir = crate::catalog::create_project_with(place, &form.name, VOCAB)?;
    if let Err(e) = fill(&dir, form.template, form.name.trim()) {
        // Made a moment ago by this call and nothing else has opened it.
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!("{} was not made: {e}", dir.display()));
    }
    Ok(dir)
}

/// The template's kit, its pieces, and the first map, written into the project at `dir`.
fn fill(dir: &Path, template: Template, name: &str) -> Result<(), String> {
    crate::catalog::create_kit(dir, template.kit())?;
    if template == Template::Starter {
        copy_prototype_kit(dir)?;
    }
    // **A person who makes a project starts with an agent guide** (AG-04, #349): the four sections, so
    // the file's shape is found rather than read about. The grant is written by whoever holds the
    // person's trust file — the form's Create button and the project page — because the store is the
    // person's and not the project's.
    crate::guide_template::write_file(dir)?;
    // **And with a style** (SG-08, #407): `design/style.md`, the nine sections of the game's look and feel, each one prompt line, so the
    // file's shape is found rather than read about. Only a project the editor makes gets one: opening any other writes no file.
    crate::design_templates::write_style(dir, bevy_grimoire_core::design::DESIGN_DIR, name)?;
    // **The derived folder is not the project's work** (#366): `.dark_factory/schema/` holds JSON Schema
    // files generated from the types at every open and from the game's registry at every start, so a project
    // that committed them would diff a file nobody edited every time a type changed. The guide and the
    // journal under `.dark_factory/` ARE the project's, so the ignore names the one folder and not the parent.
    let ignore = dir.join(GITIGNORE);
    bevy_grimoire_core::atomic_file::write_new(&ignore, SCHEMA_IGNORE)
        .map_err(|e| format!("{}: {e}", ignore.display()))?;
    let [width, depth, height] = FIRST_MAP_METRES;
    let origin = bevy_grimoire_core::map::Map::default().origin;
    crate::project_page::from_the_page(
        dir,
        "map_create",
        &serde_json::json!({ "name": FIRST_MAP, "bounds": [width, height, depth], "origin": [origin.0, origin.1, origin.2], "note": null }),
    )?;
    Ok(())
}

/// Copies the Prototype Kit's meshes and licence out of the example project into `dir`, and its rows into the Starter kit's library.
fn copy_prototype_kit(dir: &Path) -> Result<(), String> {
    let Some(example) = crate::launcher::example_project() else {
        return Err("this copy of Cauldron has no example project to take the prototype pieces from; make an Empty project instead".to_owned());
    };
    let rows_path = example.join(PROTOTYPE_ROWS);
    let text = std::fs::read_to_string(&rows_path).map_err(|e| format!("{}: {e}", rows_path.display()))?;
    let mut library = bevy_grimoire_core::library::Library::parse(&text).map_err(|e| format!("{}: {e}", rows_path.display()))?;
    library.note = Some(PROTOTYPE_NOTE.to_owned());
    let meshes = example.join(PROTOTYPE_MESHES);
    crate::launcher::copy_tree(&meshes, &dir.join(PROTOTYPE_MESHES)).map_err(|e| format!("{} was not copied: {e}", meshes.display()))?;
    let into = dir.join(crate::catalog::GRIMOIRE_DIR).join(STARTER_KIT).join(bevy_grimoire_core::policy::LIBRARY_FILE);
    bevy_grimoire_core::save::save_atomic(&into, &library.to_yaml()?)
}

/// **The file a new project is given so the generated folder is not committed**, and what it says.
///
/// Named rather than a literal at the call site: a test asserts a new project carries it and what it ignores.
pub const GITIGNORE: &str = ".gitignore";

/// **The one line a new project's `.gitignore` holds** — the folder the editor rewrites.
pub const SCHEMA_IGNORE: &str = "# Generated from the Rust types and the game's registry on every open; never edited by hand.\n.dark_factory/schema/\n";

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

/// The line that says which folder Create will make.
#[derive(Component)]
pub struct FolderLine;

/// The line under the Template choice that says what the selected template holds.
#[derive(Component)]
pub struct TemplateNote;

/// The Choose… button beside Location.
#[derive(Component)]
pub struct ChooseButton;

/// The line that says why Create is disabled.
#[derive(Component)]
pub struct ReasonLine;

pub struct NewProjectPlugin;

impl Plugin for NewProjectPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (open_the_form, spawn_the_card, show_the_form, take_the_location, close_the_form).chain().run_if(in_state(Screen::Launcher)),
        )
        .add_systems(OnExit(Screen::Launcher), |mut commands: Commands| commands.remove_resource::<NewProjectForm>());
    }
}

/// The launcher's "New project…" button asks for the form by setting [`Launcher::new_project_asked`]; the form is a
/// resource so that the screen's own state (`Launcher`) stays about the list. **Opening it forgets what the home last
/// refused**: a toast about another folder has nothing to do with this form.
fn open_the_form(
    mut commands: Commands,
    mut launcher: ResMut<Launcher>,
    form: Option<Res<NewProjectForm>>,
    refusals: Query<(Entity, &bevy_gooey::Toast)>,
) {
    if launcher.new_project_asked && form.is_none() {
        commands.insert_resource(NewProjectForm::default());
        launcher.problem = None;
        for (toast, text) in &refusals {
            if text.kind() == bevy_gooey::ToastKind::Alert {
                commands.entity(toast).despawn();
            }
        }
    }
    launcher.new_project_asked = false;
}

fn spawn_the_card(
    mut commands: Commands,
    form: Option<Res<NewProjectForm>>,
    cards: Query<(), With<FormCard>>,
) {
    let Some(form) = form else { return };
    if !cards.is_empty() {
        return;
    }
    let start = form.0.clone();
    // A modal dialog over the home, as the project page's New map is: gooey centres the card and holds the keyboard inside it.
    // Esc is its Cancel, which closes the form as the Cancel button does.
    commands
        .spawn((
            dialog(),
            FormCard,
            DespawnOnExit(Screen::Launcher),
            Name::new("new project"),
            observe(|_: On<Cancel>, mut launcher: ResMut<Launcher>| launcher.new_project_done = true),
        ))
        .with_children(|scrim| {
        scrim.spawn(bevy_gooey::sized_panel(bevy_gooey::CardWidth::Text(crate::launcher::DIALOG_CARD_TEXT_SIZES)))
            .with_children(|card| {
                card.spawn(heading("New project"));
                card.spawn((
                    text_input("Name", start.name.clone()),
                    NameField,
                    // Ready to type in the moment the form opens.
                    AutoFocus,
                    TextHint("project name".into()),
                    observe(|change: On<ValueChange<String>>, mut form: ResMut<NewProjectForm>| form.0.name = change.value.clone()),
                ));
                card.spawn((crate::launcher::quiet(FOLDER_UNNAMED), FolderLine));
                crate::chrome::gapped_row(card, crate::chrome::GAP_GROUP_STEPS, crate::chrome::NO_MARGIN, AlignItems::End).with_children(|row| {
                    row.spawn((
                        text_input("Location", start.location.clone()),
                        LocationField,
                        observe(|change: On<ValueChange<String>>, mut form: ResMut<NewProjectForm>| form.0.location = change.value.clone()),
                    ));
                    row.spawn((
                        button("Choose…"),
                        ChooseButton,
                        observe(|_: On<bevy_gooey::Activate>, mut out: MessageWriter<PickRequest>| {
                            out.write(PickRequest(PickFor::Location));
                        }),
                    ));
                });
                card.spawn((
                    choice("Template", Template::ALL.map(Template::label), Template::ALL.iter().position(|t| *t == start.template).unwrap_or_default()),
                    TemplateChoice,
                    observe(|change: On<ValueChange<usize>>, mut form: ResMut<NewProjectForm>| {
                        form.0.template = Template::ALL.get(change.value).copied().unwrap_or_default();
                    }),
                ));
                card.spawn((crate::launcher::quiet(start.template.describe()), TemplateNote));
                card.spawn((label(""), ReasonLine));
                crate::chrome::end_row(card, crate::chrome::GAP_GROUP_STEPS).with_children(|row| {
                    row.spawn((
                        button("Cancel"),
                        observe(|_: On<bevy_gooey::Activate>, mut launcher: ResMut<Launcher>| launcher.new_project_done = true),
                    ));
                    row.spawn((
                        submit_button("Create"),
                        // **Return presses it** from the name field (UI-R03, #333): a dialog with a text field names its main action.
                        bevy_gooey::DefaultButton,
                        CreateButton,
                        observe(|_: On<bevy_gooey::Activate>, form: Res<NewProjectForm>, mut launcher: ResMut<Launcher>, state: Option<ResMut<crate::editor::EditorState>>, store_root: Option<Res<crate::trusted::StoreRoot>>, mut out: MessageWriter<OpenMapRequest>| {
                            // Checked again here: a dimmed button can still be activated by a script.
                            match create(&form.0) {
                                Ok(dir) => {
                                    // **The person who made the project wrote the guide**, so this is
                                    // where its grant is written (AG-04, #349): the store is the person's,
                                    // and the form's Create is the call that holds the editor's own. A
                                    // store that will not take it is a problem the person is shown, and
                                    // the project still opens — the guide is simply withheld until the
                                    // project page can allow it.
                                    // **The grant is part of making the project**, and its store is
                                    // resolved here: `None` is the person's own `trusted.yaml`
                                    // (`crate::trusted::store_path`), so a real editor always writes one.
                                    // There is no silent "no store, no grant" arm: a guide nobody allowed
                                    // is the state this whole slice exists to prevent.
                                    // **The style is granted in the same step** (SG-08, #407), for the same reason: the person who
                                    // made the project wrote `design/style.md`, so `grimoire_style` serves it with no question.
                                    let trust_file = state.as_deref().and_then(|s| s.trust.file.as_deref());
                                    match crate::guide_template::grant(&dir, store_root.as_deref(), trust_file)
                                        .and_then(|()| crate::design_templates::grant_style(&dir, store_root.as_deref(), trust_file))
                                    {
                                        Ok(()) => launcher.problem = None,
                                        Err(e) => launcher.problem = Some(e),
                                    }
                                    launcher.new_project_done = true;
                                    out.write(OpenMapRequest(dir, FIRST_MAP.to_owned()));
                                }
                                Err(e) => launcher.problem = Some(e),
                            }
                        }),
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
    mut texts: ParamSet<(Lines<ReasonLine>, Lines<FolderLine>, Lines<TemplateNote>)>,
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
    let folder = if form.0.name.trim().is_empty() { FOLDER_UNNAMED.to_owned() } else { format!("Folder: {}", folder_name(&form.0.name)) };
    let note = form.0.template.describe();
    set_text(&mut texts.p0(), &said);
    set_text(&mut texts.p1(), &folder);
    set_text(&mut texts.p2(), note);
}

/// The words of the one line marked `M`.
type Lines<'w, 's, M> = Query<'w, 's, &'static mut Text, With<M>>;

fn set_text<M: Component>(lines: &mut Lines<M>, said: &str) {
    for mut text in lines {
        if text.0 != said {
            text.0 = said.to_owned();
        }
    }
}

/// The folder the chooser picked is the Location, in the form and in the field a person sees.
fn take_the_location(mut picked: MessageReader<Picked>, form: Option<ResMut<NewProjectForm>>, mut field: Query<&mut TextInputValue, With<LocationField>>) {
    let Some(mut form) = form else { return };
    for Picked { for_what, paths } in picked.read() {
        let (PickFor::Location, Some(folder)) = (for_what, paths.first()) else { continue };
        let folder = folder.to_string_lossy().into_owned();
        form.0.location.clone_from(&folder);
        for mut value in &mut field {
            value.0.clone_from(&folder);
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
