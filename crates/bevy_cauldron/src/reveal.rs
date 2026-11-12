//! **The editor opens the page a change is on** (AF-03, #359; series agent follow, #356).
//!
//! AF-02 (#358) followed a change **on the map**: [`crate::follow`] eases the camera to the change's
//! own box. Most of what an agent changes is not on the map at all — a library piece's description,
//! a piece's lattice, a composition, a rig's measured values, a tuning file, a pack's exclusion, a
//! note — and each of those has a page the editor already draws. So `Shown`, the one answer per
//! operation to *where its change is seen* (AF-01, #357), is now also the one answer to *where the
//! editor goes*.
//!
//! # One way to show a thing
//!
//! [`Revealing::request`] is the only entry point and [`page_of`] is the only thing that decides a
//! page. A second path — a palette row that crossed a door itself, a cross-link that set both a tab and
//! a selection — would be a second answer to "which page is this thing on", and the two would disagree
//! the first time a panel moved. Navigation calls this same function when it lands (Go to anything;
//! Back and Forward, NV-06, whose history push belongs in [`take_the_reveal`] for the reason that is
//! the one place that knows a view is about to move).
//!
//! # Selecting to show is not an edit
//!
//! A reveal writes a selection, a tab and a door — never a document. `ImportState::focus_on`,
//! `ComposeState::selected`, `BenchState::selected`, [`crate::editor::SelectedNote`] and the DATA
//! table's cursor are all editor state, so following an agent records no operation, writes no file and
//! takes no undo step, exactly as framing the camera does not.
//!
//! # A `Shown` with no page is a bug, and says so
//!
//! `Shown::File { path: "assets/grimoire/vocab.ron" }` is a real change with no page: the Files
//! workspace is GD-11 (#373) and does not exist. Doing nothing would leave the person reading
//! `the Familiar added a token` with nowhere to look, so a `Shown` the editor cannot show becomes a
//! **problem naming the operation and its shown**, which is the work item the missing view is built
//! from.
//!
//! # Two frames, because a switching rebuilds the panels
//!
//! A door switching is a state transition: the old panels are torn down and the new ones spawned in one
//! frame (`screen::cross`), and the teardown resets every `Door` resource (`screen::OWNERSHIP`). A
//! selection written on the frame the switching was *asked for* lands in the state that teardown then
//! resets, so the reveal asks, waits for the door **and the panel** to be the ones asked for, and
//! writes the selection on the frame after. Measured on the first version: a piece's row was selected,
//! the switching reset `ImportState`, and the shelf arrived with the fixture's first library row armed.

use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, ScrollPosition, UiGlobalTransform};

use crate::editor::{DoorRequest, EditorState};
use crate::ops::Shown;
use crate::project::Project;
use crate::tiles::Mode;
use crate::workspace::Workspace;

/// **A heading in the Map door's INSPECTOR panel**, named so a reveal can bring it into view.
///
/// The inspector is one long scroll — MAP SIZE, FAMILY SETTINGS, PLAY, ENVIRONMENT, … DATA, NOTES,
/// RELOAD, PLAN, PROBLEMS, COMPOSITIONS. A change shown by a section of it is shown by scrolling that
/// heading on screen, which is what a person does by hand.
///
/// **Only the three whose rows a reveal can select.** The Meshes panel's `MESH` and `SUBGRID` blocks
/// are the `DetailPane`'s own scroll on the Kit door, and the item a reveal selects there is the
/// shelf row — which `tiles::keep_selection_on_screen` already keeps on screen. A second scroller for
/// the same pane would be a second answer to where the pane is looking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectorSection {
    /// The registered tuning files' leaves.
    Data,
    /// The open map's notes.
    Notes,
    /// A family crate's settings, the `FAMILY SETTINGS` line.
    Family,
    /// **The open map's problems** (NV-11, #397): the `PROBLEMS` list, which was six wheel notches down the inspector.
    Problems,
}

/// **A heading's own name**, carried by the section it titles.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section(pub InspectorSection);

/// **Where a page is.**
#[derive(Clone, PartialEq, Eq)]
pub enum Page {
    /// A door and one of its panels, with a selection made there.
    Workspace {
        workspace: Workspace,
        tab: Mode,
        select: Select,
    },
    /// **The settings screen**, on one of its category tabs (`0` is *All*).
    Settings { tab: usize },
    /// **The project configuration screen**, whose lists are the kit's family crates and its packs.
    ProjectConfig,
}

/// **What the page selects**, so the row the change is on is the row the person's eye lands on.
#[derive(Clone, PartialEq, Eq)]
pub enum Select {
    /// A library piece on the shelf, which `ImportState::focus_on` arms.
    Piece(String),
    /// A group on the Compose panel.
    Group(String),
    /// A rig on the bench.
    Rig(String),
    /// **A rig's clip** (NV-11, #391): the rig selected on the bench with its slot `slot` soloed on the stage.
    Clip { rig: String, slot: usize },
    /// A registered data file: its first leaf's row in the DATA table.
    DataFile(String),
    /// A note on the Notes list.
    Note(String),
    /// **A sound cue** on the Sound workspace's list (NV-07, #384): the cue the list marks as selected.
    Cue(String),
    /// An inspector section alone.
    Section(InspectorSection),
    /// Nothing to select: the page is enough.
    Nothing,
}

/// **The page a `Shown` is on, or the sentence saying the editor has none.**
///
/// **The one decision**, so the receipt, the reveal and any later navigation cannot disagree about
/// where a thing lives. `project` is read only for `Shown::File`, whose page depends on whether the
/// project's `data.ron` registers the file — the DATA table is the only file page the editor has until
/// GD-11 (#373) lands, and a file the registry does not list has none.
pub fn page_of(shown: &Shown, project: Option<&Project>) -> Result<Page, String> {
    let page = match shown {
        // **The map is the camera's** (AF-02), not a page: `crate::follow` frames it, and framing is
        // not a selection. A page here would cross doors for a change already on screen.
        Shown::Map { .. } | Shown::MapWhole => {
            return Err("this change is on the map, which the camera frames and no page opens".to_owned());
        }
        // **The operation said it has nothing to look at** (AF-01's pinning test lists those). The
        // reveal is not called for one; reaching here is a caller's mistake, not a missing view.
        Shown::Nowhere => return Err("this change says it has nothing to look at".to_owned()),
        // **A job is work a person watches in the band**, which is on every screen: there is no page
        // to open for it, and the band is already showing it (GD-09, #371).
        Shown::Job { key } => {
            return Err(format!("this change is the `{key}` job, which the status band is already showing"));
        }
        // **A library piece is described on the Meshes shelf**, whose left pane also draws the
        // piece's size, labels and lattice.
        //
        // **A piece's lattice is on MESHES, not on Tiles.** `Shown::Tile`'s own doc says "the Tiles
        // tab", and the panel that draws it says otherwise: `tiles::rebuild_detail` returns into
        // `build_detail` for `Mode::Tiles` — the open *composition* — and the `SUBGRID` block,
        // `draw_subgrid`, `pick_lattice` and `click_lattice` all run on the Meshes panel. Going where
        // the change is drawn is the only reading that puts the person in front of it.
        Shown::Piece { id } | Shown::Tile { id } => Page::Workspace {
            workspace: Workspace::Kit,
            tab: Mode::Meshes,
            select: Select::Piece(id.clone()),
        },
        Shown::Composition { id } => Page::Workspace {
            workspace: Workspace::Kit,
            tab: Mode::Compose,
            select: Select::Group(id.clone()),
        },
        Shown::Rig { id } => Page::Workspace {
            workspace: Workspace::Animation,
            tab: Mode::Anim,
            select: Select::Rig(id.clone()),
        },
        Shown::Clip { rig, slot } => Page::Workspace {
            workspace: Workspace::Animation,
            tab: Mode::Anim,
            select: Select::Clip { rig: rig.clone(), slot: *slot },
        },
        Shown::Note { id } => Page::Workspace {
            workspace: Workspace::Map,
            tab: Mode::Map,
            select: Select::Note(id.clone()),
        },
        Shown::Cue { name } => Page::Workspace {
            workspace: Workspace::Sound,
            tab: Mode::Sound,
            select: Select::Cue(name.clone()),
        },
        // **A registered data file's page is the DATA table**; anything else has none until the Files
        // workspace lands (GD-11, #373). `lines` is where that page would scroll to, so it is named in
        // the refusal: the day the page exists, this arm becomes it.
        Shown::File { path, lines } => match project.and_then(|p| registered_data_file(p, path)) {
            Some(file) => Page::Workspace {
                workspace: Workspace::Map,
                tab: Mode::Map,
                select: Select::DataFile(file),
            },
            None => {
                return Err(format!(
                    "`{path}` is a project file, and the editor has no page that shows one yet (the Files \
                     workspace, #373), so nothing was selected{}",
                    match lines {
                        Some([first, last]) => format!("; the change is at lines {first}..{last}"),
                        _ => String::new(),
                    }
                ));
            }
        },
        Shown::Settings { section } => settings_page(section)?,
    };
    Ok(page)
}

/// **The settings screen's tabs are one-based on its categories**: tab `0` is *All*, and the category
/// at `CATEGORIES[i]` is tab `i + CATEGORY_TAB_BASE`. Named rather than a bare `+ 1`, because a reader
/// meeting the expression has no way to tell a one-based tab from an off-by-one.
pub const CATEGORY_TAB_BASE: usize = 1;

/// **Where a section of the editor's settings is shown.**
///
/// The sections an operation names are of three kinds, and each has its own page:
///
/// - a **pack** or the **kits** list, which the project configuration screen draws;
/// - a **category of the settings screen** (`Appearance`, `Map`, …), which is that screen's tab;
/// - a **heading of the Map door's inspector** — `notes`, `data`, a family crate's settings — which is
///   scrolled into view, because that is where the editor draws it.
///
/// A name in none of the three is a refusal rather than a default: a reveal that quietly opened the
/// settings screen for a section nobody draws would tell the person they were looking at the change.
fn settings_page(section: &str) -> Result<Page, String> {
    let lower = section.to_lowercase();
    if matches!(lower.as_str(), "packs" | "kits") {
        return Ok(Page::ProjectConfig);
    }
    if let Some(at) = crate::setting_catalogue::CATEGORIES
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&lower))
    {
        return Ok(Page::Settings { tab: at + CATEGORY_TAB_BASE });
    }
    let heading = match lower.as_str() {
        "notes" => Some(InspectorSection::Notes),
        "problems" => Some(InspectorSection::Problems),
        "data" => Some(InspectorSection::Data),
        // A family crate's settings are one line of the inspector's `FAMILY SETTINGS` block.
        other if crate::family_host::KNOWN.contains(&other) => Some(InspectorSection::Family),
        _ => None,
    };
    match heading {
        Some(section) => Ok(Page::Workspace {
            workspace: Workspace::Map,
            tab: Mode::Map,
            select: Select::Section(section),
        }),
        None => Err(format!(
            "`{section}` is not a section the editor draws: the settings screen has {}, the project \
             configuration draws the kit's packs and its family crates, and the inspector has notes, \
             problems, data and every family crate",
            crate::setting_catalogue::CATEGORIES.join(", ")
        )),
    }
}

/// **The registry id of the data file a project-relative path names**, or `None` when `data.ron` does
/// not register it.
///
/// The path a `Shown` carries is relative to the project root; the registry's own `path` is relative to
/// the grimoire directory, and `DataFiles::resolve` is the one function that turns one into the other —
/// so this compares resolved paths rather than joining twice and hoping the two agree.
pub fn registered_data_file(project: &Project, relative: &str) -> Option<String> {
    let wanted = project.root.join(relative);
    let reg = bevy_grimoire_core::data_files::DataFiles::load(&project.grimoire_dir).ok()??;
    reg.files
        .iter()
        .find(|f| reg.resolve(f) == wanted)
        .map(|f| f.id.clone())
}

/// **The reveals waiting their frame**, and the one part-way through.
///
/// **A `Session` resource**, like [`crate::follow::Following`] and for the same reason: a reveal raised
/// on the Kit door asks for the Map, and the switching it asks for resets every `Door` resource — so a
/// request kept in one would be erased by the very switching it asked for.
#[derive(Resource, Default)]
pub struct Revealing {
    queue: VecDeque<Request>,
    in_flight: Option<InFlight>,
    /// **The places a reveal has left and `nav` has not yet pushed** (NV-06, #383).
    ///
    /// Recorded here because this is the one system that knows a view is about to move for the person,
    /// and drained by `nav` because this system is at Bevy's parameter cap. The follow records its own
    /// place the same way and for the same reason: **the person's place is pushed where their view
    /// moves, never once per interpolation frame**.
    leaving: Vec<crate::nav::Place>,
}

/// One thing to show, with the operation that made it — what the refusal names.
#[derive(Clone, PartialEq)]
pub struct Request {
    /// The operation's registered name (`vocab_add_token`, `data_set`), as the log has it.
    pub operation: String,
    pub shown: Shown,
    /// **Whether the person asked for this page** (NV-05, #382), rather than an agent's change.
    ///
    /// Choosing a row in Go to anything is pressing Enter and then reading. The Enter is a key press,
    /// so the person's own hand has set `Following::busy` and would hold the reveal for
    /// [`crate::follow::FOLLOW_IDLE_S`] — which reads as the finder being broken — and the same press
    /// is a gesture that drops what an agent had queued. A request the person made is neither held nor
    /// dropped by the key that made it. The flag lives on the request, so it is spent with the request
    /// on every path that ends it (carried out, refused, or cleared by a project change) and cannot
    /// outlive it.
    pub by_person: bool,
}

#[derive(Clone, PartialEq)]
struct InFlight {
    request: Request,
    page: Page,
    /// **Whether the door has been asked for already.** A switching the editor refused — a question on
    /// screen, a map with unsaved edits — would otherwise be asked for every frame, for ever.
    asked: bool,
    /// **How many frames the selection has been tried**, against [`REVEAL_TRIES`].
    ///
    /// The row can be missing on the frame the switching lands: the tab's own loader reads the manifest
    /// or the notes file *after* the switching, so `BenchState::names()` is empty on the frame the bench
    /// first exists — measured, the bench arrived on rig index 0 (`cipher_field`) while the reveal was
    /// asked for `valkyrie`. The counter is what turns "the row is not there yet" into a wait instead
    /// of a silent miss, and what stops the wait from being for ever.
    tries: u32,
}

/// **How many frames a selection is retried before the reveal gives up and says so.** Two seconds at
/// 60 Hz: longer than any tab's first read (the manifest parse, the data listing, the notes file),
/// and short enough that a genuine miss is reported while the person is still looking at the page.
pub const REVEAL_TRIES: u32 = 120;

impl Revealing {
    /// **Show this change's page.** Queued, so three changes in one frame are three views.
    pub fn request(&mut self, operation: &str, shown: &Shown) {
        self.queue.push_back(Request {
            operation: operation.to_owned(),
            shown: shown.clone(),
            by_person: false,
        });
    }

    /// **Show this page because the person asked for it** (Go to anything, NV-05, #382): not held
    /// back by their idle clock, and not dropped by the key press that asked.
    pub fn request_by_person(&mut self, operation: &str, shown: &Shown) {
        self.queue.push_back(Request {
            operation: operation.to_owned(),
            shown: shown.clone(),
            by_person: true,
        });
    }

    /// **Whether the reveal that would be carried out next is one the person asked for.**
    fn person_is_waiting(&self) -> bool {
        match &self.in_flight {
            Some(in_flight) => in_flight.request.by_person,
            None => self.queue.front().is_some_and(|request| request.by_person),
        }
    }

    /// How many reveals are waiting for their frame. The one in flight is not among them.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Whether one is part-way through.
    pub fn settling(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Take the places a reveal is leaving, so `nav` can push them. Empties the list.
    pub fn take_leaving(&mut self) -> Vec<crate::nav::Place> {
        std::mem::take(&mut self.leaving)
    }

    /// **Forget the queue and the one in flight.** A project change makes them meaningless; a map
    /// change does not, because a page is a door and a row rather than a place in metres.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.in_flight = None;
        self.leaving.clear();
    }

    /// **The person reached for the view: forget what an agent's change had queued**, and keep what
    /// the person themselves asked for.
    ///
    /// The Enter that chooses a Go to anything row is a key press, and
    /// `follow::a_persons_input_takes_the_camera_back` drops the queue on every first key press — measured:
    /// the finder closed, the index held the row, and nothing moved, because the person's own Enter
    /// had cleared the reveal it made. The press that *asks* for a move is not the press that cancels
    /// it (the Look chord's rule, one surface over).
    pub fn drop_what_the_agent_queued(&mut self) {
        self.queue.retain(|request| request.by_person);
        if self.in_flight.as_ref().is_some_and(|in_flight| !in_flight.request.by_person) {
            self.in_flight = None;
        }
    }
}

/// **The inspector section a reveal last asked for**, so the panel scrolls to it.
///
/// A resource rather than a parameter because the panel is rebuilt by a system that never sees the
/// reveal, and rebuilt *after* it: the follower reads this a frame later, when the heading has a
/// `ComputedNode` (`chrome::Follow`'s rule, one panel over).
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectorReveal(pub Option<InspectorSection>);

/// The plugin: the queue, the system that carries one reveal a frame, and the two followers.
pub struct RevealPlugin;

impl Plugin for RevealPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Revealing>()
            .init_resource::<InspectorReveal>()
            .add_systems(
                Update,
                (
                    // **Before the camera is written** and after the switching: `apply_pending_door`
                    // runs in `PreUpdate` and the transition between the schedules, so by `Update` the
                    // new door's panels exist and the selection lands in state nothing will reset.
                    clear_the_reveal_when_the_project_changes.before(take_the_reveal),
                    take_the_reveal.before(crate::view::drive),
                    scroll_the_inspector_to_the_section,
                    scroll_the_selected_note_on_screen,
                )
                    .run_if(in_state(crate::screen::Screen::Editor)),
            );
    }
}

/// **Another project ends the reveals of this one.**
///
/// A page is a door and a row, and both belong to the project that has them: a queued reveal carried
/// across would cross a door for a piece, a group or a file the new project does not hold. The map is
/// **not** in this test — a page is not a place in metres, so a reveal survives a map change, and only
/// a project change makes it meaningless.
fn clear_the_reveal_when_the_project_changes(
    project: Option<Res<Project>>,
    mut revealing: ResMut<Revealing>,
    mut seen: Local<Option<std::path::PathBuf>>,
) {
    let root = project.as_deref().map(|p| p.root.clone());
    if *seen == root {
        return;
    }
    *seen = root;
    revealing.clear();
}

/// **Carry out one reveal a frame**: ask for the switching, wait for it, then select.
///
/// **One at a time**, as the follow is: three changes arrive faster than three frames, and each is a
/// page a person reads.
///
/// **This is also where NV-06's Back history is pushed** when it lands: it is the one place that knows
/// the person's view is about to move, so the place they were looking has one writer rather than one
/// per navigation verb.
#[allow(clippy::too_many_arguments)]
fn take_the_reveal(
    mut revealing: ResMut<Revealing>,
    mut state: Option<ResMut<EditorState>>,
    at: Where<'_, '_>,
    project: Option<Res<Project>>,
    mut import: Option<ResMut<crate::tiles::ImportState>>,
    mut build: Option<ResMut<crate::build::Build>>,
    mut filters: Option<ResMut<crate::filter::Filters>>,
    mut compose: Option<ResMut<crate::compose::ComposeState>>,
    mut bench: Option<ResMut<crate::anim_tab::BenchState>>,
    mut selected_note: Option<ResMut<crate::editor::SelectedNote>>,
    mut inspector: ResMut<InspectorReveal>,
    // **The person's own work holds a reveal too** (round 2 of the review, M3): a reveal that crossed
    // a door mid-drag moved the screen under their hands, which is the yank one surface over. **One
    // idle clock** — `Following`'s, set by `follow::a_persons_input_takes_the_camera_back` and read
    // here — rather than a second that could disagree with the first about whether they are working.
    mut settings: Query<&mut crate::settings_screen::SettingsDialog>,
    mut open_settings: MessageWriter<crate::settings_screen::OpenSettings>,
    mut open_config: MessageWriter<crate::project_config::OpenProjectConfig>,
    mut tables: Table<'_, '_>,
) {
    // **Taken, not cloned**: the reveal is put back at the end of the branch that continues it, so a
    // frame that finishes one costs no copy of the `Shown` the page was decided from.
    // **Nothing moves while the person is working.** The queue is untouched, so the reveal lands
    // once they stop rather than being thrown away: the change it names is still what the editor
    // promised to show.
    // **A reveal the person asked for is not held back by their own key press** (NV-05, #382): the
    // Enter that chose a Go to anything row is a gesture, and waiting out the idle clock after it
    // reads as the finder being broken. Spent on the frame it is read.
    if !revealing.person_is_waiting() && at.following.as_deref().is_some_and(crate::follow::Following::busy) {
        return;
    }
    let Some(in_flight) = revealing.in_flight.take() else {
        let Some(request) = revealing.queue.pop_front() else {
            return;
        };
        match page_of(&request.shown, project.as_deref()) {
            Ok(page) => {
                revealing.in_flight = Some(InFlight {
                    request,
                    page,
                    asked: false,
                    tries: 0,
                });
            }
            Err(why) => {
                // **A change with no page is a problem, not a wait.** Raised on the frame the editor
                // would have moved, so the person reads it beside the receipt that promised the view.
                if let Some(state) = state.as_mut() {
                    state.status.problem(format!(
                        "{} changed something the editor has no page for: {}. {}",
                        request.operation,
                        request.shown.words(),
                        why
                    ));
                }
            }
        }
        return;
    };
    let mut in_flight = in_flight;
    let done = match &in_flight.page {
        Page::Workspace {
            workspace: want_workspace,
            tab: want_tab,
            select,
        } => {
            if at.workspace.as_deref() != Some(want_workspace) || at.mode.as_deref() != Some(want_tab) {
                if let Some(state) = state.as_mut()
                    && state.door_request.is_none()
                    && !state.leaving
                {
                    if in_flight.asked {
                        // **Asked once and the editor did not cross**: a refused switching, or a
                        // question nobody has answered. The request is dropped rather than asked for
                        // again every frame — a switching the camera of a reveal nobody is waiting on
                        // more would never end (`follow`'s nit g, the same rule).
                        state.status.problem(format!(
                            "{} changed something on another page, and the editor could not open it. {}",
                            in_flight.request.operation,
                            in_flight.request.shown.words()
                        ));
                        revealing.in_flight = None;
                        return;
                    }
                    // **The place being left is recorded before the door is asked for** (NV-06,
                    // #383), so a Back from the new page returns to the one the person was reading —
                    // with its camera. It is recorded here and pushed by `nav`'s own system, because
                    // this system is at Bevy's parameter cap and a second `ResMut` would not compile.
                    revealing.leaving.push(crate::nav::Place::of(
                        at.workspace.as_deref(),
                        at.mode.as_deref(),
                        at.rig.as_deref(),
                        at.selection.as_deref(),
                    ));
                    state.door_request = Some(DoorRequest {
                        workspace: *want_workspace,
                        tab: Some(*want_tab),
                    });
                    in_flight.asked = true;
                }
                revealing.in_flight = Some(in_flight);
                return;
            }
            let selected = apply_selection(
                select,
                project.as_deref(),
                import.as_deref_mut(),
                build.as_deref_mut(),
                filters.as_deref_mut(),
                compose.as_deref_mut(),
                bench.as_deref_mut(),
                selected_note.as_deref_mut(),
                &mut inspector,
                &mut tables,
            );
            if !selected {
                in_flight.tries += 1;
                if in_flight.tries < REVEAL_TRIES {
                    revealing.in_flight = Some(in_flight);
                    return;
                }
                // **The page is open and the row never appeared.** Said rather than left as a page
                // that looks like it worked: the person reads which thing the editor could not find.
                if let Some(state) = state.as_mut() {
                    state.status.problem(format!(
                        "{} changed {}, and the editor opened its page but could not find its row",
                        in_flight.request.operation,
                        in_flight.request.shown.words()
                    ));
                }
            }
            true
        }
        // **The screen is opened by its own message**, which is how the chord and the launcher open
        // it — so a reveal, a key and a button cannot disagree about what opening it does.
        Page::Settings { tab } => {
            match settings.single_mut() {
                Ok(mut dialog) => {
                    if dialog.tab != *tab {
                        dialog.tab = *tab;
                    }
                    true
                }
                Err(_) => {
                    // The dialog is spawned by the system that reads this message, so on the frame it
                    // is asked for there is no entity to turn to a tab yet.
                    open_settings.write(crate::settings_screen::OpenSettings);
                    false
                }
            }
        }
        Page::ProjectConfig => {
            open_config.write(crate::project_config::OpenProjectConfig);
            true
        }
    };
    if done {
        revealing.in_flight = None;
    } else {
        revealing.in_flight = Some(in_flight);
    }
}

/// **Where the editor is**, and **who is working**: the two read-only facts every branch of
/// `take_the_reveal` asks.
///
/// A bundle rather than four parameters because the system is at Bevy's cap, and because these four
/// are one question — *can a reveal land right now, and on which page is the editor standing?*
#[derive(bevy::ecs::system::SystemParam)]
struct Where<'w, 's> {
    workspace: Option<Res<'w, Workspace>>,
    mode: Option<Res<'w, Mode>>,
    /// **The person's own work** — see `take_the_reveal`'s note on one idle clock.
    following: Option<Res<'w, crate::follow::Following>>,
    /// **The camera and the selection**, read only to record the place a reveal is leaving (NV-06,
    /// #383). Read-only on purpose: a reveal writes a selection and never the camera, so nothing here
    /// can move the view.
    rig: Option<Res<'w, crate::view::Rig>>,
    selection: Option<Res<'w, crate::selection::Selection>>,
    _marker: std::marker::PhantomData<&'s ()>,
}

/// The parameters a data-file reveal needs, bundled: the system is near Bevy's twenty-parameter cap
/// and these are one subject — the DATA table and the viewport it scrolls in.
#[derive(bevy::ecs::system::SystemParam)]
struct Table<'w, 's> {
    rows: Query<
        'w,
        's,
        (
            Entity,
            &'static mut bevy_gooey::TableCursor,
            &'static mut bevy_gooey::TableSort,
            &'static bevy_gooey::Table,
        ),
    >,
    children: Query<'w, 's, &'static Children>,
    scrolls: Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static bevy_gooey::ListView,
            &'static mut ScrollPosition,
        ),
    >,
    /// **The bench's stage and what it plays**, which a clip reveal waits on and writes (NV-11, #391).
    staged: Query<'w, 's, &'static crate::anim_stage::BenchStageOf, With<crate::anim_stage::BenchStage>>,
    scrub: Option<ResMut<'w, crate::anim_stage::BenchScrub>>,
    /// **The cue the Sound workspace's list marks as selected**, which a cue reveal writes (NV-07, #384).
    cue: Option<ResMut<'w, crate::world_lists::SelectedCue>>,
}

/// **Write the page's selection**, and answer whether the row it names was there.
///
/// **`false` is a wait, not a failure**: a tab reads its own manifest, notes file or data registry
/// after the switching, so the row a page is asked for may not exist on the frame the page does. The
/// caller retries for [`REVEAL_TRIES`] frames, so a loader's first read is a wait rather than a silent
/// miss, and reports the miss only when the row never arrives.
///
/// No document is touched — see the module note.
#[allow(clippy::too_many_arguments)]
fn apply_selection(
    select: &Select,
    project: Option<&Project>,
    import: Option<&mut crate::tiles::ImportState>,
    build: Option<&mut crate::build::Build>,
    filters: Option<&mut crate::filter::Filters>,
    compose: Option<&mut crate::compose::ComposeState>,
    bench: Option<&mut crate::anim_tab::BenchState>,
    selected_note: Option<&mut crate::editor::SelectedNote>,
    inspector: &mut InspectorReveal,
    tables: &mut Table<'_, '_>,
) -> bool {
    let mut want: Option<InspectorSection> = None;
    // **Whether the row the selection names was found.** An inspector section alone is always
    // "found": there is no row to wait for.
    let mut found = true;
    match select {
        Select::Piece(id) => {
            // **A row the filter hides cannot be shown.** `keep_library_selection_visible` moves a
            // hidden selection off it on the next frame, so a reveal that left the filter alone would
            // be undone silently. Showing the change wins over a narrowing typed earlier, which one
            // keystroke types again.
            if let Some(filters) = filters
                && !filters.keeps(crate::filter::Pane::Candidates, id)
            {
                filters.clear(crate::filter::Pane::Candidates);
            }
            if let Some(import) = import {
                import.focus_on(&crate::tiles::EditTarget::Library(id.clone()));
            }
            // The shelf draws the library page, not the kit's tiles.
            if let Some(build) = build {
                build.browsing = None;
            }
        }
        Select::Group(id) => {
            match (compose, project) {
                (Some(compose), Some(project)) => {
                    match project.compositions.compositions.iter().position(|c| &c.id == id) {
                        Some(at) => {
                            compose.selected = at;
                            compose.focus = crate::compose::Pane::Groups;
                        }
                        None => found = false,
                    }
                }
                _ => found = false,
            }
        }
        Select::Rig(id) => {
            match bench {
                Some(bench) => match bench.names().iter().position(|n| *n == id) {
                    Some(at) => bench.selected = at,
                    // The manifest has not been read yet on the frame the bench first exists.
                    None => found = false,
                },
                None => found = false,
            }
        }
        Select::Clip { rig, slot } => {
            // **The rig first, then the stage, then the slot.** Selecting the rig makes the bench stage its mesh, and a fresh stage starts
            // from the default mix (`anim_stage::drive_bench_stage`): a solo written before the stage is the rig's would be reset by it.
            // So the slot is soloed on the frame the stage is the selected rig's, and until then the page is not found.
            found = false;
            if let Some(bench) = bench {
                let at = bench.names().iter().position(|n| *n == rig);
                let rig_def = bench.rigs.as_ref().and_then(|rigs| rigs.get(rig));
                let has_slot = rig_def.is_some_and(|def| *slot < def.slots.len());
                let mesh = rig_def.map(|def| def.mesh.clone());
                if let (Some(at), true, Some(mesh)) = (at, has_slot, mesh) {
                    bench.selected = at;
                    bench.view = crate::anim_tab::View::One;
                    if tables.staged.iter().any(|stage| stage.is_of(&mesh))
                        && let Some(scrub) = tables.scrub.as_mut()
                    {
                        scrub.solo = Some(*slot);
                        scrub.mixed.clear();
                        found = true;
                    }
                }
            }
        }
        Select::DataFile(file) => {
            want = Some(InspectorSection::Data);
            found = match project {
                Some(project) => reveal_the_data_row(project, file, &mut tables.rows, &tables.children, &mut tables.scrolls),
                None => false,
            };
        }
        Select::Note(id) => {
            want = Some(InspectorSection::Notes);
            if let Some(selected_note) = selected_note {
                selected_note.0 = Some(id.clone());
            }
        }
        Select::Cue(name) => {
            // **A cue the project does not hold is a row that never appears**, so the reveal says so rather than marking nothing.
            found = project.is_some_and(|p| crate::world_lists::cue_names_of(p).is_ok_and(|names| names.contains(name)));
            if found && let Some(cue) = tables.cue.as_mut() {
                cue.0 = Some(name.clone());
            }
        }
        Select::Section(section) => want = Some(*section),
        Select::Nothing => {}
    }
    if inspector.0 != want {
        inspector.0 = want;
    }
    found
}

/// **Put the DATA table's cursor on the file's first leaf, and scroll it there.**
///
/// **gooey scrolls to its cursor only for a key or a click** (`table::pick` arms the cache flag
/// `update_tables` reads), so a cursor written from code would name a row the table never brings into
/// view. The scroll is computed here from the viewport's own geometry — its height over the rows
/// `ListView` says it shows is one row's pitch — and the sort is cleared first, because a column the
/// person sorted would put the leaf somewhere other than its index. Clearing it is what a search
/// result does to a sorted column everywhere.
fn reveal_the_data_row(
    project: &Project,
    file: &str,
    tables: &mut Query<(Entity, &mut bevy_gooey::TableCursor, &mut bevy_gooey::TableSort, &bevy_gooey::Table)>,
    children: &Query<&Children>,
    scrolls: &mut Query<(&ComputedNode, &UiGlobalTransform, &bevy_gooey::ListView, &mut ScrollPosition)>,
) -> bool {
    let leaves = crate::ops_data::listing(&project.grimoire_dir);
    let Some(row) = leaves.iter().position(|l| l.file == file) else {
        // The registry has not been read, or it does not list this file: the panel's own text says
        // which, and the caller waits on the first and reports on the second.
        return false;
    };
    // **The table may not be built yet.** It is spawned by the panel's own refresh, which runs after
    // the switching; a listing with rows and no table is a wait.
    let mut selected = false;
    for (table_entity, mut cursor, mut sort, table) in tables.iter_mut() {
        // **The DATA table first, and only it.** A sort cleared on every table in the world would
        // reorder a person's table the reveal never looked at — and the panel is not the only table
        // an editor may be drawing.
        let Some(rows) = table.source::<crate::editor::DataRows>() else { continue };
        if sort.column.is_some() {
            *sort = bevy_gooey::TableSort::default();
        }
        // **The key gooey derives for a row**, asked of the table's own source: `RowSource::key` is the
        // one function that says how a row is named, so this cannot disagree with a click on that row.
        let key = bevy_gooey::RowSource::key(rows, row);
        if cursor.0 != Some(key) {
            cursor.0 = Some(key);
        }
        selected = true;
        // The viewport is the nearest descendant of the table that is a `ListView`: the table's own
        // scrolled box, which is where it keeps how many rows it shows.
        let Some(viewport) = children.iter_descendants(table_entity).find(|e| scrolls.contains(*e)) else {
            continue;
        };
        let Ok((node, at, view, mut scroll)) = scrolls.get_mut(viewport) else { continue };
        if view.rows <= 0.0 || node.size().y <= 0.0 {
            continue;
        }
        // Physical pixels in, logical out, as every follower in this crate converts: `ComputedNode` is
        // physical and `ScrollPosition` is logical, and `inverse_scale_factor` is the conversion.
        // **The viewport's own bounds, read the one way every follower in this crate reads them**
        // (`chrome::scroll_bounds`): Bevy writes the clamped value into `ComputedNode::scroll_position`
        // and leaves the `ScrollPosition` component alone, so a follower that fed the component back
        // would compute every later reveal from a place the list never went.
        // **The two numbers `scroll_to_reveal` wants are both ON SCREEN, and both physical.** It
        // compares the row's edges with the list's, so a row's offset from the top of the list's
        // *content* has to be moved to where it would be drawn: the viewport's own screen top, plus
        // the row's offset, minus how far the list has scrolled (physical: a scroll position is
        // logical, `ComputedNode` geometry is not, and `inverse_scale_factor` is the conversion).
        //
        // Measured before this was written: the row was given a bare content offset and the list its
        // screen centre, so the reveal saw a row "above the fold" for every table whose content
        // started at zero and scrolled the list to a place the leaf was not.
        let (scroll_y, max) = crate::chrome::scroll_bounds(node);
        let inv = node.inverse_scale_factor();
        let Some((row_mid, pitch)) = table_row_geometry(
            (at.translation.y, node.size().y),
            view.rows,
            row,
            scroll_y,
            inv,
        ) else {
            continue;
        };
        let Some(want) = crate::chrome::scroll_to_reveal(
            (row_mid, pitch * 0.5),
            (at.translation.y, node.size().y * 0.5),
            scroll_y,
            max,
            inv,
        ) else {
            continue;
        };
        if (scroll.0.y - want).abs() > 0.5 {
            scroll.0.y = want;
        }
    }
    selected
}

/// **Where a table's `row` is on screen, and one row's pitch** — both **physical** pixels, which is
/// the space `chrome::scroll_to_reveal` compares in, and the space `ComputedNode` reports geometry in.
///
/// `viewport` is the scrolled box's screen centre and physical height; `scroll` is where it is
/// scrolled to in **logical** pixels (`ScrollPosition` is logical, `ComputedNode` is not), and
/// `inverse_scale` is the conversion between them.
///
/// **The conversion is where this was wrong.** The first version wrote
/// `(scroll / inverse_scale).max(0.0) * inverse_scale`, which is the scroll in logical pixels again —
/// an identity that happens to be right at scale 1 and is short by half at scale 2, so on a Mac the
/// reveal under-scrolled every table by exactly the distance it had already scrolled. A row's
/// distance from the top of the content is a physical figure, so the scroll subtracted from it must
/// be physical too: `scroll / inverse_scale` (logical × scale = physical), and nothing after it.
pub fn table_row_geometry(
    viewport: (f32, f32),
    rows_shown: f32,
    row: usize,
    scroll: f32,
    inverse_scale: f32,
) -> Option<(f32, f32)> {
    let (centre_y, height) = viewport;
    if rows_shown <= 0.0 || height <= 0.0 || inverse_scale <= 0.0 {
        return None;
    }
    // One row's pitch, from the viewport's own height over the rows `ListView` says it shows — the
    // only public answer to a gooey table's row height.
    let pitch = height / rows_shown;
    let scrolled_physical = (scroll / inverse_scale).max(0.0);
    let row_mid = centre_y - height * 0.5 + row as f32 * pitch + pitch * 0.5 - scrolled_physical;
    Some((row_mid, pitch))
}

/// **Scroll the Map door's inspector so the revealed heading is on screen.**
///
/// `chrome::Follow`'s two-step, for its reason: the panel is rebuilt on the change, so this frame's
/// `ComputedNode` describes the previous layout. Keyed on the section rather than on the resource's
/// change flag, so an unrelated write cannot re-arm it.
fn scroll_the_inspector_to_the_section(
    reveal: Res<InspectorReveal>,
    headings: Query<(&Section, &ComputedNode, &UiGlobalTransform)>,
    mut panes: Query<
        (&ComputedNode, &UiGlobalTransform, &mut ScrollPosition),
        With<crate::editor::MapControlsPane>,
    >,
    mut follow: Local<crate::chrome::Follow<InspectorSection>>,
) {
    if !follow.should_scroll(reveal.0) {
        return;
    }
    let Some(want) = reveal.0 else { return };
    let Some((row_mid, row_half)) = headings
        .iter()
        .find(|(s, _, _)| s.0 == want)
        .map(|(_, n, t)| (t.translation.y, n.size.y * 0.5))
    else {
        return;
    };
    for (pane, pane_tf, mut scroll) in &mut panes {
        let (at, max) = crate::chrome::scroll_bounds(pane);
        if let Some(want) = crate::chrome::scroll_to_reveal(
            (row_mid, row_half),
            (pane_tf.translation.y, pane.size().y * 0.5),
            at,
            max,
            pane.inverse_scale_factor,
        ) {
            scroll.0.y = want;
        }
    }
}

/// **Scroll the Map door's inspector so the selected note's row is on screen.**
///
/// The Notes list is a column inside the inspector's scroll, not a `scroll_list` of its own, so its
/// follower scrolls the panel — `keep_note_box_on_screen`'s shape, one row over. Keyed on the id.
fn scroll_the_selected_note_on_screen(
    selected: Option<Res<crate::editor::SelectedNote>>,
    rows: Query<(&crate::editor::NoteRow, &ComputedNode, &UiGlobalTransform)>,
    mut panes: Query<
        (&ComputedNode, &UiGlobalTransform, &mut ScrollPosition),
        With<crate::editor::MapControlsPane>,
    >,
    mut follow: Local<crate::chrome::Follow<String>>,
) {
    let Some(id) = selected.as_deref().and_then(|s| s.0.clone()) else {
        // Nothing selected: the follower forgets, so the next selection scrolls.
        follow.should_scroll(None);
        return;
    };
    if !follow.should_scroll(Some(id.clone())) {
        return;
    }
    let Some((row_mid, row_half)) = rows
        .iter()
        .find(|(r, _, _)| r.id == id)
        .map(|(_, n, t)| (t.translation.y, n.size.y * 0.5))
    else {
        return;
    };
    for (pane, pane_tf, mut scroll) in &mut panes {
        let (at, max) = crate::chrome::scroll_bounds(pane);
        if let Some(want) = crate::chrome::scroll_to_reveal(
            (row_mid, row_half),
            (pane_tf.translation.y, pane.size().y * 0.5),
            at,
            max,
            pane.inverse_scale_factor,
        ) {
            scroll.0.y = want;
        }
    }
}
