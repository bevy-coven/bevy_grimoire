//! **The Design workspace: the game's style, the rendered page and the place's board** (SG-05, #404; the series #399).
//!
//! `Cmd`/`Ctrl`+`0` opens a tenth workspace in three parts:
//!
//! - **On the left, the design tree** ([`bevy_grimoire_core::design::Tree`], SG-01): the game, its regions, levels and areas, each with its
//!   topics as children, and a count of problems on any place that has one. A click selects the place's page (its `style.md`) or the topic.
//! - **In the middle, the selected file, rendered** by gooey's `markdown_view` with its pictures in place. **Edit** swaps the page for the
//!   file editor (GD-11, with SG-04's Markdown support) in the same place; **Done** saves it and renders it again. A link in the page to
//!   another design file opens it here, one to any other project file goes through the cross-links, and a web link asks the person before
//!   the system browser is started.
//! - **On the right, the place's dream board** ([`bevy_grimoire_core::board`], SG-03) as gooey's card grid: each picture with its caption,
//!   its licence line, a *Restricted* line when it may not ship and a *Missing on this machine* line when its file is not here.
//!
//! # One way to arrive
//!
//! [`show_file`] is the only thing that selects a page, and every road ends in it: the reveal (`reveal::page_of` answers a `Shown::File`
//! under `design/` with this workspace), Go to anything, Back and Forward, a cross-link, the Map inspector's Style link and an agent's edit.
//!
//! # As an agent reads it
//!
//! The toggle above the page renders the selected place's whole chain through [`bevy_grimoire_core::style::compose_place`], which ends in the
//! same `compose_chain` `grimoire_style` does, with the trust decisions the editor's own store gives. The files the person has not allowed
//! read as the one line an agent reads in their place.
//!
//! # A person's edit is not an operation
//!
//! As in the Files workspace, **Done** writes the file as an outside editor would (`files::save_file`), so the watchers read the change as they
//! read any other. A file that changes on disk while Edit holds unsaved typing asks **Reload** or **Keep mine** and is never overwritten.
//!
//! # Pictures
//!
//! A page's pictures and the board's cards are loaded by [`DesignPictures`], confined to the project (a path that leaves it, a link that
//! resolves outside it, a web address and anything that is not a PNG or JPEG are not loaded; the page draws the box gooey draws for a
//! missing one). Each file is decoded once and again only when its size or modification time moves.
//!
//! Spawned on demand and despawned with its workspace, never hidden: the editor's protected tests count every button and take the first
//! search field of the whole world.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::picking::Pickable;
use bevy::picking::events::PointerClick;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};
use bevy::ui_widgets::{Activate, ValueChange};
use bevy_gooey::{
    Card, CardSelection, MarkdownBase, MarkdownLink, MarkdownOutline, MarkdownPicture, MarkdownView, MarkdownViewport, SelectMode, Table, TableSelection,
    TreeNode, markdown_view, observe, scroll_to_heading, tree,
};
use bevy_grimoire_core::board::{self, Board};
use bevy_grimoire_core::design::{self, Place, PlaceKind};
use bevy_grimoire_core::style;
use bevy_grimoire_core::trust::Decision;

use crate::chrome::{self, Frame, FrameSystems, Side};
use crate::editor::EditorState;
use crate::files::{self, Files};
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::workspace::Workspace;

/// **How often the design tree, the selected page and its board are read again, in seconds.** Half a second is "at once" to a person, and the
/// tree is a few small files (the reader refuses one over [`design::STYLE_FILE_MAX_BYTES`]).
pub const REFRESH_S: f32 = 0.5;

/// The width of the tree's panel, in logical pixels.
pub const TREE_PANEL_W: f32 = 300.0;

/// The width of the board's panel, in logical pixels: two cards of the grid at the skin's size.
pub const BOARD_PANEL_W: f32 = 380.0;

/// **The largest picture the page and the board decode**, in bytes (8 MiB): a photograph a person pinned is a few MB, and past this it is a file a
/// page should not load whole on the UI thread.
pub const PICTURE_MAX_BYTES: u64 = 8 * 1024 * 1024;

/// **The most pixels a picture may have to be decoded**, 16,777,216 (4096 x 4096): a small file can hold a very large image (a decompression bomb in a
/// cloned project), so the header's own size is read first ([`bevy_grimoire_core::image_header`]) and a picture over this is not decoded.
pub const PICTURE_MAX_PIXELS: u64 = 4096 * 4096;

/// **The game's own page**, which the Design workspace opens on and **Create style** writes.
pub const GAME_PAGE: &str = "design/style.md";

/// What the status line says when a web link is chosen and the person has not answered.
const OPEN_ADDRESS_TITLE: &str = "Open this address in your browser?";

// ── What is a page ───────────────────────────────────────────────────────────

/// **Whether a project-relative path is a page of the game's style**: a Markdown file under `design/`, by a path that stays inside the project.
pub fn is_page(path: &str) -> bool {
    let path = path.trim();
    path.starts_with("design/")
        && !path.split('/').any(|part| part == ".." || part.is_empty())
        && Path::new(path).extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(design::MARKDOWN_EXT))
}

/// **The words a place is called**: the game, or the folder's own name (a region, a map, an area).
pub fn place_label(place: &Place) -> String {
    match place.kind {
        PlaceKind::Game => "The game".to_owned(),
        _ => place.path.rsplit('/').next().unwrap_or(&place.path).to_owned(),
    }
}

/// **The folder of a page**: `design/levels/m/dialogue.md` is in `design/levels/m`.
fn place_of(page: &str) -> &str {
    page.rsplit_once('/').map_or(design::DESIGN_DIR, |(folder, _)| folder)
}

/// **The topic a page is**: its file's name without `.md` (`style` for the place's overall feel).
fn topic_of(page: &str) -> &str {
    let name = page.rsplit('/').next().unwrap_or(page);
    name.strip_suffix(".md").or_else(|| name.strip_suffix(".MD")).unwrap_or(name)
}

/// **The place a place is inside**: the path with its last two segments (`levels/<map>`, `areas/<a>`, `regions/<r>`) taken off.
fn parent_of(place: &str) -> Option<&str> {
    let (rest, _) = place.rsplit_once('/')?;
    let (parent, _) = rest.rsplit_once('/')?;
    (parent.len() >= design::DESIGN_DIR.len()).then_some(parent)
}

// ── What is read ─────────────────────────────────────────────────────────────

/// One place of the tree, as the rows are drawn from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceRow {
    pub path: String,
    pub kind: PlaceKind,
    pub label: String,
    /// How many problems of the tree are about this place's own files or folder.
    pub problems: usize,
    /// Its topics other than `style`, as `(name, file)`.
    pub topics: Vec<(String, String)>,
    /// Whether the place has a `style.md`.
    pub has_style: bool,
}

/// What the middle of the workspace shows for the selected file.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Page {
    /// Nothing is selected, or no project is open.
    #[default]
    None,
    /// The file is not there: its place has no such page yet.
    Missing,
    /// The file is there and cannot be read whole: the reason.
    Unread(String),
    /// The file's text.
    Text(String),
    /// The place's whole chain as an agent reading `grimoire_style` for it is served it.
    Agent(String),
}

/// One picture of a board, as a card is drawn from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardRow {
    pub path: String,
    pub caption: String,
    pub licence: Option<String>,
    pub restricted: bool,
    pub missing: bool,
    pub uncaptioned: bool,
}

/// **Everything the three parts are drawn from**, read together so they cannot disagree with each other about the same instant.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Snapshot {
    pub root: PathBuf,
    pub places: Vec<PlaceRow>,
    pub page: Page,
    /// The problems about the selected place, as the one problem line says each.
    pub problems: Vec<String>,
    pub board: Vec<BoardRow>,
    /// Why the board could not be read, when it could not.
    pub board_error: Option<String>,
    /// The topics the selected place may start now (`design_templates::topics_offered`).
    pub offered: Vec<String>,
}

/// **The Design workspace's state**: what is selected, and what was last read. `Project`-owned: a switch to another workspace and back finds
/// the same page, and another project starts with none.
#[derive(Resource, Default)]
pub struct DesignView {
    selected: Option<String>,
    editing: bool,
    as_agent: bool,
    topic_menu: bool,
    /// The line a reveal asked for, counted from 0, until the page has been scrolled there.
    line: Option<usize>,
    /// **The line the last reveal asked for**, kept after the page has been scrolled there (SG-09, #408): where a followed line's link sent the person.
    last_line_asked: Option<usize>,
    /// The board picture whose card is lit.
    card: Option<String>,
    /// A picture of the page the person asked to be scrolled to (a card was chosen), by its index in the page.
    scroll_to_picture: Option<usize>,
    /// The web address the person was asked about.
    asked_address: Option<String>,
    snap: Snapshot,
    since: f32,
    /// The next frame reads at once: something the readers show has just changed.
    stale: bool,
}

impl DesignView {
    /// The page selected, project-relative.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// **The line a reveal asked for**, counted from 1, until the page has been scrolled there (SG-09, #408: a followed line's link asks for it).
    pub fn line_asked(&self) -> Option<usize> {
        self.line
    }

    /// **The line the last reveal asked for**, still set once the page has scrolled there.
    pub fn last_line_asked(&self) -> Option<usize> {
        self.last_line_asked
    }

    /// The page the file editor holds, while Edit is pressed.
    pub fn editing_file(&self) -> Option<&str> {
        self.selected.as_deref().filter(|_| self.editing)
    }

    /// Whether the page is the agent's reading of the chain.
    pub fn as_agent(&self) -> bool {
        self.as_agent
    }

    /// What was last read.
    pub fn snapshot(&self) -> &Snapshot {
        &self.snap
    }

    /// The place of the selected page.
    pub fn place(&self) -> Option<&str> {
        self.selected.as_deref().map(place_of)
    }

    /// **Select a page**, leaving Edit: the unsaved typing of the page left stays in its buffer.
    fn select(&mut self, page: &str, line: Option<usize>) {
        if self.selected.as_deref() != Some(page) {
            self.editing = false;
            self.topic_menu = false;
            self.card = None;
        }
        self.selected = Some(page.to_owned());
        self.line = line.filter(|line| *line > 0);
        if self.line.is_some() {
            self.last_line_asked = self.line;
        }
        self.stale = true;
    }

    /// The tree's problems that are about `place`.
    fn problems_about(tree: &design::Tree, place: &Place) -> Vec<String> {
        tree.problems
            .iter()
            .filter(|p| {
                p.where_.as_ref().and_then(|w| w.file.as_deref()).is_some_and(|file| file == place.path || place_of(file) == place.path)
            })
            .map(|p| p.line())
            .collect()
    }
}

// ── Pictures ─────────────────────────────────────────────────────────────────

/// One decoded picture, with what the file looked like when it was decoded.
struct Decoded {
    len: u64,
    modified: Option<SystemTime>,
    handle: Option<Handle<Image>>,
}

/// **The pictures of the page and the board**, loaded confined to the project and decoded once per change of the file.
#[derive(Resource, Default, Clone)]
pub struct DesignPictures(Arc<Mutex<HashMap<PathBuf, Decoded>>>);

impl DesignPictures {
    /// **The image at a project-relative path**, or `None` when it is not one the page may draw: not a PNG or JPEG, over
    /// [`PICTURE_MAX_BYTES`], outside the project once links are followed, or not decodable. A web address is never fetched.
    pub fn load(&self, server: &AssetServer, root: &Path, rel: &str) -> Option<Handle<Image>> {
        if rel.contains("://") || rel.starts_with('/') || rel.split(['/', '\\']).any(|part| part == "..") {
            return None;
        }
        let base = std::fs::canonicalize(root).ok()?;
        let full = std::fs::canonicalize(base.join(rel)).ok()?;
        // **A link that leaves the project is not a picture of it**: the page is the project's, and what it can make the editor read is not.
        if !full.starts_with(&base) {
            return None;
        }
        let meta = std::fs::metadata(&full).ok().filter(|m| m.is_file() && m.len() <= PICTURE_MAX_BYTES)?;
        let (len, modified) = (meta.len(), meta.modified().ok());
        let mut cache = self.0.lock().ok()?;
        if let Some(held) = cache.get(&full)
            && held.len == len
            && held.modified == modified
        {
            return held.handle.clone();
        }
        let ext = full.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
        let handle = match ext.as_str() {
            "png" | "jpg" | "jpeg" => std::fs::read(&full).ok().filter(|bytes| within_the_pixel_cap(rel, bytes)).and_then(|bytes| {
                Image::from_buffer(
                    &bytes,
                    ImageType::Extension(if ext == "png" { "png" } else { "jpg" }),
                    CompressedImageFormats::NONE,
                    true,
                    ImageSampler::Default,
                    RenderAssetUsages::default(),
                )
                .ok()
            }),
            _ => None,
        }
        .map(|image| server.add(image));
        cache.insert(full, Decoded { len, modified, handle: handle.clone() });
        handle
    }

    /// **The base a page's pictures are read against**: `dir` is the page's folder, and the resolver is [`Self::load`].
    pub fn base(&self, server: Option<&AssetServer>, root: &Path, dir: &str) -> MarkdownBase {
        let pictures = self.clone();
        let server = server.cloned();
        let root = root.to_path_buf();
        MarkdownBase::new(dir, move |path: &str| server.as_ref().and_then(|server| pictures.load(server, &root, path)))
    }
}

// ── Components ───────────────────────────────────────────────────────────────

/// The tree's panel, in the left dock.
#[derive(Component)]
pub struct DesignPanel;

/// The tree itself: a gooey `tree`, with what each row names.
#[derive(Component, Default)]
pub struct DesignTree {
    /// What each row's key names: the page it opens and the place it is on.
    names: HashMap<u64, String>,
    /// The rows the tree was last drawn from.
    drawn: Option<Vec<PlaceRow>>,
    /// The key the selection was last written for.
    selected: Option<u64>,
}

/// The middle column: the header above the page, and the page.
#[derive(Component)]
pub struct DesignHost;

/// The part above the page: the path, the buttons, the problems and the question about a changed file.
#[derive(Component)]
pub struct DesignTop;

/// **The page's own place**: the rendered page, the agent's reading of the chain, a sentence saying there is none, or the file editor.
#[derive(Component)]
pub struct DesignBody;

/// The rendered page (a gooey `markdown_view`).
#[derive(Component)]
pub struct DesignPage;

/// The board's panel, in the right dock.
#[derive(Component)]
pub struct DesignBoardPanel;

/// The column the board's cards are drawn into.
#[derive(Component)]
pub struct DesignBoardBody;

/// The board's card grid, with the picture each card is for, in order.
#[derive(Component)]
pub struct DesignCards(pub Vec<String>);

/// **A button of the header**, and what it does.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum DesignAction {
    /// Swap the page for the file editor.
    Edit,
    /// Save the file and render it again.
    Done,
    /// Show or hide the topics the place may start.
    NewTopicMenu,
    /// Start this topic here, from its template.
    NewTopic(String),
    /// Write the place's `style.md` from the template.
    CreateStyle,
    /// The file changed on disk under unsaved typing: take the disk's text.
    Reload,
    /// The file changed on disk under unsaved typing: keep mine.
    KeepMine,
}

/// The "As an agent reads it" toggle.
#[derive(Component)]
pub struct DesignAgentToggle;

// ── The plugin ───────────────────────────────────────────────────────────────

/// The Design workspace's systems.
pub struct DesignViewPlugin;

impl Plugin for DesignViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DesignView>()
            .init_resource::<DesignPictures>()
            .add_systems(OnEnter(Screen::Editor), spawn_the_design_workspace.after(FrameSystems).run_if(Workspace::design_is_open))
            .add_systems(
                Update,
                (
                    read_the_design,
                    keep_the_tree,
                    keep_the_top,
                    keep_the_body,
                    keep_the_board,
                    make_the_pictures_clickable,
                    scroll_the_page,
                    take_the_address_answer,
                    drop_to_the_selected_board,
                )
                    .chain()
                    .run_if(in_state(Screen::Editor))
                    .run_if(Workspace::design_is_open),
            )
            .add_observer(on_picture_click);
    }
}

/// **Spawn the tree, the page's column and the board** when the Design workspace is entered. Each part is filled by its own system.
fn spawn_the_design_workspace(mut commands: Commands, frame: Res<Frame>) {
    let panel = chrome::panel_root(&mut commands, &frame, Side::Left, TREE_PANEL_W, true, false, "DESIGN").insert(DesignPanel).id();
    commands.entity(panel).with_children(|p| {
        chrome::title(p, "DESIGN");
        p.spawn(bevy_gooey::fill_region()).with_children(|column| {
            column.spawn((tree(Vec::<TreeNode>::new()), DesignTree::default(), SelectMode::Single, observe(on_tree_pick)));
        });
    });
    let ink = chrome::ink();
    let host = commands
        .spawn((
            DesignHost,
            files::page_host(),
            BackgroundColor(ink.panel_bg),
            chrome::Ground(ink.panel_bg),
            chrome::PanelGround,
            BorderColor::all(ink.panel_edge),
            chrome::PanelEdge,
            bevy::picking::hover::Hovered::default(),
            AccessibleLabel::new("Design page"),
        ))
        .with_children(|host| {
            host.spawn((DesignTop, bevy_gooey::column_with(chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() })));
            host.spawn((
                DesignBody,
                bevy_gooey::fill_region(),
                bevy_gooey::Sizing { min_width: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
            ));
        })
        .id();
    commands.entity(frame.viewport).add_child(host);
    let board = chrome::panel_root(&mut commands, &frame, Side::Right, BOARD_PANEL_W, true, false, "BOARD").insert(DesignBoardPanel).id();
    commands.entity(board).with_children(|p| {
        chrome::title(p, "BOARD");
        // FOLLOW-OK: a column of cards, each a focusable button that gooey's own navigation scrolls into view; there is no selection of ours to follow.
        chrome::scroll_list(p, DesignBoardBody);
    });
}

// ── Reading ──────────────────────────────────────────────────────────────────

/// **The trust decision for a style file**, from the person's own store: the same question `grimoire_style` asks. A store that will not read
/// withholds the file, as it does there.
fn decider<'a>(
    store: &'a Result<bevy_grimoire_core::trust::TrustStore, String>,
    root: &'a Path,
) -> impl FnMut(&str, &str) -> Result<Decision, String> + 'a {
    move |path, text| match store {
        Ok(store) => store.text_decision(root, path, text),
        Err(e) => Err(e.clone()),
    }
}

/// **Read the tree, the selected page and its board** when [`REFRESH_S`] has passed or something changed, and write the snapshot only when it
/// differs: a settled workspace costs a comparison.
#[allow(clippy::too_many_arguments)]
fn read_the_design(
    time: Res<Time<Real>>,
    project: Option<Res<Project>>,
    state: Option<Res<EditorState>>,
    store_root: Option<Res<crate::trusted::StoreRoot>>,
    mut view: ResMut<DesignView>,
) {
    let Some(project) = project else {
        if view.snap != Snapshot::default() {
            view.snap = Snapshot::default();
        }
        return;
    };
    view.since += time.delta_secs();
    if view.since < REFRESH_S && !view.stale && view.snap.root == project.root {
        return;
    }
    view.since = 0.0;
    view.stale = false;
    let tree = design::Tree::read(&project.root);
    // **The workspace opens on the game's page**, which is where a style starts and where Create style writes.
    if view.selected.is_none() {
        view.selected = Some(GAME_PAGE.to_owned());
    }
    let Some(selected) = view.selected.clone() else { return };
    let place_path = place_of(&selected).to_owned();

    let places: Vec<PlaceRow> = tree
        .places
        .iter()
        .map(|place| PlaceRow {
            path: place.path.clone(),
            kind: place.kind,
            label: place_label(place),
            problems: DesignView::problems_about(&tree, place).len(),
            topics: place.topics.iter().filter(|t| t.name != design::STYLE_TOPIC).map(|t| (t.name.clone(), t.file.clone())).collect(),
            has_style: place.topic(design::STYLE_TOPIC).is_some(),
        })
        .collect();

    let page = if view.as_agent {
        let store = crate::trusted::load_at(store_root.as_deref(), state.as_deref().and_then(|s| s.trust.file.as_deref()));
        let topic = Some(topic_of(&selected)).filter(|t| *t != design::STYLE_TOPIC);
        match style::compose_place(&tree, &place_path, topic, &mut decider(&store, &project.root)) {
            Some(Ok(answer)) => Page::Agent(answer.text),
            Some(Err(why)) => Page::Unread(why),
            None => Page::Missing,
        }
    } else {
        match tree.text(&selected) {
            Some(text) => Page::Text(text.to_owned()),
            None => {
                let full = project.root.join(&selected);
                if !full.is_file() {
                    Page::Missing
                } else {
                    let why = tree
                        .problems
                        .iter()
                        .find(|p| p.where_.as_ref().and_then(|w| w.file.as_deref()) == Some(selected.as_str()))
                        .map(|p| p.line())
                        .unwrap_or_else(|| format!("`{selected}` could not be read whole"));
                    Page::Unread(why)
                }
            }
        }
    };

    let problems = tree.place(&place_path).map(|place| DesignView::problems_about(&tree, place)).unwrap_or_default();
    let (board, board_error) = match Board::of_place(&project.root, &place_path) {
        Ok(read) => {
            let rows = read
                .pictures
                .iter()
                .map(|e| BoardRow {
                    path: e.path.clone(),
                    caption: e.caption.clone(),
                    licence: e.licence.clone(),
                    restricted: e.restricted,
                    missing: !project.root.join(&e.path).is_file(),
                    uncaptioned: e.is_uncaptioned(),
                })
                .collect();
            (rows, None)
        }
        Err(why) => (Vec::new(), Some(why)),
    };
    let offered = if tree.place(&place_path).is_some() { crate::design_templates::topics_offered(&tree, &place_path) } else { Vec::new() };
    let snap = Snapshot { root: project.root.clone(), places, page, problems, board, board_error, offered };
    if view.snap != snap {
        view.snap = snap;
    }
}

// ── The tree ─────────────────────────────────────────────────────────────────

/// **The rows of the tree**: each place with its topics and then its places below it, every node with children open.
fn nodes_of(places: &[PlaceRow], names: &mut HashMap<u64, String>) -> Vec<TreeNode> {
    fn build(place: &PlaceRow, places: &[PlaceRow], names: &mut HashMap<u64, String>) -> TreeNode {
        let page = format!("{}/{}.{}", place.path, design::STYLE_TOPIC, design::MARKDOWN_EXT);
        let key = files::key_of(&place.path);
        names.insert(key, page);
        let label = match place.problems {
            0 => place.label.clone(),
            1 => format!("{}  (1 problem)", place.label),
            n => format!("{}  ({n} problems)", place.label),
        };
        let mut children: Vec<TreeNode> = place
            .topics
            .iter()
            .map(|(name, file)| {
                let key = files::key_of(file);
                names.insert(key, file.clone());
                TreeNode::new(key, name.clone())
            })
            .collect();
        children.extend(places.iter().filter(|p| parent_of(&p.path) == Some(place.path.as_str())).map(|child| build(child, places, names)));
        let node = TreeNode::new(key, label);
        // **Every node is expanded, a leaf included**: a leaf that gains its first child (the game after its first New topic) must show it, and `open_keys` only holds nodes that were open.
        node.expanded().children(children)
    }
    places.iter().filter(|p| p.kind == PlaceKind::Game).map(|game| build(game, places, names)).collect()
}

/// **Keep the tree's rows and its selection true** to the snapshot.
fn keep_the_tree(view: Res<DesignView>, mut trees: Query<(Entity, &mut Table, &mut DesignTree)>, mut commands: Commands) {
    let Ok((entity, mut table, mut tree)) = trees.single_mut() else { return };
    if tree.drawn.as_ref() != Some(&view.snap.places) && !view.snap.places.is_empty() {
        let mut names = HashMap::new();
        let nodes = nodes_of(&view.snap.places, &mut names);
        if let Some(rows) = table.source_mut::<bevy_gooey::Tree>() {
            let open = rows.open_keys();
            let known: Vec<u64> = tree.names.keys().copied().collect();
            rows.set_nodes(nodes);
            // A node the person had closed stays closed; one that is new is open.
            for key in known {
                if !open.contains(&key) {
                    rows.set_open(key, false);
                }
            }
        }
        tree.names = names;
        tree.drawn = Some(view.snap.places.clone());
        tree.selected = None;
    }
    let want = view.selected.as_deref().map(|page| {
        let place = place_of(page);
        if topic_of(page) == design::STYLE_TOPIC { files::key_of(place) } else { files::key_of(page) }
    });
    if want != tree.selected {
        tree.selected = want;
        match want {
            Some(key) if tree.names.contains_key(&key) => {
                commands.entity(entity).insert(TableSelection::of(key));
            }
            _ => {
                commands.entity(entity).insert(TableSelection::default());
            }
        }
    }
}

/// **A click in the tree selects the page of the row**: a place's `style.md`, or the topic.
fn on_tree_pick(picked: On<ValueChange<TableSelection>>, trees: Query<&DesignTree>, mut commands: Commands) {
    let Some(key) = picked.value.iter().next() else { return };
    let Ok(tree) = trees.single() else { return };
    let Some(page) = tree.names.get(&key).cloned() else { return };
    commands.queue(move |world: &mut World| show_file(world, &page, None));
}

// ── Selecting a page ─────────────────────────────────────────────────────────

/// **Show a page of the game's style**: the one way a page is selected (see the module note). Asks for the Design workspace when it is not the
/// one open, as `crosslinks::open_in_files` asks for the Files workspace.
pub fn show_file(world: &mut World, rel: &str, line: Option<usize>) {
    if let Some(mut view) = world.get_resource_mut::<DesignView>() {
        view.select(rel, line);
    }
    let here = world.get_resource::<Workspace>().copied();
    if here == Some(Workspace::Design) {
        return;
    }
    let Some(mut state) = world.get_resource_mut::<EditorState>() else { return };
    if state.door_request.is_none() && !state.leaving {
        state.door_request = Some(crate::editor::DoorRequest { workspace: Workspace::Design, tab: None });
    } else {
        state.status.problem(crate::editor::LEAVING_QUESTION_REFUSAL.to_owned());
    }
}

// ── The header ───────────────────────────────────────────────────────────────

/// What the header was last drawn from.
#[derive(Component, Clone, PartialEq, Eq, Default)]
struct TopDrawn(Option<TopKey>);

/// **Everything the header shows**, so it is rebuilt when one of them changes and not otherwise.
#[derive(Clone, PartialEq, Eq)]
struct TopKey {
    selected: Option<String>,
    page: PageKind,
    editing: bool,
    dirty: bool,
    conflict: bool,
    as_agent: bool,
    topic_menu: bool,
    offered: Vec<String>,
    problems: Vec<String>,
    place_label: String,
}

/// What kind of page the header sits over.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PageKind {
    None,
    Missing,
    Unread,
    Text,
    Agent,
}

fn kind_of(page: &Page) -> PageKind {
    match page {
        Page::None => PageKind::None,
        Page::Missing => PageKind::Missing,
        Page::Unread(_) => PageKind::Unread,
        Page::Text(_) => PageKind::Text,
        Page::Agent(_) => PageKind::Agent,
    }
}

/// **Draw the header**: the page's path (with a dot while it has unsaved typing), Edit or Done, the agent toggle, New topic and Create style, the
/// problems about the place, and the question a changed file asks.
fn keep_the_top(
    mut commands: Commands,
    view: Res<DesignView>,
    files: Res<Files>,
    tops: Query<(Entity, Option<&TopDrawn>), With<DesignTop>>,
    children: Query<&Children>,
) {
    let Ok((top, drawn)) = tops.single() else { return };
    let selected = view.selected.clone();
    let file = selected.as_deref().and_then(|rel| files.get(rel));
    let place_label = view
        .snap
        .places
        .iter()
        .find(|p| Some(p.path.as_str()) == view.place())
        .map(|p| p.label.clone())
        .unwrap_or_default();
    let key = TopKey {
        selected: selected.clone(),
        page: kind_of(&view.snap.page),
        editing: view.editing,
        dirty: file.is_some_and(files::OpenFile::dirty),
        conflict: file.is_some_and(|f| f.external.is_some()),
        as_agent: view.as_agent,
        topic_menu: view.topic_menu,
        offered: view.snap.offered.clone(),
        problems: view.snap.problems.clone(),
        place_label,
    };
    if drawn.and_then(|d| d.0.as_ref()) == Some(&key) {
        return;
    }
    if children.get(top).is_ok() {
        commands.entity(top).despawn_related::<Children>();
    }
    commands.entity(top).insert(TopDrawn(Some(key.clone()))).with_children(|top| {
        let path = key.selected.clone().unwrap_or_default();
        let dot = if key.dirty { "  \u{25CF}" } else { "" };
        top.spawn(bevy_gooey::label(format!("{path}{dot}")));
        top.spawn(bevy_gooey::row(1.0)).with_children(|buttons| {
            let exists = matches!(key.page, PageKind::Text);
            if key.editing {
                buttons.spawn((bevy_gooey::button("Done"), DesignAction::Done, observe(on_action)));
            } else if exists && !key.as_agent {
                buttons.spawn((bevy_gooey::button("Edit page"), DesignAction::Edit, observe(on_action)));
            }
            if matches!(key.page, PageKind::Missing) && key.selected.as_deref().is_some_and(|page| topic_of(page) == design::STYLE_TOPIC) {
                buttons.spawn((bevy_gooey::button("Create style"), DesignAction::CreateStyle, observe(on_action)));
            }
            if !key.offered.is_empty() {
                buttons.spawn((bevy_gooey::button("New topic"), DesignAction::NewTopicMenu, observe(on_action)));
            }
        });
        // **The toggle is a row of its own**: beside the buttons it took the room and squeezed New topic to 24 px, which a click could not hit.
        top.spawn((bevy_gooey::toggle("As an agent reads it", key.as_agent), DesignAgentToggle, observe(on_agent_toggle)));
        if key.topic_menu {
            // **A wrapping row**: five topic buttons do not fit the middle column of the smallest window (1024 x 640), and a plain row squeezed each to about 23 px.
            crate::chrome::wrapping_row(top, 1.0, crate::chrome::NO_MARGIN, AlignItems::Center).with_children(|menu| {
                for topic in &key.offered {
                    // Padded with two spaces a side, so the shortest name (`ui`) is still a target at least 40 px wide; the label is compared trimmed.
                    menu.spawn((bevy_gooey::button(format!("  {topic}  ")), DesignAction::NewTopic(topic.clone()), observe(on_action)));
                }
            });
        }
        // **A problem is shown above the page it belongs to, in the one problem shape** (UI-05): what is wrong, because, and the fix.
        for line in &key.problems {
            top.spawn(bevy_gooey::label(line.clone()));
        }
        if key.conflict
            && let Some(rel) = key.selected.as_deref()
        {
            top.spawn(bevy_gooey::row(1.0)).with_children(|bar| {
                bar.spawn(bevy_gooey::label(files::conflict_sentence(rel)));
                bar.spawn((bevy_gooey::button("Reload"), DesignAction::Reload, observe(on_action)));
                bar.spawn((bevy_gooey::button("Keep mine"), DesignAction::KeepMine, observe(on_action)));
            });
        }
    });
}

/// **The toggle shows the page as an agent reads it, or the file.**
fn on_agent_toggle(change: On<ValueChange<bool>>, toggles: Query<(), With<DesignAgentToggle>>, mut view: ResMut<DesignView>) {
    if toggles.get(change.source).is_err() || view.as_agent == change.value {
        return;
    }
    view.as_agent = change.value;
    // The chain is composed for the place; the file editor holds one file, so Edit is left.
    if change.value {
        view.editing = false;
    }
    view.stale = true;
}

/// A press on one of the header's buttons.
fn on_action(activate: On<Activate>, actions: Query<&DesignAction>, mut commands: Commands) {
    let Ok(action) = actions.get(activate.entity).cloned() else { return };
    commands.queue(move |world: &mut World| run_action(world, action));
}

/// **Carry out a header button.** A refusal is the sentence on the status line, never a silent no-op.
fn run_action(world: &mut World, action: DesignAction) {
    let Some(project) = world.get_resource::<Project>().cloned() else { return };
    let Some(selected) = world.get_resource::<DesignView>().and_then(|v| v.selected.clone()) else { return };
    let said = match action {
        DesignAction::Edit => match files::hold_for_editing(world, &selected) {
            Ok(()) => {
                if let Some(mut view) = world.get_resource_mut::<DesignView>() {
                    view.editing = true;
                    view.stale = true;
                }
                None
            }
            Err(why) => Some(Err(format!("NOT OPENED: {why}"))),
        },
        DesignAction::Done => {
            let saved = world.resource_scope(|_, mut files: Mut<Files>| files::save_file(&mut files, &project.root, &selected));
            match saved {
                Ok(()) => {
                    if let Some(mut view) = world.get_resource_mut::<DesignView>() {
                        view.editing = false;
                        view.stale = true;
                    }
                    Some(Ok(format!("saved {selected}")))
                }
                Err(why) => Some(Err(format!("NOT SAVED: {why}"))),
            }
        }
        DesignAction::Reload | DesignAction::KeepMine => {
            if let Some(mut files) = world.get_resource_mut::<Files>()
                && let Some(file) = files.get_mut(&selected)
                && let Some(theirs) = file.external.take()
            {
                if action == DesignAction::Reload {
                    file.text = theirs.clone();
                    file.disk = theirs;
                    file.pushed += 1;
                } else {
                    file.disk = theirs;
                }
            }
            None
        }
        DesignAction::NewTopicMenu => {
            if let Some(mut view) = world.get_resource_mut::<DesignView>() {
                view.topic_menu = !view.topic_menu;
            }
            None
        }
        DesignAction::CreateStyle => {
            let place = place_of(&selected).to_owned();
            let name = if place == design::DESIGN_DIR {
                project.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "The game".to_owned())
            } else {
                place.rsplit('/').next().unwrap_or(&place).to_owned()
            };
            Some(write_and_show(world, &project, crate::design_templates::write_style(&project.root, &place, &name)))
        }
        DesignAction::NewTopic(topic) => {
            let place = place_of(&selected).to_owned();
            let name = if place == design::DESIGN_DIR { "The game".to_owned() } else { place.rsplit('/').next().unwrap_or(&place).to_owned() };
            let written = crate::design_templates::create_topic(&project.root, &place, &topic, &name);
            if let Some(mut view) = world.get_resource_mut::<DesignView>() {
                view.topic_menu = false;
            }
            Some(write_and_show(world, &project, written))
        }
    };
    if let (Some(said), Some(mut state)) = (said, world.get_resource_mut::<EditorState>()) {
        match said {
            Ok(note) => state.status.note(note),
            Err(why) => state.status.problem(why),
        }
    }
}

/// **A page the editor wrote from a template is the person's own act**, so it is allowed (the text is the editor's own, as the wizard's is) and
/// shown. `Err` is the refusal's sentence.
fn write_and_show(world: &mut World, project: &Project, written: Result<PathBuf, String>) -> Result<String, String> {
    let file = written?;
    let rel = file.strip_prefix(&project.root).map(|p| p.to_string_lossy().replace('\\', "/")).map_err(|_| format!("{} is not inside the project", file.display()))?;
    let store_root = world.get_resource::<crate::trusted::StoreRoot>().cloned();
    let trust_file = world.get_resource::<EditorState>().and_then(|s| s.trust.file.clone());
    // **A grant that could not be written is said, on screen**: the page is written and shown, and grimoire_style will ask before it serves it.
    let granted = crate::design_templates::grant_file(&project.root, &rel, store_root.as_ref(), trust_file.as_deref());
    if let Some(mut view) = world.get_resource_mut::<DesignView>() {
        view.select(&rel, None);
    }
    if let Err(why) = granted {
        return Err(format!("wrote {rel}, but it could not be allowed ({why}), so grimoire_style will ask before it serves it"));
    }
    Ok(format!("wrote {rel}"))
}

// ── The page ─────────────────────────────────────────────────────────────────

/// What the page was last drawn from.
#[derive(Component, Clone, PartialEq, Eq, Default)]
struct BodyDrawn(Option<(Entity, Page, Option<String>, bool, PathBuf)>);

/// **Draw the page**: the rendered Markdown, the agent's reading of the chain, or a sentence saying why there is none. While Edit holds the
/// file the page is taken away and the file editor takes its place (`files::keep_the_design_editor`).
fn keep_the_body(
    mut commands: Commands,
    view: Res<DesignView>,
    pictures: Res<DesignPictures>,
    server: Option<Res<AssetServer>>,
    bodies: Query<(Entity, Option<&BodyDrawn>), With<DesignBody>>,
) {
    let Ok((body, drawn)) = bodies.single() else { return };
    let key = (
        body,
        if view.editing { Page::None } else { view.snap.page.clone() },
        view.selected.clone(),
        view.editing,
        view.snap.root.clone(),
    );
    if drawn.and_then(|d| d.0.as_ref()) == Some(&key) {
        return;
    }
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).insert(BodyDrawn(Some(key.clone())));
    if view.editing {
        return;
    }
    let dir = view.selected.as_deref().map(place_of).unwrap_or(design::DESIGN_DIR).to_owned();
    let say = |commands: &mut Commands, words: String| {
        commands.entity(body).with_children(|b| {
            b.spawn(bevy_gooey::label(words));
        });
    };
    match &view.snap.page {
        Page::None => say(&mut commands, "Pick a place in the tree".to_owned()),
        Page::Missing => {
            let place = view.place().and_then(|p| view.snap.places.iter().find(|row| row.path == p));
            let name = place.map_or_else(|| "this place".to_owned(), |p| p.label.clone());
            say(&mut commands, format!("{name} has no style yet. Create style writes a page to start from."));
        }
        Page::Unread(why) => say(&mut commands, why.clone()),
        Page::Text(text) => {
            let base = pictures.base(server.as_deref(), &view.snap.root, &dir);
            commands.entity(body).with_children(|b| {
                b.spawn((markdown_view(text, base), DesignPage, observe(on_link)));
            });
        }
        Page::Agent(text) => {
            // **Rendered against the project's root, and only the chain's words**: its files sit in different folders and each cites its pictures relative to its own, so a picture drawn from here is the missing box; the pictures a served file cites are listed under it as text.
            let base = pictures.base(server.as_deref(), &view.snap.root, "");
            commands.entity(body).with_children(|b| {
                b.spawn((markdown_view(text, base), DesignPage, observe(on_link)));
            });
        }
    }
}

/// **A link in the page**: another design file opens here, any other project file goes through the cross-links, a web link asks first.
fn on_link(link: On<MarkdownLink>, view: Res<DesignView>, mut commands: Commands) {
    use bevy_gooey::markdown::LinkKind;
    match link.kind {
        // gooey scrolls the page to the heading itself.
        LinkKind::Anchor => {}
        LinkKind::Web => {
            let address = link.dest.clone();
            commands.queue(move |world: &mut World| ask_about_an_address(world, address));
        }
        LinkKind::Relative => {
            let dir = view.selected.as_deref().map(place_of).unwrap_or(design::DESIGN_DIR).to_owned();
            let target = link.dest.split(['#', '?']).next().unwrap_or("").to_owned();
            commands.queue(move |world: &mut World| {
                // Joined onto the page's folder and folded, so a path that climbs out of the project is refused rather than followed.
                match bevy_gooey::markdown::resolve("", &format!("{dir}/{target}")) {
                    Ok(rel) if is_page(&rel) => show_file(world, &rel, None),
                    Ok(rel) => crate::crosslinks::open_in_files(world, &rel, None),
                    // **A fixed sentence**: the destination is the page's text, and the status line reaches an agent through grimoire_state.
                    Err(_) => {
                        if let Some(mut state) = world.get_resource_mut::<EditorState>() {
                            state.status.problem("NOT OPENED: a link in the page leaves the project");
                        }
                    }
                }
            });
        }
    }
}

/// **Ask the person before the system browser starts**: a page is the project's text, and a cloned project's link is not the person's choice.
fn ask_about_an_address(world: &mut World, address: String) {
    // **Only http and https are asked about**: gooey calls any `scheme:` a web link, and the opener would hand `file:`, `smb:`, `mailto:` or a
    // custom handler to the system. A fixed sentence says so; the address is the page's text and goes nowhere an agent reads.
    if !is_web_address(&address) {
        if let Some(mut state) = world.get_resource_mut::<EditorState>() {
            state.status.problem("NOT OPENED: a link in the page is not an http or https address, so nothing was opened");
        }
        return;
    }
    let asked = {
        let Some(mut confirm) = world.get_resource_mut::<crate::confirm::Confirm>() else { return };
        confirm.ask(crate::confirm::Asked::OpenAddress, OPEN_ADDRESS_TITLE, address.clone(), "Open", "Not now");
        confirm.asking(crate::confirm::Asked::OpenAddress)
    };
    if asked && let Some(mut view) = world.get_resource_mut::<DesignView>() {
        view.asked_address = Some(address);
    }
}

/// **Whether an address is one the editor asks to open**: `http://` or `https://`, in either case, and nothing else.
pub fn is_web_address(address: &str) -> bool {
    let lower = address.trim_start().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// **Whether a picture's own header says it is small enough to decode** ([`PICTURE_MAX_PIXELS`]). A header that does not read is refused too.
fn within_the_pixel_cap(name: &str, bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(bevy_grimoire_core::image_header::HEAD)];
    bevy_grimoire_core::image_header::read(name, head).is_ok_and(|h| u64::from(h.width) * u64::from(h.height) <= PICTURE_MAX_PIXELS)
}

/// **The answer to the address question**: Open starts the browser (or records the address when this run opens none), and anything else drops it.
fn take_the_address_answer(mut commands: Commands, view: Res<DesignView>, confirm: Option<ResMut<crate::confirm::Confirm>>) {
    let (Some(mut confirm), true) = (confirm, view.asked_address.is_some()) else { return };
    let Some(yes) = confirm.answer(crate::confirm::Asked::OpenAddress) else { return };
    commands.queue(move |world: &mut World| {
        let address = world.get_resource_mut::<DesignView>().and_then(|mut v| v.asked_address.take());
        if let (true, Some(address)) = (yes, address) {
            crate::app_verbs::open_link(world, &address);
        }
    });
}

// ── The board ────────────────────────────────────────────────────────────────

/// What the board was last drawn from: the project, the place, the rows, why they would not read, and the lit card.
type BoardKey = (PathBuf, Option<String>, Vec<BoardRow>, Option<String>, Option<String>);

/// What the board was last drawn from.
#[derive(Component, Clone, PartialEq, Eq, Default)]
struct BoardDrawn(Option<BoardKey>);

/// **Draw the board of the selected place** as a grid of cards: the caption, the licence line, *Restricted* and *Missing on this machine*.
fn keep_the_board(
    mut commands: Commands,
    view: Res<DesignView>,
    pictures: Res<DesignPictures>,
    server: Option<Res<AssetServer>>,
    bodies: Query<(Entity, Option<&BoardDrawn>), With<DesignBoardBody>>,
) {
    let Ok((body, drawn)) = bodies.single() else { return };
    let key = (view.snap.root.clone(), view.place().map(str::to_owned), view.snap.board.clone(), view.snap.board_error.clone(), view.card.clone());
    if drawn.and_then(|d| d.0.as_ref()) == Some(&key) {
        return;
    }
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).insert(BoardDrawn(Some(key)));
    if let Some(why) = &view.snap.board_error {
        let why = why.clone();
        commands.entity(body).with_children(|b| {
            b.spawn(bevy_gooey::label(why));
        });
        return;
    }
    if view.snap.board.is_empty() {
        commands.entity(body).with_children(|b| {
            b.spawn(bevy_gooey::label("No pictures on this board. Drop a picture here to add one."));
        });
        return;
    }
    let cards: Vec<Card> = view
        .snap
        .board
        .iter()
        .map(|row| {
            let mut card = Card::new(row.caption.clone());
            if !row.missing
                && let Some(server) = server.as_deref()
                && let Some(image) = pictures.load(server, &view.snap.root, &row.path)
            {
                card = card.image(image);
            }
            card = card.line(row.licence.clone().unwrap_or_else(|| "No licence given".to_owned()));
            if row.restricted {
                card = card.line("Restricted: not shipped with the game");
            }
            if row.missing {
                card = card.line("Missing on this machine");
            }
            card
        })
        .collect();
    let paths: Vec<String> = view.snap.board.iter().map(|r| r.path.clone()).collect();
    let lit = view.card.as_ref().and_then(|c| paths.iter().position(|p| p == c));
    commands.entity(body).with_children(|b| {
        b.spawn((bevy_gooey::card_grid(cards), DesignCards(paths), CardSelection(lit), observe(on_card_chosen)));
    });
}

/// **Choosing a card scrolls the page to its picture's first citation.**
fn on_card_chosen(change: On<ValueChange<usize>>, grids: Query<&DesignCards>, mut view: ResMut<DesignView>) {
    if !change.is_final {
        return;
    }
    let Ok(cards) = grids.get(change.source) else { return };
    let Some(path) = cards.0.get(change.value).cloned() else { return };
    let Some(selected) = view.selected.clone() else { return };
    let text = match &view.snap.page {
        Page::Text(text) => text.clone(),
        _ => return,
    };
    view.card = Some(path.clone());
    view.scroll_to_picture = design::pictures(&text, &selected).iter().position(|p| p.file.as_deref() == Some(path.as_str()));
}

/// **A picture of the page takes a click**, which gooey's `image_box` does not (it sets `Pickable::IGNORE`, as every picture in that crate does).
fn make_the_pictures_clickable(added: Query<Entity, Added<MarkdownPicture>>, mut commands: Commands) {
    for picture in &added {
        commands.entity(picture).insert(Pickable::default());
    }
}

/// **A click on a picture of the page lights its card.**
fn on_picture_click(click: On<PointerClick>, pictures: Query<&MarkdownPicture>, mut commands: Commands) {
    let Ok(picture) = pictures.get(click.entity) else { return };
    let index = picture.index();
    commands.queue(move |world: &mut World| pick_picture(world, index));
}

/// **Light the card of the page's `index`th picture**, and leave the page where it is. A picture that is not on the board lights none.
pub fn pick_picture(world: &mut World, index: usize) {
    let Some(mut view) = world.get_resource_mut::<DesignView>() else { return };
    let (Some(selected), Page::Text(text)) = (view.selected.clone(), view.snap.page.clone()) else { return };
    let cited = design::pictures(&text, &selected).get(index).and_then(|p| p.file.clone());
    view.card = cited.filter(|path| view.snap.board.iter().any(|row| &row.path == path));
}

/// **Scroll the page**: to the picture a card was chosen for, and to the heading a reveal's line is under. Waits for the page to have been built
/// and laid out, then asks once.
#[allow(clippy::too_many_arguments)]
fn scroll_the_page(
    mut commands: Commands,
    mut view: ResMut<DesignView>,
    pages: Query<(Entity, &MarkdownView, &MarkdownOutline)>,
    viewports: Query<(&ChildOf, &ComputedNode, &UiGlobalTransform, &mut ScrollPosition), With<MarkdownViewport>>,
    pictures: Query<(&MarkdownPicture, &ComputedNode, &UiGlobalTransform)>,
) {
    let Ok((page, source, outline)) = pages.single() else { return };
    if let Some(line) = view.line
        && !outline.0.is_empty()
    {
        view.line = None;
        // The heading the line is under: the last whose line is not past it, by the position the page's outline lists it at.
        let headings = design::headings(source.source());
        let at = headings.iter().rposition(|h| h.line <= line);
        if let Some(entry) = at.and_then(|at| outline.0.get(at)) {
            commands.trigger(scroll_to_heading(page, entry.id.clone()));
        }
    }
    let Some(index) = view.scroll_to_picture else { return };
    let Some((_, picture_node, picture_at)) = pictures.iter().find(|(p, ..)| p.index() == index) else { return };
    let mut viewports = viewports;
    let Some((_, viewport, viewport_at, mut scroll)) = viewports.iter_mut().find(|(parent, ..)| parent.parent() == page) else { return };
    if picture_node.size().y <= 0.0 || viewport.size().y <= 0.0 {
        return;
    }
    view.scroll_to_picture = None;
    let (scroll_y, max) = chrome::scroll_bounds(viewport);
    let inv = viewport.inverse_scale_factor();
    let picture_top = picture_at.translation.y - picture_node.size().y / 2.0;
    let viewport_top = viewport_at.translation.y - viewport.size().y / 2.0;
    scroll.0.y = (scroll_y + (picture_top - viewport_top) * inv).clamp(0.0, max);
}

// ── A file dropped on the workspace ──────────────────────────────────────────

/// **A picture dropped anywhere in the workspace goes on the selected place's board**, through `board_set`: one log line and one undo step, as
/// the editor's own drop (`editor::drop_to_board`) and an agent's `apply`. A drop has no caption, so it goes on needing one.
fn drop_to_the_selected_board(
    mut drops: MessageReader<bevy::window::FileDragAndDrop>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    view: Res<DesignView>,
    mut state: Option<ResMut<EditorState>>,
) {
    for drop in drops.read() {
        let bevy::window::FileDragAndDrop::DroppedFile { path_buf, .. } = drop else { continue };
        if crate::add_meshes::is_mesh_file(path_buf) {
            continue;
        }
        let (Some(project), Some(state), Some(place)) = (project.as_deref(), state.as_deref_mut(), view.place()) else { continue };
        let Some(path) = path_buf.to_str() else {
            state.status.problem(format!("{} is not a path the board can name, so it is not on a board", path_buf.display()));
            continue;
        };
        let params = serde_json::json!({"action": "add", "place": place, "path": path, "caption": board::PENDING_CAPTION});
        let mut ctx = crate::ops::Ctx {
            project: Some(project.clone()),
            open: open.as_deref().cloned(),
            source: crate::ops::Source::Human,
            session: crate::ops::session_id(),
            ..crate::ops::Ctx::default()
        };
        match crate::ops::dispatch(&mut ctx, "board_set", &params) {
            Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => {
                state.status.note(format!("{}: say what to take from it with `ref <path> <caption>` on the Map's terrain line", outcome.summary));
                state.record_logged(operation_id, outcome.summary, false);
            }
            Ok(crate::ops::Applied::Refused(e)) | Err(e) => state.status.problem(e.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_a_markdown_file_under_design() {
        assert!(is_page("design/style.md"));
        assert!(is_page("design/levels/m/dialogue.md"));
        assert!(!is_page("design/levels/m/board/fog.png"));
        assert!(!is_page("assets/notes.md"));
        assert!(!is_page("design/../secret.md"));
        assert!(!is_page("design//x.md"));
    }

    #[test]
    fn a_place_is_inside_the_place_the_path_names() {
        assert_eq!(parent_of("design/levels/m"), Some("design"));
        assert_eq!(parent_of("design/regions/r/levels/m"), Some("design/regions/r"));
        assert_eq!(parent_of("design/levels/m/areas/a"), Some("design/levels/m"));
        assert_eq!(parent_of("design"), None);
        assert_eq!(place_of("design/levels/m/style.md"), "design/levels/m");
        assert_eq!(topic_of("design/levels/m/dialogue.md"), "dialogue");
        assert_eq!(topic_of("design/style.md"), "style");
    }
}
