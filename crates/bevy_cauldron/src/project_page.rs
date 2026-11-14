//! **The project page: a project's maps and kits** (#296) — where a project opened from the home, or named on the command line,
//! lands before an editor door opens. It is the chooser's replacement: `chooser.rs` was a keyboard-first four-column screen, and this
//! is the same job in the window's own gooey widgets, standing in the home's shell (`launcher::spawn_shell`), so the sidebar stays.
//!
//! ```text
//! Projects › my_game
//!   Maps    [+ New map] [bedroom: 10 × 10 m, 4 m tall, 13 pieces placed] …
//!   Kits    prototype · 12 pieces · Default for new pieces     + New kit
//! ```
//!
//! # What it does, by the chooser's own names
//!
//! * **Kit create** and **map create** are the registry's operations (`kit_create`, `map_create`), dispatched with the project's root and no
//!   loaded project: the page is the screen before a door, exactly as the chooser was, and an agent's call and a person's click are one
//!   write. A new map opens at once, as the chooser's name field did.
//! * **Authoring**: the kit marked *Default for new pieces* is `kits.yaml`'s `authoring`, changed by `kit_set_authoring`.
//! * **Delete** asks first, in the one prompt (`confirm.rs`), naming what goes; a kit the project still depends on is refused with the files
//!   that name it ([`crate::catalog::ask_delete_kit`]).
//! * **Bash**: a map's *uses pieces from* is one of the combinations `kits.yaml` declares, chosen when the map is made and changed from its
//!   menu ([`crate::catalog::cycle_bash`]).
//! * **Map info** is on the card: its size in metres, how many pieces stand in it, where its pieces come from and why it will not open.
//! * **An agent's `map_open` and `door_open`** enter the editor from here as from the chooser: `ops_brp` reads this screen as the one a
//!   door is entered from.
//!
//! Every action is a [`PageAction`] message, so a button, a context menu and a test all ask the same way.
//!
//! # Esc goes back, and nothing here quits
//!
//! Esc closes a dialog, and with none open goes back to the grid. Quit is Cmd/Ctrl+Q (`confirm.rs`'s `QuitApp`), never a key a person
//! presses to leave a dialog.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;
use bevy::ecs::hierarchy::ChildSpawner;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::observe;
use bevy_gooey::{Activate, Cancel, ContextMenu, ContextTarget, FieldProblem, Role, ValueChange, button, choice, context_menu, dialog, heading, label, menu_item, number_field, panel, section, separator, text_input, toast, toast_alert, toggle};

use crate::catalog::{self, Catalog, Kit, MapEntry, MapSummary, Pending, PolicyEntry, plural, size_in_metres};
use crate::confirm::{Asked, Confirm};
use crate::launcher::{self, HomeContent, Launcher, ProjectPictures, SidebarRoot, SplashArt};
use crate::project_assets::{self, ProjectRoot};
use crate::screen::Screen;
use crate::ui::submit_button;
use crate::workspace::Workspace;
use bevy_grimoire_core::trust::{self, TrustStore};

/// The one message every action of the page goes through.
#[derive(Message, Clone, Debug, PartialEq)]
pub enum PageAction {
    /// Open the map `0` in the Map door.
    OpenMap(String),
    /// Open the kit `0` in the Kit door.
    OpenKit(String),
    /// Ask for the new-map dialog.
    NewMap,
    /// Ask for the new-kit dialog.
    NewKit,
    /// Ask for the rename dialog of the map `0`.
    RenameMap(String),
    /// Ask whether to delete the map `0`.
    DeleteMap(String),
    /// Ask whether to delete the kit `0`.
    DeleteKit(String),
    /// Make the kit `0` the one new work lands in.
    MakeDefault(String),
    /// Change where the map `0` draws its pieces from: the next combination `kits.yaml` declares, or every kit.
    NextSource(String),
    /// Ask for the settings dialog of the map `0`: its size, origin and note.
    MapSettings(String),
    /// Ask for the policy dialog of the kit `0`: what it leaves out and changes.
    KitPolicy(String),
    /// Ask whether to remove `entry` from the policy of the kit `kit`.
    RemovePolicy { kit: String, entry: PolicyEntry },
    /// Back to the home's grid.
    Back,
    /// **The agent guide section** (AG-05, #350). Each is the person's own act on the page, like a trust
    /// grant: **no operation dispatches one**, so no agent can write the guide it is following.
    ///
    /// The area's text changed; held as an unsaved edit until Save.
    GuideEdited(String),
    /// Write the text area's content to the file and grant it.
    SaveGuide,
    /// Throw the unsaved edit away and show the file's text.
    RevertGuide,
    /// A person allows this exact content.
    AllowGuide,
    /// A person trusts the whole project, so every later edit is allowed.
    TrustProject,
    /// Write the template through [`crate::guide_template::write`].
    CreateGuide,
    /// Take the file's text over the unsaved edit.
    ReloadGuide,
    /// Write the unsaved edit over the file's text.
    KeepMyGuide,
}

/// **What the page is showing.** Rebuilt from disk every time the screen is entered, and whenever the project's files change.
#[derive(Resource, Debug)]
pub struct ProjectPage {
    pub root: PathBuf,
    pub catalog: Catalog,
    /// What the question on screen was about, until it is answered.
    pub asking: Option<Pending>,
    /// The policy entry the question on screen is about to remove, with the kit it belongs to, until it is answered.
    asking_policy: Option<(String, PolicyEntry)>,
    /// The cards are out of date: the next frame draws them again.
    pub dirty: bool,
    /// The project's files as they were when the catalog was read ([`fingerprint`]).
    seen: Vec<(PathBuf, SystemTime)>,
    /// Seconds until the files are looked at again.
    since: f32,
    /// **The Agent guide section's held text** (AG-05, #350): the file as it last read, the person's
    /// unsaved edit, and whether somebody else changed the file under it. On the page rather than as a
    /// resource of its own, so a redraw and an action cannot read two different values.
    pub guide: GuidePage,
    /// **Is this the page drawn inside the workbench** (the Project workspace, NV-08, #385), not the screen before a door? There is no
    /// breadcrumb back to the home (the editor has a project open and closing it is Close project), the maps' and kits' *open* go through the
    /// editor's own tabs and workspaces, and a map that is open in the editor is not renamed, changed or deleted from under it.
    pub in_workspace: bool,
}

impl ProjectPage {
    /// The project's folder name: the page's title.
    pub fn name(&self) -> String {
        self.root.file_name().map_or_else(|| self.root.display().to_string(), |n| n.to_string_lossy().into_owned())
    }
}

/// How often, in seconds, the page looks at the project's files to see whether a map or a kit was made, renamed or deleted elsewhere (an
/// agent, the editor, a person's file manager). A listing that follows its directory is what the chooser's `follow_the_maps_directory` did.
const WATCH_SECS: f32 = 1.0;

/// The files whose change makes the page out of date: the maps folder's entries, `kits.yaml`, and each kit's library.
pub fn fingerprint(root: &Path) -> Vec<(PathBuf, SystemTime)> {
    let base = root.join(catalog::GRIMOIRE_DIR);
    let mut watched = vec![base.join(catalog::MAPS_DIR), base.join(bevy_grimoire_core::kits::KITS_FILE)];
    for dir in std::fs::read_dir(&base).into_iter().flatten().flatten().map(|e| e.path()) {
        watched.push(dir.join(bevy_grimoire_core::policy::LIBRARY_FILE));
    }
    for map in std::fs::read_dir(base.join(catalog::MAPS_DIR)).into_iter().flatten().flatten() {
        watched.push(map.path());
    }
    let mut out: Vec<_> = watched.into_iter().filter_map(|p| Some((p.clone(), std::fs::metadata(&p).ok()?.modified().ok()?))).collect();
    out.sort();
    out
}

/// A dialog's fields. Observers write it; Create reads it.
#[derive(Resource, Debug, Clone, PartialEq)]
struct Form {
    kind: FormKind,
    name: String,
    /// Width, depth and height of a map, in metres.
    metres: [f32; 3],
    /// Where a map's floor starts, in metres (x, y, z).
    origin: [f32; 3],
    /// A map's note.
    note: String,
    /// Which "uses pieces from" is chosen: 0 is every kit, `n` is the project's `n`th bash.
    source: usize,
    /// Whether a new kit becomes the one new work lands in.
    default: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum FormKind {
    NewMap,
    NewKit,
    /// Renaming the map of this name.
    RenameMap(String),
    /// Changing the size, origin and note of the map of this name.
    MapSettings(String),
}

impl Form {
    /// A form of `kind` with nothing typed: a new map is ten metres square and four tall, at the world's origin.
    fn new(kind: FormKind, name: String) -> Form {
        Form { kind, name, metres: DEFAULT_METRES, origin: [0.0; 3], note: String::new(), source: 0, default: false }
    }
}

/// A new map is ten metres square and four tall, as `Map::default` says.
const DEFAULT_METRES: [f32; 3] = [10.0, 10.0, 4.0];
/// How far one drag pixel or one key press moves a size, and the least and most a size may be.
const METRES_STEP: f32 = 0.5;
const METRES_MIN: f32 = 1.0;
const METRES_MAX: f32 = 1000.0;

#[derive(Component)]
pub struct PageDialog;
#[derive(Component)]
pub struct NameField;
#[derive(Component)]
pub struct SavedAs;
#[derive(Component)]
pub struct CreateButton;
/// A map size field of the New map and Map settings dialogs: width (0), depth (1) or height (2).
#[derive(Component)]
pub struct SizeField(pub usize);
/// A map origin field of the Map settings dialog: x (0), y (1) or z (2).
#[derive(Component)]
pub struct OriginField(pub usize);
/// The note field of the Map settings dialog.
#[derive(Component)]
pub struct NoteField;
/// The page's body: the breadcrumb and the Maps and Kits sections. `pub` so a test can tell the page apart from the home.
#[derive(Component)]
pub struct PageBody;
#[derive(Component)]
struct MapMenu;
#[derive(Component)]
struct KitMenu;

/// A map card, by the map's name.
#[derive(Component)]
pub struct MapCard(pub String);

/// A kit row, by the kit's label.
#[derive(Component)]
pub struct KitRow(pub String);

// ── the Agent guide section (AG-05, #350) ─────────────────────────────────────────────────────────

/// The Agent guide section of the project page.
#[derive(Component)]
pub struct AgentGuide;
/// The section's status line: `allowed`, `waiting for you` or `missing`.
#[derive(Component)]
pub struct AgentGuideStatus;
/// The section's `text_area`, which holds the file's text and the person's unsaved edits. **Drawn only when
/// the whole guide fits**, so its presence is the field the page can write from.
#[derive(Component)]
pub struct AgentGuideText;

/// **The read-only prefix the section shows when the guide does not fit.** Deliberately not
/// [`AgentGuideText`]: nothing typed into a view of part of the file can be written over the whole of it.
#[derive(Component)]
pub struct AgentGuideShown;
#[derive(Component)]
pub struct AgentGuideSave;
#[derive(Component)]
pub struct AgentGuideRevert;
#[derive(Component)]
pub struct AgentGuideAllow;
#[derive(Component)]
pub struct AgentGuideTrust;
#[derive(Component)]
pub struct AgentGuideCreate;
#[derive(Component)]
pub struct AgentGuideReload;
#[derive(Component)]
pub struct AgentGuideKeepMine;

/// **The row the section's state-dependent buttons are in**, filled and refilled by
/// [`fill_the_guide_buttons`].
#[derive(Component)]
pub struct AgentGuideButtons;

/// What a button row was last built for, so it is refilled when — and only when — one of these moves.
#[derive(Component, Clone, Copy, PartialEq)]
struct ButtonsFor {
    state: crate::agent_guide::GuideState,
    conflict: bool,
    editing: bool,
}

/// **What the Agent guide section is holding, and what it last read.**
///
/// **One resource, so the file, the edit and the answer cannot disagree.** `on_disk` is the text the
/// section last read or wrote; an area whose text is not `on_disk` has unsaved edits, and a file whose
/// text is neither `on_disk` nor the area's is a change from somewhere else.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuidePage {
    /// The text the section last read from, or wrote to, the file.
    pub on_disk: String,
    /// The area's text while it has unsaved edits; `None` when it matches the disk.
    pub editing: Option<String>,
    /// **Somebody else changed the file while there were unsaved edits.** The file is not overwritten until
    /// the person chooses Reload or Keep mine.
    pub conflict: bool,
    /// **What the file says and which state it is in**, read from disk each time the page looks.
    pub view: GuideView,
}

impl GuidePage {
    /// Whether the area holds text the file does not.
    pub fn dirty(&self) -> bool {
        self.editing.is_some()
    }
}

/// **The home's page**: the screen before a door opens, when a project is chosen. Added by the home's plugins (`launcher::LauncherPlugin`).
pub struct ProjectPagePlugin;

impl Plugin for ProjectPagePlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<PageSystemsPlugin>() {
            app.add_plugins(PageSystemsPlugin);
        }
        app.add_systems(OnEnter(Screen::Project), (build_page, fill_page).chain()).add_systems(
            Update,
            (back_on_escape.before(bevy_gooey::GooeySystems::Navigate), launcher::hide_the_divider).chain().before(act).run_if(in_state(Screen::Project)),
        );
    }
}

/// **What the page does wherever it is drawn** (NV-08, #385): its actions, dialogs, guide section and redraws, and the Project workspace's
/// panel. Added by the editor, so the Project workspace draws its page in a build that has no home, and by [`ProjectPagePlugin`] when the
/// editor has not (the order the plugins are added in does not matter). Nothing here needs the home's resources: the splash, the picture
/// cache and the launcher are all optional, so a stepped editor without the home still gets a page.
pub struct PageSystemsPlugin;

impl Plugin for PageSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PageAction>()
            // **The same page inside the workbench**: spawned when the Project workspace is entered (the frame is rebuilt on every switch,
            // `screen::cross`), and forgotten on the next switch that is not to it.
            .add_systems(
                OnEnter(Screen::Editor),
                (
                    spawn_the_workspace_page.run_if(Workspace::project_is_open).after(crate::chrome::FrameSystems),
                    forget_the_workspace_page.run_if(not(Workspace::project_is_open)),
                ),
            )
            .add_systems(
                Update,
                (act, the_guide_section, read_the_guide, answer_the_question, follow_the_files, redraw, fill_the_guide_buttons, show_the_form)
                    .chain()
                    .run_if(page_is_showing),
            );
    }
}

/// **Is the page on screen, as the screen before a door or as the Project workspace?** It needs the page it draws: a frame between the switch and the
/// spawn has none, and a system that takes `ResMut<ProjectPage>` panics on a missing resource rather than skipping.
fn page_is_showing(screen: Option<Res<State<Screen>>>, workspace: Option<Res<Workspace>>, page: Option<Res<ProjectPage>>) -> bool {
    page.is_some()
        && match screen.as_deref().map(State::get) {
            Some(Screen::Project) => true,
            Some(Screen::Editor) => workspace.is_some_and(|w| *w == Workspace::Project),
            _ => false,
        }
}

/// The width of the Project workspace's panel, logical px: two map cards and the gap between them.
const WORKSPACE_PAGE_W: f32 = 560.0;

/// The root of the Project workspace's panel, so a test and the layout can name it.
#[derive(Component)]
pub struct WorkspacePage;

/// **The page, read from disk**: the catalog, the files it was read from and the guide, as the page is first drawn. A catalog that will not
/// read is a page with nothing on it and the problem to say so.
fn read_the_page(root: PathBuf, state: Option<&crate::editor::EditorState>, store_root: Option<&crate::trusted::StoreRoot>, in_workspace: bool) -> (ProjectPage, Option<String>) {
    let (catalog, problem) = match Catalog::scan(&root) {
        Ok(c) => (c, None),
        Err(e) => (Catalog { kits: Vec::new(), maps: Vec::new(), authoring: None, bashes: Vec::new() }, Some(e)),
    };
    let seen = fingerprint(&root);
    // **The guide is read as the page is built**, not a frame later: the section's first draw is what a
    // person sees, and a first draw that said "allowed" about a project with no guide would be a line that
    // is wrong until something else moved.
    let mut page = ProjectPage { root, catalog, asking: None, asking_policy: None, dirty: false, seen, since: 0.0, guide: GuidePage::default(), in_workspace };
    let view = agent_guide_view(&page, store_root, state.and_then(|s| s.trust.file.clone()).as_deref());
    page.guide.on_disk = view.text.clone();
    page.guide.view = view;
    (page, problem)
}

/// **Spawn the project's page into the Project workspace's panel** (NV-08, #385): the body `fill_page` draws on the home, without the
/// breadcrumb, in a panel of the left dock. Spawned on demand and not before, for the outliner's reason: its buttons and text boxes are
/// entities the screens that count every button in the world would find on the Map. The splash and the picture cache are the home's and
/// optional: an editor stood up without the home draws the same page, with no splash picture and a cache of its own.
#[allow(clippy::too_many_arguments)]
fn spawn_the_workspace_page(
    mut commands: Commands,
    frame: Res<crate::chrome::Frame>,
    project: Option<Res<crate::project::Project>>,
    mut images: ResMut<Assets<Image>>,
    splash: Option<ResMut<SplashArt>>,
    pictures: Option<ResMut<ProjectPictures>>,
    state: Option<Res<crate::editor::EditorState>>,
    store_root: Option<Res<crate::trusted::StoreRoot>>,
) {
    let Some(project) = project else { return };
    let mut splash = splash;
    if let Some(splash) = splash.as_deref_mut() {
        launcher::ensure_splash(&mut images, splash);
    }
    let (page, problem) = read_the_page(project.root.clone(), state.as_deref(), store_root.as_deref(), true);
    if let Some(problem) = problem {
        commands.spawn(toast_alert(problem).sticky());
    }
    let panel = crate::chrome::panel_root(&mut commands, &frame, crate::chrome::Side::Left, WORKSPACE_PAGE_W, true, false, "PROJECT").insert(WorkspacePage).id();
    let mut scroll = None;
    commands.entity(panel).with_children(|p| {
        crate::chrome::title(p, "PROJECT");
        // FOLLOW-OK: sections of cards and rows, not a list the arrows walk; the scroll exists for HEIGHT, as the Map's inspector's does.
        scroll = Some(crate::chrome::scroll_list(p, ()).id());
    });
    let Some(scroll) = scroll else { return };
    let mut own_pictures = ProjectPictures::default();
    let mut pictures = pictures;
    let pictures = pictures.as_deref_mut().unwrap_or(&mut own_pictures);
    spawn_the_page(&mut commands, scroll, &page, &mut images, splash.as_deref().and_then(|s| s.0.clone()), pictures);
    commands.insert_resource(page);
}

/// **The page's two context menus and its body, under `parent`, drawn**: the one path the home (`fill_page`) and the Project workspace
/// (`spawn_the_workspace_page`) both take, so they cannot come to differ.
fn spawn_the_page(commands: &mut Commands, parent: Entity, page: &ProjectPage, images: &mut Assets<Image>, splash: Option<(Handle<Image>, Vec2)>, pictures: &mut ProjectPictures) {
    let map_menu = commands.spawn((context_menu(()), MapMenu, ChildOf(parent))).id();
    let kit_menu = commands.spawn((context_menu(()), KitMenu, ChildOf(parent))).id();
    spawn_menus(commands, map_menu, kit_menu, &page.catalog);
    let body = commands
        .spawn((bevy_gooey::fill_region_with(crate::chrome::GAP_GROUP_STEPS), PageBody, ChildOf(parent)))
        .id();
    draw_body(commands, body, page, map_menu, kit_menu, images, splash, pictures);
}

/// **A page left behind by the Project workspace goes when another workspace is entered**, so nothing reads a stale catalog.
fn forget_the_workspace_page(page: Option<Res<ProjectPage>>, mut commands: Commands) {
    if page.is_some_and(|p| p.in_workspace) {
        commands.remove_resource::<ProjectPage>();
    }
}

fn build_page(
    mut commands: Commands,
    root: Res<ProjectRoot>,
    mut images: ResMut<Assets<Image>>,
    mut splash: ResMut<SplashArt>,
    state: Option<Res<crate::editor::EditorState>>,
    store_root: Option<Res<crate::trusted::StoreRoot>>,
) {
    launcher::ensure_splash(&mut images, &mut splash);
    let Some(root) = root.current() else {
        error!("the project page was entered with no project to show");
        return;
    };
    let (page, problem) = read_the_page(root, state.as_deref(), store_root.as_deref(), false);
    commands.insert_resource(page);
    if let Some(problem) = problem {
        commands.spawn(toast_alert(problem).sticky());
    }
    launcher::spawn_shell(&mut commands, Screen::Project);
}

#[allow(clippy::too_many_arguments)]
fn fill_page(
    mut commands: Commands,
    page: Option<Res<ProjectPage>>,
    launcher: Res<Launcher>,
    splash: Res<SplashArt>,
    updater: Option<Res<crate::update::Updater>>,
    sidebar: Query<Entity, With<SidebarRoot>>,
    content: Query<Entity, With<HomeContent>>,
    mut images: ResMut<Assets<Image>>,
    mut pictures: ResMut<ProjectPictures>,
) {
    let (Some(page), Ok(sidebar), Ok(content)) = (page, sidebar.single(), content.single()) else { return };
    launcher::fill_sidebar(&mut commands, sidebar, &launcher, &splash, updater.as_deref());
    spawn_the_page(&mut commands, content, &page, &mut images, splash.0.clone(), &mut pictures);
}

/// The page's body: the breadcrumb, then the Maps section and the Kits section.
#[allow(clippy::too_many_arguments)]
fn draw_body(commands: &mut Commands, body: Entity, page: &ProjectPage, map_menu: Entity, kit_menu: Entity, images: &mut Assets<Image>, splash: Option<(Handle<Image>, Vec2)>, pictures: &mut ProjectPictures) {
    commands.entity(body).with_children(|body| {
        if page.in_workspace {
            // **No breadcrumb inside the workbench**: the project is open, and leaving it is Close project, which asks about unsaved work.
            body.spawn(heading(page.name()));
        } else {
            crate::chrome::gapped_row(body, crate::chrome::GAP_ROW_STEPS, crate::chrome::NO_MARGIN, AlignItems::Center).with_children(|crumbs| {
                crumbs.spawn((button("Projects"), observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::Back); })));
                crumbs.spawn(label("›"));
                crumbs.spawn(heading(page.name()));
            });
        }
        let picture = launcher::project_picture(&page.root, images, splash, pictures);
        let maps = page.catalog.maps.iter().map(|m| (m.clone(), picture.clone())).collect::<Vec<_>>();
        body.spawn(section(
            "Maps",
            true,
            Children::spawn(bevy::ecs::spawn::SpawnWith(move |parent: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>| {
                parent
                    .spawn((
                        bevy_gooey::row_with(crate::chrome::GAP_GROUP_STEPS, bevy_gooey::RowLayout { wrap: Some(FlexWrap::Wrap), ..default() }),
                        bevy_gooey::Spacing { padding: bevy_gooey::Sides::y(crate::chrome::STEP_ROW), ..default() },
                    ))
                    .with_children(|grid| {
                        grid.spawn((button("+ New map"), observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::NewMap); })));
                        for (map, picture) in &maps {
                            map_card(grid, map, picture.clone(), map_menu);
                        }
                    });
            })),
        ));
        let kits = page.catalog.kits.clone();
        let authoring = page.catalog.authoring.clone();
        body.spawn(section(
            "Kits",
            true,
            Children::spawn(bevy::ecs::spawn::SpawnWith(move |parent: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>| {
                parent.spawn((bevy_gooey::column(crate::chrome::GAP_TIGHT_STEPS), bevy_gooey::Spacing { padding: bevy_gooey::Sides::y(crate::chrome::STEP_ROW), ..default() })).with_children(|rows| {
                    for kit in &kits {
                        kit_row(rows, kit, authoring.as_deref() == Some(kit.label.as_str()), kit_menu);
                    }
                    rows.spawn((button("+ New kit"), observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::NewKit); })));
                });
            })),
        ));
        // **The agent guide** (AG-05, #350): the file an agent is given for this project, read here, edited
        // here, and allowed here — because a person who answered "not now" to the modal needs a second
        // place to say yes, and because reading the file is how anybody decides.
        draw_agent_guide(body, &page.guide);
    });
}

/// **What the Agent guide section draws**, read from the file and the store on the frame the page redraws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GuideView {
    pub state: crate::agent_guide::GuideState,
    /// **A problem that stopped the decision**, shown instead of a guess: a store that will not read, or a
    /// project root that will not resolve. `None` when the store gave an answer.
    pub trouble: Option<String>,
    pub text: String,
    /// The AG-02 problem a guide that cannot be served has, shown instead of the editor.
    pub problem: Option<String>,
}

/// **Read the guide as the section needs it**: the file's text, the state the store gives it, or the
/// problem that stops it being served. A missing file is `Missing`, not a problem.
///
/// **The state is the store's answer, not the section's guess.** The same decision `grimoire/agent_guide`
/// makes is what colours the status line, so a person pressing Allow here and an agent asking are looking
/// at one rule.
pub fn agent_guide_view(page: &ProjectPage, store_root: Option<&crate::trusted::StoreRoot>, trust_file: Option<&Path>) -> GuideView {
    use crate::agent_guide::GuideState;
    let path = crate::agent_guide::project_file(&page.root);
    let text = match bevy_grimoire_core::agent_guide::read_layer(bevy_grimoire_core::agent_guide::Source::Project, &path) {
        Ok(Some(text)) => text,
        Ok(None) => return GuideView { state: GuideState::Missing, text: String::new(), problem: None, trouble: None },
        Err(problem) => {
            return GuideView { state: GuideState::Waiting, text: String::new(), problem: Some(problem.line()), trouble: None };
        }
    };
    // **`None` is the person's own store** (`crate::trusted::resolve`), as it is for the guide the agent is
    // served — the page and the answer must agree about what is allowed.
    //
    // **A store that will not read is said, not swallowed.** It is not "everything is allowed" (the grant
    // would be a fiction) and it is not silently "waiting for you" either: a person whose store is damaged
    // is the only one who can fix it, and the section is where it is written down.
    let state = match crate::trusted::load_at(store_root, trust_file).and_then(|store| store.guide_decision(&page.root, &text)) {
        Ok(trust::Decision::Trusted(_)) => GuideState::Allowed,
        Ok(trust::Decision::Untrusted) => GuideState::Waiting,
        Err(e) => {
            return GuideView { state: GuideState::Waiting, text, problem: None, trouble: Some(e) };
        }
    };
    GuideView { state, text, problem: None, trouble: None }
}

/// **How many rows the section's `text_area` holds.** gooey's area is fixed-height and clipped.
pub const GUIDE_ROWS: usize = 12;

/// **How many characters wide that area is**, from gooey's own `TEXT_AREA_CHARS` at the pinned revision
/// (`widgets.rs:185`, `26.0`). It is copied rather than imported because it is private there, and the copy
/// is what [`FULLY_SHOWN_CHARS`] is measured from — the number a person sees in a guide has to match the
/// widget they are typing in.
pub const GUIDE_COLUMNS: usize = 26;

/// **Draw the Agent guide section**: its status line, then the editor, then the state's own buttons.
///
/// **The section is drawn from [`GuidePage`]**, not from the file again: an unsaved edit lives there, and a
/// redraw that re-read the disk under it would be a redraw that threw the person's typing away.
fn draw_agent_guide(parent: &mut bevy::ecs::relationship::RelatedSpawnerCommands<ChildOf>, held: &GuidePage) {
    let view = &held.view;
    // **The whole file or none of it**, and "the whole file" is counted in the rows the box has, not in
    // characters: the area wraps at [`GUIDE_COLUMNS`] and clips past [`GUIDE_ROWS`], so a short character
    // count can still hide a payload below the fold. Past either bound the section says how much it is
    // showing and offers no Save, no Allow, no Revert and no Trust: writing over a file from a view of part
    // of it is how a guide gets truncated.
    let total = view.text.chars().count();
    let rows = bevy_grimoire_core::agent_guide::visual_rows(&view.text, GUIDE_COLUMNS);
    let truncated = !fully_shown(&view.text);
    // **A file nobody can serve says its problem where the state would be**, because "waiting for you" over
    // a file that is too large or not text would be an offer to allow something unusable.
    let status = if let Some(problem) = &view.problem {
        problem.clone()
    } else if let Some(trouble) = &view.trouble {
        format!("{} · {trouble}", view.state.word())
    } else if truncated {
        // **Said in the box's own terms**: rows against rows, and the character count as the supporting
        // fact. "The first 312 characters" was true of a file whose *thirteenth line* was the invisible one.
        format!(
            "{} · {} · showing the first {GUIDE_ROWS} of {rows} lines ({FULLY_SHOWN_CHARS} characters of \
             {total}); edit the file in your editor, at {}. Nothing here can be saved or allowed until the \
             whole guide fits the box.",
            view.state.word(),
            crate::agent_guide::PROJECT_PATH_SHOWN,
            crate::agent_guide::PROJECT_PATH_SHOWN
        )
    } else if held.conflict {
        format!("changed on disk · {} was changed somewhere else, and your unsaved edits were kept", crate::agent_guide::PROJECT_PATH_SHOWN)
    } else {
        format!("{} · {}", view.state.word(), crate::agent_guide::PROJECT_PATH_SHOWN)
    };
    // **What is shown when it does not all fit**: the whole lines that do, never a character cut through
    // the middle of a line — a prefix ending mid-word reads as a damaged file rather than a partial one.
    let text: String = if truncated {
        view.text
            .lines()
            .scan(0usize, |rows, line| {
                let here = bevy_grimoire_core::agent_guide::visual_rows(line, GUIDE_COLUMNS);
                let fits = *rows + here <= GUIDE_ROWS;
                if fits {
                    *rows += here;
                }
                Some((line, fits))
            })
            .take_while(|(_, fits)| *fits)
            .map(|(line, _)| format!("{line}\n"))
            .collect()
    } else {
        held.editing.clone().unwrap_or_else(|| view.text.clone())
    };
    let state = view.state;
    let problem = view.problem.clone();
    parent.spawn((
        section(
            "Agent guide",
            true,
            Children::spawn(SpawnWith(move |parent: &mut bevy::ecs::relationship::RelatedSpawner<ChildOf>| {
                parent.spawn((bevy_gooey::column(crate::chrome::GAP_TIGHT_STEPS), bevy_gooey::Spacing { padding: bevy_gooey::Sides::y(crate::chrome::STEP_ROW), ..default() })).with_children(|rows| {
                    rows.spawn((label(status.clone()), AgentGuideStatus));
                    // **A file nobody can read is its problem, not an editor**: saving over it would throw
                    // away whatever it is, and the person is owed the reason instead.
                    if let Some(problem) = &problem {
                        rows.spawn(bevy_gooey::toast_alert(problem.clone()));
                        return;
                    }
                    if state == crate::agent_guide::GuideState::Missing {
                        rows.spawn((
                            button("Create guide"),
                            AgentGuideCreate,
                            observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::CreateGuide); }),
                        ));
                        return;
                    }
                    // **A partial view is a label, not a field.** Drawn as a `text_area` it would take a
                    // typist's keystrokes into the *prefix*, and `Keep mine` would then write that prefix
                    // over the whole file — so the partial case shows the same prefix as read-only prose and
                    // offers nothing else. The status line says what is shown and where to edit the rest.
                    if truncated {
                        rows.spawn((label(text.clone()), AgentGuideShown));
                        return;
                    }
                    rows.spawn((
                        bevy_gooey::text_area("The guide", text.clone(), GUIDE_ROWS),
                        AgentGuideText,
                        observe(|change: On<ValueChange<String>>, mut out: MessageWriter<PageAction>| {
                            out.write(PageAction::GuideEdited(change.value.clone()));
                        }),
                    ));
                    // **The state's own buttons are filled by [`fill_the_guide_buttons`]**, not drawn with the
                    // section, so a press of the first key — which flips whether there is an edit and so
                    // whether Allow is offered — changes this row alone and never rebuilds the area under the
                    // person typing in it.
                    rows.spawn((bevy_gooey::row(crate::chrome::GAP_GROUP_STEPS), AgentGuideButtons));
                });
            })),
        ),
    ));
}

/// A button row, what it was last built for and what it holds now.
type ButtonRow = (Entity, Option<&'static ButtonsFor>, Option<&'static Children>);

/// **Fill the Agent guide's button row from what the section holds now**: Reload and Keep mine in a
/// conflict, otherwise Save and Revert, with Allow and Trust beside them while the guide is waiting for
/// the person. Allow is offered only while there is no unsaved edit.
///
/// **This row alone is rebuilt when the state moves** — not the page body. The first key typed flips
/// whether there is an edit, and a rebuild of the body at that moment replaced the area under the typist,
/// so the field lost the keyboard after one character.
fn fill_the_guide_buttons(mut commands: Commands, page: Option<Res<ProjectPage>>, rows: Query<ButtonRow, With<AgentGuideButtons>>) {
    let Some(page) = page else { return };
    let now = ButtonsFor { state: page.guide.view.state, conflict: page.guide.conflict, editing: page.guide.editing.is_some() };
    let ButtonsFor { state, conflict, editing } = now;
    for (row, built, children) in &rows {
        if built == Some(&now) {
            continue;
        }
        for child in children.into_iter().flatten() {
            commands.entity(*child).try_despawn();
        }
        commands.entity(row).try_insert(now).with_children(|row| {
            if conflict {
                // **The person says which text wins.** Neither is thrown away until they do.
                row.spawn((submit_button("Reload"), AgentGuideReload, observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::ReloadGuide); })));
                row.spawn((button("Keep mine"), AgentGuideKeepMine, observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::KeepMyGuide); })));
            } else if state == crate::agent_guide::GuideState::Waiting {
                // **A guide nobody has allowed is read before it is allowed**, and both answers
                // AG-03's modal offers are here, because this is where a person comes back to.
                //
                // **No Allow while there is an unsaved edit**: the edit is text on nobody's disk,
                // and granting it would both allow unread content and drop the edit. Save is the
                // act that writes it, and the grant follows the save.
                if !editing {
                    row.spawn((submit_button("Allow this guide"), AgentGuideAllow, observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::AllowGuide); })));
                }
                row.spawn((button("Trust this project"), AgentGuideTrust, observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::TrustProject); })));
                row.spawn((button("Save"), AgentGuideSave, observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::SaveGuide); })));
                row.spawn((button("Revert"), AgentGuideRevert, observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::RevertGuide); })));
            } else {
                row.spawn((submit_button("Save"), AgentGuideSave, observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::SaveGuide); })));
                row.spawn((button("Revert"), AgentGuideRevert, observe(|_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::RevertGuide); })));
            }
        });
    }
}

/// A map: its picture, name, size in metres and how many pieces stand in it. One that will not open says why and can be deleted.
fn map_card(grid: &mut ChildSpawner, map: &MapEntry, picture: Option<ImageNode>, menu: Entity) {
    let name = map.name.clone();
    match &map.summary {
        MapSummary::Unreadable(why) => {
            let gone = name.clone();
            grid.spawn((launcher::card_layout(), MapCard(name.clone()))).insert(Role::Card).with_children(|card| {
                card.spawn(launcher::quiet(name.clone()));
                card.spawn(label("Will not open"));
                card.spawn(launcher::quiet(launcher::middle_ellipsis(why, 90)));
                card.spawn((button("Delete"), observe(move |_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::DeleteMap(gone.clone())); })));
            });
        }
        MapSummary::Read { placements, stamps, bounds, bash, .. } => {
            let open = name.clone();
            grid.spawn((
                // CHROME-OK: a map card is none of chrome's three clickables, it is a gooey button's box holding a picture (bevy_gooey#41)
                launcher::card_button(),
                ContextMenu(menu),
                MapCard(name.clone()),
                observe(move |_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::OpenMap(open.clone())); }),
            ))
            .with_children(|card| {
                if let Some(picture) = picture {
                    card.spawn(launcher::thumb_box(picture.image.clone())).insert((picture, launcher::card_line()));
                }
                card.spawn((Text::new(name.clone()), Role::ButtonText, launcher::card_line(), Pickable::IGNORE));
                card.spawn((launcher::quiet(size_in_metres(*bounds)), Role::Sized, launcher::card_line(), Pickable::IGNORE));
                let pieces = placements + stamps;
                // Every line but the last leaves `card_line`'s room below it, so the last is the one with nothing after it.
                let (pieces_line, last) = (if pieces == 0 { "Empty".to_owned() } else { format!("{pieces} {} placed", plural(pieces, "piece")) }, bash.is_none());
                if last {
                    card.spawn((launcher::quiet(pieces_line), Pickable::IGNORE));
                } else {
                    card.spawn((launcher::quiet(pieces_line), Role::Sized, launcher::card_line(), Pickable::IGNORE));
                }
                if let Some(bash) = bash {
                    card.spawn((launcher::quiet(format!("Pieces from {bash}")), Pickable::IGNORE));
                }
            });
        }
    }
}

/// A kit's row: its name, how many pieces it holds, and a badge on the one new work lands in.
fn kit_row(rows: &mut ChildSpawner, kit: &Kit, is_default: bool, menu: Entity) {
    let label_text = kit.label.clone();
    let open = kit.label.clone();
    rows.spawn((
        // CHROME-OK: a kit row is none of chrome's three clickables, it is a gooey button's box with a count and a badge: the button's skin pads it and puts its half gap between its
        // parts, as it did the raw node (the gap and padding the node asked for were overwritten).
        bevy_gooey::button_box(),
        bevy_gooey::RowLayout { align: Some(AlignItems::Center), ..default() },
        ContextMenu(menu),
        KitRow(label_text.clone()),
        observe(move |_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::OpenKit(open.clone())); }),
    ))
    .with_children(|row| {
        row.spawn((Text::new(label_text), Role::ButtonText, Pickable::IGNORE));
        row.spawn((launcher::quiet(format!("{} {}", kit.pieces, plural(kit.pieces, "piece"))), Pickable::IGNORE));
        if is_default {
            row.spawn((Text::new("Default for new pieces"), Role::Label, Pickable::IGNORE));
        }
    });
}

/// What a context-menu item of a map or a kit does.
#[derive(Clone, Copy)]
enum Verb {
    Open,
    Rename,
    Source,
    Delete,
    MakeDefault,
    Settings,
    Policy,
}

fn spawn_menus(commands: &mut Commands, map_menu: Entity, kit_menu: Entity, catalog: &Catalog) {
    let mut maps = vec![("Open", Verb::Open), ("Rename", Verb::Rename), ("Map settings…", Verb::Settings)];
    if !catalog.bashes.is_empty() {
        maps.push(("Change where its pieces come from", Verb::Source));
    }
    for (text, verb) in maps {
        commands.spawn((menu_item(text), ChildOf(map_menu))).observe(on_map_verb(verb));
    }
    commands.spawn((separator(), ChildOf(map_menu)));
    commands.spawn((menu_item("Delete"), ChildOf(map_menu))).observe(on_map_verb(Verb::Delete));
    for (text, verb) in [("Open", Verb::Open), ("Make default for new pieces", Verb::MakeDefault), ("Policy…", Verb::Policy)] {
        commands.spawn((menu_item(text), ChildOf(kit_menu))).observe(on_kit_verb(verb));
    }
    commands.spawn((separator(), ChildOf(kit_menu)));
    commands.spawn((menu_item("Delete"), ChildOf(kit_menu))).observe(on_kit_verb(Verb::Delete));
}

#[allow(clippy::type_complexity)]
fn on_map_verb(verb: Verb) -> impl FnMut(On<Activate>, Query<&ChildOf>, Query<&ContextTarget>, Query<&MapCard>, MessageWriter<PageAction>) {
    move |activate, parents, targets, cards, mut out| {
        let Some(MapCard(name)) = ContextTarget::of(activate.entity, &parents, &targets).and_then(|e| cards.get(e).ok()) else { return };
        out.write(match verb {
            Verb::Open => PageAction::OpenMap(name.clone()),
            Verb::Rename => PageAction::RenameMap(name.clone()),
            Verb::Settings => PageAction::MapSettings(name.clone()),
            Verb::Source => PageAction::NextSource(name.clone()),
            Verb::Delete => PageAction::DeleteMap(name.clone()),
            Verb::MakeDefault | Verb::Policy => return,
        });
    }
}

#[allow(clippy::type_complexity)]
fn on_kit_verb(verb: Verb) -> impl FnMut(On<Activate>, Query<&ChildOf>, Query<&ContextTarget>, Query<&KitRow>, MessageWriter<PageAction>) {
    move |activate, parents, targets, rows, mut out| {
        let Some(KitRow(label)) = ContextTarget::of(activate.entity, &parents, &targets).and_then(|e| rows.get(e).ok()) else { return };
        out.write(match verb {
            Verb::Open => PageAction::OpenKit(label.clone()),
            Verb::Delete => PageAction::DeleteKit(label.clone()),
            Verb::MakeDefault => PageAction::MakeDefault(label.clone()),
            Verb::Policy => PageAction::KitPolicy(label.clone()),
            Verb::Rename | Verb::Source | Verb::Settings => return,
        });
    }
}

/// Esc closes a dialog; with none open and no question up it goes back to the grid. It runs before gooey's own Escape handling, so the Esc
/// that closes a dialog is still a dialog's when this reads it.
fn back_on_escape(keys: Res<ButtonInput<KeyCode>>, dialogs: Query<(), With<PageDialog>>, confirm: Option<Res<Confirm>>, mut out: MessageWriter<PageAction>) {
    if keys.just_pressed(KeyCode::Escape) && dialogs.is_empty() && !confirm.is_some_and(|c| c.is_open()) {
        out.write(PageAction::Back);
    }
}

/// Dispatches one operation the way the chooser did: the project's root and no loaded project, the person as the source, the log line in
/// the project's `oplog.jsonl`. A refusal is the sentence the page shows.
pub(crate) fn from_the_page(root: &Path, operation: &str, params: &serde_json::Value) -> Result<serde_json::Value, String> {
    let mut ctx = crate::ops::Ctx { root: Some(root.to_path_buf()), source: crate::ops::Source::Human, session: "project page".to_owned(), ..crate::ops::Ctx::default() };
    match crate::ops::dispatch(&mut ctx, operation, params) {
        Ok(crate::ops::Applied::Done { outcome, .. }) => Ok(outcome.after.unwrap_or(serde_json::Value::Null)),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// Runs `operation` on the map `name`, which the operation needs loaded with its project, and which it saves.
fn on_the_map(root: &Path, name: &str, operation: &str, params: &serde_json::Value) -> Result<(), String> {
    let project = crate::project::Project::open(root, None)?;
    let open = crate::project::OpenMap::open(&project, name)?;
    let mut ctx = crate::ops::Ctx { root: Some(root.to_path_buf()), project: Some(project), open: Some(open), source: crate::ops::Source::Human, session: "project page".to_owned(), ..crate::ops::Ctx::default() };
    match crate::ops::dispatch(&mut ctx, operation, params) {
        Ok(crate::ops::Applied::Done { .. }) => Ok(()),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    }
}

/// The action messages, carried out.
#[allow(clippy::too_many_arguments)]
fn act(
    mut commands: Commands,
    mut actions: MessageReader<PageAction>,
    mut page: ResMut<ProjectPage>,
    confirm: Option<ResMut<Confirm>>,
    mut next: ResMut<NextState<Screen>>,
    form: Option<Res<Form>>,
    mut launcher: Option<ResMut<Launcher>>,
    dialogs: Query<Entity, With<PageDialog>>,
    open: Option<Res<crate::project::OpenMap>>,
) {
    let mut confirm = confirm;
    for action in actions.read().cloned() {
        // **A map the editor has open is not renamed, changed or deleted from under it** (NV-08, #385): the page inside the workbench is
        // the editor's own, and its open map is held in memory.
        if page.in_workspace
            && let PageAction::RenameMap(name) | PageAction::MapSettings(name) | PageAction::DeleteMap(name) | PageAction::NextSource(name) = &action
            && open.as_ref().is_some_and(|o| &o.map.name == name)
        {
            commands.spawn(toast_alert(format!("`{name}` is open in the editor, so it is not changed from here: open another map first.")).sticky());
            continue;
        }
        match action {
            // **The Agent guide's actions are the section's own system's** ([`the_guide_section`]), which
            // reads the same messages with its own cursor. Nothing here touches the guide.
            PageAction::GuideEdited(_)
            | PageAction::SaveGuide
            | PageAction::RevertGuide
            | PageAction::AllowGuide
            | PageAction::TrustProject
            | PageAction::CreateGuide
            | PageAction::ReloadGuide
            | PageAction::KeepMyGuide => {}
            PageAction::Back => {
                // The grid is the project's parent: the page's sidebar entry for it is Projects.
                if let Some(launcher) = launcher.as_mut() {
                    launcher.tab = launcher::HomeTab::Projects;
                }
                next.set(Screen::Launcher);
            }
            PageAction::OpenMap(name) => open_the_map(&mut commands, &page, name),
            PageAction::OpenKit(label) => match page.catalog.kit(&label).map(|k| k.flag.clone()) {
                None => {
                    commands.spawn(toast_alert(format!("there is no kit `{label}`")).sticky());
                }
                Some(None) => {
                    commands.spawn(toast_alert(format!("`{label}` is not a bound kit, so there is nothing to open it as.")).sticky());
                }
                // Inside the workbench the kit's pieces are the Kit workspace's, one switch away: the project stays open.
                // It shows every kit the project holds, not this one alone, and the toast says so.
                Some(Some(_)) if page.in_workspace => {
                    commands.queue(move |world: &mut World| match crate::screen::cross(world, Workspace::Kit) {
                        Ok(()) => {
                            world.spawn(toast(format!("The Kit workspace shows every kit of the project, `{label}` among them")));
                        }
                        Err(e) => {
                            world.spawn(toast_alert(e).sticky());
                        }
                    });
                }
                Some(Some(flag)) => {
                    let argv = vec![page.root.display().to_string(), "--door".to_owned(), crate::workspace::Workspace::Kit.label().to_lowercase(), "--kit".to_owned(), flag];
                    commands.queue(move |world: &mut World| {
                        if let Err(e) = project_assets::open_project_argv(world, &argv) {
                            world.spawn(toast_alert(e).sticky());
                        }
                    });
                }
            },
            PageAction::NewMap => open_dialog(&mut commands, &page, Form::new(FormKind::NewMap, String::new()), form.as_deref()),
            PageAction::NewKit => {
                let first = page.catalog.kits.is_empty();
                open_dialog(&mut commands, &page, Form { default: first, ..Form::new(FormKind::NewKit, String::new()) }, form.as_deref());
            }
            PageAction::RenameMap(name) => open_dialog(&mut commands, &page, Form::new(FormKind::RenameMap(name.clone()), name), form.as_deref()),
            PageAction::MapSettings(name) => match page.catalog.map(&name).map(|m| &m.summary) {
                Some(MapSummary::Read { bounds, origin, note, .. }) => {
                    let (width, height, depth) = *bounds;
                    let settings = Form { metres: [width, depth, height], origin: [origin.0, origin.1, origin.2], note: note.clone().unwrap_or_default(), ..Form::new(FormKind::MapSettings(name.clone()), name) };
                    open_dialog(&mut commands, &page, settings, form.as_deref());
                }
                Some(MapSummary::Unreadable(why)) => {
                    commands.spawn(toast_alert(format!("`{name}` will not open, so it has no settings to change: {why}")).sticky());
                }
                None => {
                    commands.spawn(toast_alert(format!("there is no map `{name}`")).sticky());
                }
            },
            PageAction::KitPolicy(label) => {
                if form.is_some() || !dialogs.is_empty() {
                    continue;
                }
                match page.catalog.kit(&label).ok_or_else(|| format!("there is no kit `{label}`")).and_then(catalog::policy_entries) {
                    Ok(entries) => open_policy_dialog(&mut commands, &label, entries),
                    Err(e) => {
                        commands.spawn(toast_alert(e).sticky());
                    }
                }
            }
            PageAction::RemovePolicy { kit, entry } => {
                for dialog in &dialogs {
                    commands.entity(dialog).despawn();
                }
                if let Some(confirm) = confirm.as_deref_mut() {
                    confirm.ask(
                        Asked::DeleteEntry,
                        format!("Remove this from the policy of `{kit}`?"),
                        format!("{}. Only this line of the kit's policy file goes, and its comments stay.", entry.shown),
                        "Remove it",
                        "Keep it",
                    );
                }
                page.asking_policy = Some((kit, entry));
            }
            PageAction::DeleteMap(name) => {
                let asked = catalog::ask_delete_map(&page.catalog, &name);
                ask_to_delete(&mut commands, &mut page, confirm.as_deref_mut(), asked);
            }
            PageAction::DeleteKit(label) => {
                let asked = catalog::ask_delete_kit(&page.root, &page.catalog, &label);
                ask_to_delete(&mut commands, &mut page, confirm.as_deref_mut(), asked);
            }
            PageAction::MakeDefault(label) => {
                if page.catalog.authoring.as_deref() == Some(label.as_str()) {
                    // Already the one is not a change, and the operation says so by refusing; there is nothing to report.
                    continue;
                }
                match from_the_page(&page.root, "kit_set_authoring", &serde_json::json!({ "kit": label })) {
                    Ok(_) => page.dirty = true,
                    Err(e) => {
                        commands.spawn(toast_alert(e).sticky());
                    }
                }
            }
            PageAction::NextSource(name) => match catalog::cycle_bash(&page.catalog, &name) {
                Ok(_) => page.dirty = true,
                Err(e) => {
                    commands.spawn(toast_alert(e).sticky());
                }
            },
        }
    }
}

/// **Open the map `name`**: from the home, the project is opened on it; from the Project workspace, the project is already open, so the map
/// becomes the live one as a tab does (`session::choose_map`, which asks first about unsaved edits) and the Map workspace is shown.
fn open_the_map(commands: &mut Commands, page: &ProjectPage, name: String) {
    if page.in_workspace {
        commands.queue(move |world: &mut World| {
            // **Whatever the editor has open**: another map is switched to as its tab is (`choose_map`, which asks first when the open map has
            // unsaved edits, and does nothing when this one is already open); an editor with NO map open, as one opened on a kit is, has
            // nothing to ask about and opens this one (`open_map_now`: `choose_map` returns at once with no `OpenMap`).
            let opened = if world.get_resource::<crate::project::OpenMap>().is_some() {
                crate::session::choose_map(world, &name);
                // **A question raised is answered where it was asked**: crossing to the Map workspace now would rebuild the editor's state
                // and take the held request with it, so "Save and go" would go nowhere. The answer opens the map.
                if world.get_resource::<crate::editor::EditorState>().is_some_and(|s| s.map_request.is_some()) {
                    return;
                }
                Ok(())
            } else {
                crate::session::open_map_now(world, &name)
            };
            match opened.and_then(|()| crate::screen::cross(world, Workspace::Map)) {
                Ok(()) => {}
                Err(e) => {
                    world.spawn(toast_alert(e).sticky());
                }
            }
        });
    } else {
        let root = page.root.clone();
        commands.queue(move |world: &mut World| {
            if let Err(e) = project_assets::open_project(world, &root, Some(&name)) {
                world.spawn(toast_alert(e).sticky());
            }
        });
    }
}

/// Raises the delete question for `asked`, or says why it cannot be.
fn ask_to_delete(commands: &mut Commands, page: &mut ProjectPage, confirm: Option<&mut Confirm>, asked: Result<Pending, String>) {
    match asked {
        Err(e) => {
            commands.spawn(toast_alert(e).sticky());
        }
        Ok(pending) => {
            let what = if pending.kit { "kit" } else { "map" };
            if let Some(confirm) = confirm {
                confirm.ask(
                    Asked::DeleteEntry,
                    format!("Delete the {what} `{}`?", pending.name),
                    if pending.kit { "The whole kit directory goes, with everything in it. This cannot be undone." } else { "The map file is removed. This cannot be undone." },
                    format!("Delete {what} `{}`", pending.name),
                    "Keep it",
                );
            }
            page.asking = Some(pending);
        }
    }
}

/// The answer to the delete question: the thing it named goes, or nothing does. A policy entry's removal is answered here too, and the
/// policy dialog comes back with what is left.
fn answer_the_question(mut commands: Commands, mut page: ResMut<ProjectPage>, confirm: Option<ResMut<Confirm>>, mut out: MessageWriter<PageAction>) {
    let Some(mut confirm) = confirm else { return };
    let Some(agreed) = confirm.answer(Asked::DeleteEntry) else { return };
    if let Some((label, entry)) = page.asking_policy.take() {
        if agreed {
            let removed = page.catalog.kit(&label).ok_or_else(|| format!("there is no kit `{label}`")).and_then(|kit| catalog::remove_policy_entry(kit, &entry.row));
            if let Err(e) = removed {
                commands.spawn(toast_alert(e).sticky());
            }
        }
        out.write(PageAction::KitPolicy(label));
        return;
    }
    let Some(pending) = page.asking.take() else { return };
    if !agreed {
        return;
    }
    match catalog::delete_entry(&page.root, &pending) {
        Ok(()) => {
            page.dirty = true;
            commands.spawn(toast(format!("Deleted `{}`", pending.name)));
        }
        Err(e) => {
            commands.spawn(toast_alert(e).sticky());
        }
    }
}

/// **Read the guide from disk** into the page's held state, before it draws.
///
/// **Read every [`GUIDE_READ_SECS`]**, not only when the page redraws: a person editing the guide in their
/// own text editor is the case this section exists for, and a section that showed a stale copy until the
/// maps' directory moved would be one that quietly disagreed with the file.
///
/// **An unsaved edit survives the read**: the text drawn is `editing` while there is one, and the disk's
/// text is what the edit is compared against. **A file that moved under an unsaved edit is a conflict**,
/// reported by the status line, and the file is only written by [`the_guide_section`], which refuses while
/// a conflict stands.
const GUIDE_READ_SECS: f32 = 0.5;

fn read_the_guide(
    mut page: ResMut<ProjectPage>,
    state: Option<Res<crate::editor::EditorState>>,
    store_root: Option<Res<crate::trusted::StoreRoot>>,
    real: Res<Time<Real>>,
    mut since: Local<f32>,
) {
    *since += real.delta_secs();
    // **Every time the page is about to redraw, and otherwise each [`GUIDE_READ_SECS`].** A redraw the
    // section itself asked for must see the file it asked about, so the read cannot wait for a clock the
    // next frame may not have reached.
    if !page.dirty && *since < GUIDE_READ_SECS {
        return;
    }
    *since = 0.0;
    let trust_file = state.and_then(|s| s.trust.file.clone());
    let view = agent_guide_view(&page, store_root.as_deref(), trust_file.as_deref());
    let disk = view.text.clone();
    let mut moved = false;
    // **A standing conflict is not re-read away.** It is what the person is deciding between, and the only
    // two things that settle it are **Reload** and **Keep mine** — a read loop that re-read the disk and
    // cleared the flag would erase the state one frame after it was raised, and the buttons that resolve it
    // with it.
    if page.guide.conflict {
        // Nothing to update: show the state as it was found, and let the person choose.
    } else if page.guide.dirty() {
        // **The person is typing.** A file that changed under them is a conflict, never a silent overwrite.
        if disk != page.guide.on_disk {
            page.guide.on_disk = disk;
            page.guide.conflict = true;
            moved = true;
        }
    } else if disk != page.guide.on_disk {
        page.guide.on_disk = disk;
        moved = true;
    }
    if page.guide.view != view {
        page.guide.view = view;
        moved = true;
    }
    if moved {
        page.dirty = true;
    }
}

/// **The Agent guide section's own actions** (AG-05, #350), carried out.
///
/// **Save is not an operation.** It is the person's own act on the page, like a trust grant: the guide is
/// what tells an agent what to do, so an agent that could write it could instruct itself.
///
/// **A file that changed on disk while there were unsaved edits is never overwritten.** The section says
/// so, and **Reload** (take the disk's text) or **Keep mine** (write the person's) is how the two are
/// settled — the rule `a_file_that_does_not_read_is_never_overwritten` states for every other file the
/// editor writes, applied to the one file a person types into directly.
fn the_guide_section(
    mut commands: Commands,
    mut actions: MessageReader<PageAction>,
    mut page: ResMut<ProjectPage>,
    state: Option<ResMut<crate::editor::EditorState>>,
    store_root: Option<Res<crate::trusted::StoreRoot>>,
) {
    for action in actions.read().cloned() {
        let guide = crate::agent_guide::project_file(&page.root);
        // **The person's own store when none is named** (`crate::trusted::store_path`), the same resolution
        // the guide the agent is served uses, so an Allow here is an Allow there.
        let store = state.as_deref().and_then(|s| s.trust.file.clone());
        let root_res = store_root.as_deref();
        match action {
            PageAction::GuideEdited(text) => {
                // **An edit equal to the file is not an edit.** A redraw re-draws the area with the file's
                // text, and that arrival must not count as unsaved work.
                page.guide.editing = (text != page.guide.on_disk).then_some(text);
                // **No redraw here, ever**: the buttons that change with it (Allow goes while there is an
                // edit) are refilled in place by [`fill_the_guide_buttons`], because a rebuilt area is a
                // field that has lost the keyboard under the person typing in it.
            }
            PageAction::RevertGuide => {
                page.guide.editing = None;
                page.dirty = true;
            }
            PageAction::ReloadGuide => match read_guide(&guide) {
                Ok(disk) => {
                    page.guide.on_disk = disk;
                    page.guide.editing = None;
                    page.guide.conflict = false;
                    page.dirty = true;
                }
                Err(e) => toast_failed(&mut commands, e),
            },
            PageAction::KeepMyGuide => {
                let Some(text) = page.guide.editing.clone() else { continue };
                match write_and_grant(&guide, &page.root, &text, root_res, store.as_deref()) {
                    Ok(()) => {
                        page.guide.on_disk = text;
                        page.guide.editing = None;
                        page.guide.conflict = false;
                        page.dirty = true;
                        toast_done(&mut commands, "Kept your guide");
                    }
                    Err(e) => toast_failed(&mut commands, e),
                }
            }
            PageAction::SaveGuide => {
                let Some(text) = page.guide.editing.clone() else { continue };
                // **A conflict is not written over**, whatever the edit says: the person chooses Reload or
                // Keep mine first, and that choice is the whole of this state.
                if page.guide.conflict {
                    toast_failed(&mut commands, format!("{} changed on disk while you were editing, so nothing was written. Choose Reload or Keep mine.", guide.display()));
                    continue;
                }
                // **The file moved under the edit**: say so and write nothing. The two buttons on the card
                // are the choice; a save here would throw one of the two texts away.
                let disk = match read_guide(&guide) {
                    Ok(disk) => disk,
                    Err(e) => {
                        toast_failed(&mut commands, e);
                        continue;
                    }
                };
                if disk != page.guide.on_disk {
                    page.guide.on_disk = disk;
                    page.guide.conflict = true;
                    page.dirty = true;
                    toast_failed(
                        &mut commands,
                        format!("{} was changed on disk while you were editing, so nothing was written. Reload it or keep yours.", guide.display()),
                    );
                    continue;
                }
                match write_and_grant(&guide, &page.root, &text, root_res, store.as_deref()) {
                    Ok(()) => {
                        page.guide.on_disk = text;
                        page.guide.editing = None;
                        page.guide.conflict = false;
                        page.dirty = true;
                        toast_done(&mut commands, "Saved this project's guide");
                    }
                    Err(e) => toast_failed(&mut commands, e),
                }
            }
            // **Allow this guide, and only the text the person was shown.**
            //
            // Two ways this could grant text nobody read, and both are refused:
            // - **An unsaved edit** is text that is on nobody's disk, and granting it would also drop the
            //   edit: Save is the act that writes it, and the grant follows the save.
            // - **A file that moved between the draw and the click** — the time-of-check/time-of-use hole —
            //   would give whoever wrote it a grant for text the person never saw. So `on_disk` is granted
            //   only while the file still holds it; otherwise the conflict is raised and nothing is granted.
            PageAction::AllowGuide => {
                if let Some(_editing) = &page.guide.editing {
                    toast_failed(&mut commands, "Save the edit first: Allow grants the file, and this text is not written yet.");
                    continue;
                }
                let disk = match read_guide(&guide) {
                    Ok(disk) => disk,
                    Err(e) => {
                        toast_failed(&mut commands, e);
                        continue;
                    }
                };
                if disk != page.guide.on_disk {
                    page.guide.on_disk = disk;
                    page.guide.conflict = true;
                    page.dirty = true;
                    toast_failed(
                        &mut commands,
                        format!("{} was changed on disk since the page read it, so nothing was allowed. Reload it, read it, and allow it then.", guide.display()),
                    );
                    continue;
                }
                match grant_in_store(root_res, store.as_deref(), &page.root, &page.guide.on_disk) {
                    Ok(()) => {
                        page.dirty = true;
                        toast_done(&mut commands, "Allowed this guide");
                    }
                    Err(e) => toast_failed(&mut commands, e),
                }
            }
            PageAction::TrustProject => match trust_project_in_store(root_res, store.as_deref(), &page.root) {
                Ok(()) => {
                    page.dirty = true;
                    toast_done(&mut commands, "Trusted this project, so its guides are served");
                }
                Err(e) => toast_failed(&mut commands, e),
            },
            PageAction::CreateGuide => match crate::guide_template::write(&page.root, root_res, store.as_deref()) {
                Ok(file) => {
                    page.guide.on_disk = crate::guide_template::TEMPLATE.to_owned();
                    page.guide.editing = None;
                    page.guide.conflict = false;
                    page.dirty = true;
                    toast_done(&mut commands, format!("Wrote {}", file.display()));
                }
                Err(e) => toast_failed(&mut commands, e),
            },
            _ => {}
        }
    }
}

/// **The guide on disk, read the one bounded way**, or the refusal that says why it cannot be.
///
/// **`std::fs::read_to_string` is not used anywhere in this section.** A guide path arrives with a cloned
/// project, so a FIFO or a `/dev/zero` symlink would block or grow without end inside a UI system — the same
/// hole AG-02's reader closed, in the paths this page writes through (`SaveGuide`, `ReloadGuide`,
/// `AllowGuide`). The core reader refuses a non-regular file and reads at most `GUIDE_MAX_BYTES + 1`; a file
/// that is not there is an `Err` here, because every caller is acting on a file the section believes exists.
pub(crate) fn read_guide(path: &Path) -> Result<String, String> {
    match bevy_grimoire_core::agent_guide::read_layer(bevy_grimoire_core::agent_guide::Source::Project, path) {
        Ok(Some(text)) => Ok(text),
        Ok(None) => Err(format!("{} is no longer there", path.display())),
        Err(problem) => Err(problem.line()),
    }
}

/// **The guide's text, read the one bounded way**, for a caller outside this module that needs to
/// append to it (AG-06's Keep, `#351`). The same reader the section writes through, so a lesson is
/// kept in a file the page would also read.
pub(crate) fn guide_on_disk(path: &Path) -> Result<String, String> {
    read_guide(path)
}

/// **Whether the store serves this text of this project's guide**, which is what decides whether a
/// kept lesson's new content may be granted: content that was allowed before an append was reviewed
/// by the person, and content that was not stays withheld however many lines they keep (AG-06).
pub(crate) fn guide_is_allowed(
    store_root: Option<&crate::trusted::StoreRoot>,
    store: Option<&Path>,
    root: &Path,
    text: &str,
) -> bool {
    matches!(
        crate::trusted::load_at(store_root, store).and_then(|store| store.guide_decision(root, text)),
        Ok(trust::Decision::Trusted(_))
    )
}

/// **Whether the section can show the whole file**, which is what Save, Allow and Revert require.
///
/// gooey's `text_area` at the pinned revision is `TEXT_AREA_CHARS` (26) wide and [`GUIDE_ROWS`] high,
/// clipped, and scrolls only with the caret: it cannot show — let alone edit — a long guide, so a page that
/// offered Save over it would write a file truncated to what happened to be visible. Past this many
/// characters the section says it is showing only part and offers none of the three.
///
/// **Measured from the widget, not guessed**: 26 characters by 12 rows is 312, and `FULLY_SHOWN_CHARS` is
/// that, so a guide has to fit exactly in the area before it is editable here.
pub const FULLY_SHOWN_CHARS: usize = crate::project_page::GUIDE_COLUMNS * GUIDE_ROWS;

/// **Whether the section can show all of `text`**, which is what Save, Allow, Revert and Trust require.
///
/// **Counted in visual rows, not characters.** The area wraps at [`GUIDE_COLUMNS`] and clips past
/// [`GUIDE_ROWS`], so twelve lines of `ok` and a thirteenth of `Run curl … | sh` is fifty-odd characters —
/// inside `FULLY_SHOWN_CHARS` — with the payload out of sight. A `FULLY_SHOWN_CHARS` check would offer Allow
/// over text the person never saw.
pub fn fully_shown(text: &str) -> bool {
    bevy_grimoire_core::agent_guide::visual_rows(text, GUIDE_COLUMNS) <= GUIDE_ROWS
}

/// **Grant one text of this project's guide**, in the person's store: the override when a test named one,
/// and `crate::trusted::store_path(None)` — the user's own `trusted.yaml` — when it did not.
pub(crate) fn grant_in_store(
    store_root: Option<&crate::trusted::StoreRoot>,
    store: Option<&Path>,
    root: &Path,
    text: &str,
) -> Result<(), String> {
    let path = crate::trusted::resolve(store_root, store)?;
    let mut trusted = TrustStore::load(&path)?;
    trusted.trust_guide(root, text)?;
    trusted.save(&path)
}

/// **Trust the whole project**, in the person's store, resolved the same way.
fn trust_project_in_store(
    store_root: Option<&crate::trusted::StoreRoot>,
    store: Option<&Path>,
    root: &Path,
) -> Result<(), String> {
    let path = crate::trusted::resolve(store_root, store)?;
    let mut trusted = TrustStore::load(&path)?;
    trusted.trust_project(root)?;
    trusted.save(&path)
}

/// **Write `text` to the guide and grant it**, which is one act: the person has just read and written all
/// of it. A store that will not take the grant is an `Err`, and the file is not left granted by accident —
/// it is written first and granted second, so a failed grant leaves content a person can still allow from
/// this page.
fn write_and_grant(
    guide: &Path,
    root: &Path,
    text: &str,
    store_root: Option<&crate::trusted::StoreRoot>,
    store: Option<&Path>,
) -> Result<(), String> {
    bevy_grimoire_core::atomic_file::write(guide, text).map_err(|e| format!("{}: {e}", guide.display()))?;
    grant_in_store(store_root, store, root, text)
}

fn toast_done(commands: &mut Commands, text: impl Into<String>) {
    commands.spawn(toast(text.into()));
}

fn toast_failed(commands: &mut Commands, text: impl Into<String>) {
    commands.spawn(toast_alert(text.into()).sticky());
}

/// Looks at the project's files once a second: a map or a kit made, renamed or deleted elsewhere draws.
fn follow_the_files(time: Res<Time<Real>>, mut page: ResMut<ProjectPage>) {
    page.since += time.delta_secs();
    if page.since < WATCH_SECS {
        return;
    }
    page.since = 0.0;
    let now = fingerprint(&page.root);
    if now != page.seen {
        page.seen = now;
        page.dirty = true;
    }
}

/// Draws the body again from disk after anything changed.
#[allow(clippy::too_many_arguments)]
fn redraw(
    mut commands: Commands,
    mut page: ResMut<ProjectPage>,
    body: Query<Entity, With<PageBody>>,
    map_menu: Query<Entity, With<MapMenu>>,
    kit_menu: Query<Entity, With<KitMenu>>,
    splash: Option<Res<SplashArt>>,
    mut images: ResMut<Assets<Image>>,
    pictures: Option<ResMut<ProjectPictures>>,
) {
    if !page.dirty {
        return;
    }
    page.dirty = false;
    let (Ok(body), Ok(map_menu), Ok(kit_menu)) = (body.single(), map_menu.single(), kit_menu.single()) else { return };
    match Catalog::scan(&page.root) {
        Ok(catalog) => page.catalog = catalog,
        Err(e) => {
            commands.spawn(toast_alert(e).sticky());
            return;
        }
    }
    page.seen = fingerprint(&page.root);
    commands.entity(body).despawn_children();
    let mut own_pictures = ProjectPictures::default();
    let mut pictures = pictures;
    let pictures = pictures.as_deref_mut().unwrap_or(&mut own_pictures);
    draw_body(&mut commands, body, &page, map_menu, kit_menu, &mut images, splash.as_deref().and_then(|s| s.0.clone()), pictures);
}

// ── the dialogs ──────────────────────────────────────────────────────────────────────────────────

/// Opens the dialog for `form`, unless one is already up.
fn open_dialog(commands: &mut Commands, page: &ProjectPage, form: Form, existing: Option<&Form>) {
    if existing.is_some() {
        return;
    }
    let (title, accent) = match &form.kind {
        FormKind::NewMap => ("New map", "Create"),
        FormKind::NewKit => ("New kit", "Create"),
        FormKind::RenameMap(_) => ("Rename the map", "Rename"),
        FormKind::MapSettings(_) => ("Map settings", "Save"),
    };
    let bashes: Vec<String> = std::iter::once("Every kit".to_owned()).chain(page.catalog.bashes.iter().cloned()).collect();
    let (kind, first, make_default) = (form.kind.clone(), form.name.clone(), form.default);
    let (metres, origin, note) = (form.metres, form.origin, form.note.clone());
    commands.insert_resource(form);
    let dialog = commands.spawn((dialog(), PageDialog, observe(close_the_dialog))).id();
    let card = commands.spawn((panel(), ChildOf(dialog))).id();
    commands.entity(card).with_children(|card| {
        card.spawn(heading(title));
        if let FormKind::MapSettings(name) = &kind {
            // A map's name is changed by Rename, which also moves its file; here it is only what the settings are of.
            card.spawn(label(format!("Map: {name}")));
        } else {
            card.spawn((
                text_input("Name", first),
                NameField,
                bevy_gooey::TextHint("a name".into()),
                observe(|change: On<ValueChange<String>>, mut form: ResMut<Form>| form.name = change.value.clone()),
            ));
            card.spawn((label(""), SavedAs));
        }
        if matches!(kind, FormKind::NewMap | FormKind::MapSettings(_)) {
            for (axis, name) in ["Width", "Depth", "Height"].into_iter().enumerate() {
                card.spawn((
                    number_field(format!("{name} (metres)"), metres[axis], METRES_STEP, METRES_MIN..=METRES_MAX),
                    SizeField(axis),
                    observe(move |change: On<ValueChange<f32>>, mut form: ResMut<Form>| form.metres[axis] = change.value),
                ));
            }
        }
        if kind == FormKind::NewMap {
            card.spawn((choice("Uses pieces from", bashes, 0), observe(|change: On<ValueChange<usize>>, mut form: ResMut<Form>| form.source = change.value)));
        }
        if matches!(kind, FormKind::MapSettings(_)) {
            for (axis, name) in ["x", "y (height)", "z"].into_iter().enumerate() {
                card.spawn((
                    number_field(format!("Origin {name} (metres)"), origin[axis], METRES_STEP, -METRES_MAX..=METRES_MAX),
                    OriginField(axis),
                    observe(move |change: On<ValueChange<f32>>, mut form: ResMut<Form>| form.origin[axis] = change.value),
                ));
            }
            card.spawn((text_input("Note", note), NoteField, bevy_gooey::TextHint("what this map is for".into()), observe(|change: On<ValueChange<String>>, mut form: ResMut<Form>| form.note = change.value.clone())));
        }
        if kind == FormKind::NewKit {
            card.spawn((toggle("Make it the default for new pieces", make_default), observe(|change: On<ValueChange<bool>>, mut form: ResMut<Form>| form.default = change.value)));
        }
        crate::chrome::end_row(card, crate::chrome::GAP_GROUP_STEPS).with_children(|row| {
            row.spawn((button("Cancel"), observe(|_: On<Activate>, mut commands: Commands, dialogs: Query<Entity, With<PageDialog>>| close(&mut commands, &dialogs))));
            row.spawn((submit_button(accent), bevy_gooey::DefaultButton, CreateButton, observe(create_clicked)));
        });
    });
}

/// The policy dialog: what the kit `kit_label` leaves out and changes, one row each with a Remove.
fn open_policy_dialog(commands: &mut Commands, kit_label: &str, entries: Vec<PolicyEntry>) {
    let dialog = commands.spawn((dialog(), PageDialog, observe(close_the_dialog))).id();
    let card = commands.spawn((panel(), ChildOf(dialog))).id();
    commands.entity(card).with_children(|card| {
        card.spawn(heading(format!("Policy of {kit_label}")));
        if entries.is_empty() {
            card.spawn(label("This kit leaves nothing out and changes nothing."));
        }
        for entry in entries {
            let kit = kit_label.to_owned();
            card.spawn(bevy_gooey::row_with(crate::chrome::GAP_GROUP_STEPS, bevy_gooey::RowLayout { align: Some(AlignItems::Center), justify: Some(JustifyContent::SpaceBetween), ..default() })).with_children(|row| {
                row.spawn(label(entry.shown.clone()));
                row.spawn((button("Remove"), observe(move |_: On<Activate>, mut out: MessageWriter<PageAction>| { out.write(PageAction::RemovePolicy { kit: kit.clone(), entry: entry.clone() }); })));
            });
        }
        crate::chrome::end_row(card, 0.0).with_children(|row| {
            row.spawn((submit_button("Close"), observe(|_: On<Activate>, mut commands: Commands, dialogs: Query<Entity, With<PageDialog>>| close(&mut commands, &dialogs))));
        });
    });
}

/// Esc, or the pad's B, closes the dialog.
fn close_the_dialog(cancel: On<Cancel>, mut commands: Commands) {
    commands.entity(cancel.entity).despawn();
    commands.remove_resource::<Form>();
}

fn close(commands: &mut Commands, dialogs: &Query<Entity, With<PageDialog>>) {
    for dialog in dialogs {
        commands.entity(dialog).despawn();
    }
    commands.remove_resource::<Form>();
}

/// What the form says is wrong, or `None`: an empty name is not wrong yet (nothing has been typed), but is not ready either ([`ready`]).
fn problem(form: &Form, catalog: &Catalog) -> Option<String> {
    let saved = bevy_grimoire_core::naming::to_snake_case(&form.name);
    if saved.is_empty() {
        return (!form.name.trim().is_empty()).then(|| "That leaves nothing usable: use letters or digits.".to_owned());
    }
    match &form.kind {
        FormKind::NewMap if catalog.map(&saved).is_some() => Some(format!("A map called `{saved}` is already here.")),
        FormKind::NewKit if catalog.kit(&saved).is_some() => Some(format!("A kit called `{saved}` is already here.")),
        FormKind::RenameMap(old) if *old == saved => Some("That is already its name.".to_owned()),
        FormKind::RenameMap(_) if catalog.map(&saved).is_some() => Some(format!("A map called `{saved}` is already here.")),
        FormKind::NewMap | FormKind::MapSettings(_) if form.metres.iter().any(|m| !m.is_finite() || *m <= 0.0) => Some("Every size needs to be more than zero.".to_owned()),
        _ => None,
    }
}

/// Whether Create can be pressed.
fn ready(form: &Form, catalog: &Catalog) -> bool {
    !bevy_grimoire_core::naming::to_snake_case(&form.name).is_empty() && problem(form, catalog).is_none()
}

/// What a submitted form leads to.
enum Done {
    /// The map was made: open it.
    OpenMap(String),
    /// Something changed on disk: draw the page again.
    Refresh,
}

/// Does what the form asks, through the operations; the page is still the screen before a door.
fn submit(form: &Form, page: &ProjectPage) -> Result<Done, String> {
    if !ready(form, &page.catalog) {
        return Err(problem(form, &page.catalog).unwrap_or_else(|| "Name it first.".to_owned()));
    }
    let saved = bevy_grimoire_core::naming::to_snake_case(&form.name);
    match &form.kind {
        FormKind::NewMap => {
            let [width, depth, height] = form.metres;
            let origin = bevy_grimoire_core::map::Map::default().origin;
            let after = from_the_page(&page.root, "map_create", &serde_json::json!({ "name": form.name, "bounds": [width, height, depth], "origin": [origin.0, origin.1, origin.2], "note": null }))?;
            let path = after["path"].as_str().map(PathBuf::from).ok_or_else(|| "the map was made but its path was not answered".to_owned())?;
            if let Some(bash) = form.source.checked_sub(1).and_then(|i| page.catalog.bashes.get(i)) {
                catalog::write_bash(&path, Some(bash))?;
            }
            Ok(Done::OpenMap(saved))
        }
        FormKind::NewKit => {
            from_the_page(&page.root, "kit_create", &serde_json::json!({ "name": form.name }))?;
            // The first kit made is where work lands by itself; any other becomes it only when asked.
            if form.default && !page.catalog.kits.is_empty() {
                from_the_page(&page.root, "kit_set_authoring", &serde_json::json!({ "kit": saved }))?;
            }
            Ok(Done::Refresh)
        }
        FormKind::RenameMap(old) => {
            on_the_map(&page.root, old, "map_rename", &serde_json::json!({ "name": saved }))?;
            Ok(Done::Refresh)
        }
        FormKind::MapSettings(name) => {
            let [width, depth, height] = form.metres;
            let [x, y, z] = form.origin;
            // Cleared when empty: a note that is only spaces is no note.
            let note = Some(form.note.trim()).filter(|n| !n.is_empty());
            on_the_map(&page.root, name, "map_set_settings", &serde_json::json!({ "bounds": [width, height, depth], "origin": [x, y, z], "note": note }))?;
            Ok(Done::Refresh)
        }
    }
}

fn create_clicked(_: On<Activate>, mut commands: Commands, form: Option<Res<Form>>, mut page: ResMut<ProjectPage>, dialogs: Query<Entity, With<PageDialog>>) {
    let Some(form) = form else { return };
    match submit(&form, &page) {
        Ok(done) => {
            close(&mut commands, &dialogs);
            page.dirty = true;
            if let Done::OpenMap(name) = done {
                open_the_map(&mut commands, &page, name);
            }
        }
        Err(e) => {
            commands.spawn(toast_alert(e).sticky());
        }
    }
}

/// Puts the form's words on the dialog every time it changes: what the name is saved as, what is wrong with it, and whether Create is ready.
fn show_the_form(
    mut commands: Commands,
    form: Option<Res<Form>>,
    page: Res<ProjectPage>,
    name: Query<Entity, With<NameField>>,
    create: Query<Entity, With<CreateButton>>,
    mut saved_as: Query<&mut Text, With<SavedAs>>,
) {
    let Some(form) = form.filter(|f| f.is_changed()) else { return };
    let saved = bevy_grimoire_core::naming::to_snake_case(&form.name);
    let problem = problem(&form, &page.catalog);
    for entity in &name {
        match &problem {
            Some(text) => {
                commands.entity(entity).insert(FieldProblem::error(text.clone()));
            }
            None => {
                commands.entity(entity).remove::<FieldProblem>();
            }
        }
    }
    let ready = ready(&form, &page.catalog);
    for entity in &create {
        if ready {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else {
            commands.entity(entity).insert(InteractionDisabled);
        }
    }
    let said = if saved.is_empty() { String::new() } else { format!("Saved as {saved}") };
    for mut text in &mut saved_as {
        if text.0 != said {
            text.0.clone_from(&said);
        }
    }
}
