//! **Each waiting wish is a row in the AGENT block, and the person opens it or discards it** (#354,
//! AW-02).
//!
//! AW-01 (#353) queues a wish: the one tool that would have helped, its four fields, and the issue
//! address the editor built when the wish was made. This module is the person's half. The wish is a
//! row in the Map door's AGENT block (`familiar`): whose it is, the `tool` line, and the three other
//! fields when it is unfolded. Two answers, and both reach the agent through
//! `grimoire_state.wishes`:
//!
//! - **Open on GitHub** opens the wish's own address with the editor's one opener
//!   ([`crate::app_verbs::open_link`]). The person reviews the filled-in form on GitHub and presses
//!   Submit. **The editor never posts**: `opened` means the page was opened, not that an issue
//!   exists, and the editor does not claim to know.
//! - **Discard** drops the wish and opens nothing.
//!
//! # The rows are shared, not the wish's
//!
//! AG-06's lessons (#351) join the same list, so a row is written for more than one kind from the
//! start: [`RowKind`] names which, and a row has a one-line title, an expandable body and its own
//! answers. Nothing here puts a `Wish` where a lesson cannot stand.
//!
//! # Matching wishes are shown before opening
//!
//! Unfolding a row asks GitHub's public search for up to [`SEARCH_SHOWN`] open `agent-wish` issues on
//! [`WISH_REPO`] that match the `tool` line, and lists them as links, so a person can join a wish that
//! exists instead of filing a second one. **The check informs and never blocks**: a search that fails
//! says why in one line, and Open on GitHub still opens.
//!
//! The search reaches GitHub through `ureq` without a token, as `update.rs` reaches releases, on a
//! task-pool thread, so a slow answer never holds a frame. A headless run asks nobody:
//! [`WishSearchOff`] is the seam a test — and the harness, which has no network — puts in its place.
//!
//! # While the block is hidden
//!
//! A status-band chip counts the waiting rows ("2 waiting for you"), and a click on it opens the
//! block on the first of them. The activity bar's Agent badge (NV-03, #380) replaces the chip when it
//! lands.

use std::collections::BTreeSet;
use std::time::Duration;

use bevy::prelude::*;
use bevy::tasks::Task;
use bevy::picking::hover::Hovered;
use bevy_gooey::{Activate, Role};
use bevy_grimoire_core::wish::{percent_encode, WISH_REPO};

use crate::present::{QueuedWish, WishSearch, WishState, Wishes};
use crate::screen::Screen;

/// **How many matching issues a row lists**: enough to see a duplicate, few enough to read.
pub const SEARCH_SHOWN: usize = 3;

/// **How long the search may take**, in seconds, before the row says GitHub did not answer. A check
/// that informs must not be the reason a row stands blank for minutes.
pub const SEARCH_TIMEOUT_S: u64 = 6;

/// **The address the search asks**: GitHub's public issue search, over this repository's open wishes.
///
/// **The tool's own name, not its sentence**: the words a duplicate would share are the two or three
/// in the tool's name, and a 200-character query matches nothing. So the first word of the `tool`
/// line goes in, which is the name an agent writes first by [`bevy_grimoire_core::wish`]'s own rule
/// ("its name and one sentence").
pub fn search_url(tool: &str) -> String {
    let named = tool.lines().next().unwrap_or_default();
    let name = named
        .split([':', ',', ' ', '.', '(', '`'])
        .find(|word| !word.is_empty())
        .unwrap_or_default();
    let query = format!("repo:{WISH_REPO} is:issue is:open label:agent-wish {name}");
    format!("https://api.github.com/search/issues?per_page={SEARCH_SHOWN}&q={}", percent_encode(&query))
}

/// **One issue the search matched**: its number, its title, and the address a person opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WishMatch {
    pub number: u64,
    pub title: String,
    pub url: String,
}

/// **A run that asks GitHub nothing**, and answers every search with what it is given.
///
/// A test inserts one to prove both halves of the rule — a search that matched and one that failed —
/// and the harness inserts the failing answer, because a test process has no business asking the
/// network and a suite that did would fail on an aeroplane. It is
/// [`crate::launcher::ClipboardOff`]'s shape: the seam is a resource the caller inserts, never a
/// branch in the code that ships.
#[derive(Resource, Clone, Debug)]
pub struct WishSearchOff(pub Result<Vec<WishMatch>, String>);

impl Default for WishSearchOff {
    /// **This run asks GitHub nothing**, so every search says so and the person is still free to open.
    fn default() -> WishSearchOff {
        WishSearchOff(Err("this run asks GitHub nothing".to_owned()))
    }
}

/// One search in flight: the wish it is for, the tool words it asked with, and the task.
struct SearchInFlight {
    id: String,
    asked: String,
    task: Task<Result<Vec<WishMatch>, String>>,
}

/// **The searches in flight**, by wish id, so a row that is rebuilt does not lose the answer it asked
/// for. The tool words it was asked with travel beside it: an answer to a question the row no longer
/// asks is dropped rather than shown against different words.
#[derive(Resource, Default)]
pub struct WishSearches(Vec<SearchInFlight>);

/// The rows, the searches and the band's count of them.
pub struct WishViewPlugin;

impl Plugin for WishViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WishOpenRows>()
            .init_resource::<WishSearches>()
            .add_systems(OnEnter(Screen::Editor), spawn_the_chip.after(crate::chrome::FrameSystems))
            .add_systems(
                Update,
                (ask_the_unfolded, land_the_searches, follow_the_rows, follow_the_search_lines, follow_the_chip)
                    .run_if(in_state(Screen::Editor)),
            );
    }
}

/// **Which rows the person has unfolded**, by wish id. Session state: a row read on one door is read
/// on the next, for the reason the wish queue itself survives a door change.
#[derive(Resource, Default)]
pub struct WishOpenRows(BTreeSet<String>);

impl WishOpenRows {
    /// Whether this row is unfolded.
    pub fn is_open(&self, id: &str) -> bool {
        self.0.contains(id)
    }

    /// Unfold or fold the row, and say what it is now.
    pub fn toggle(&mut self, id: &str) -> bool {
        if self.0.remove(id) {
            false
        } else {
            self.0.insert(id.to_owned());
            true
        }
    }

    /// Unfold the row without touching the others.
    pub fn open(&mut self, id: &str) {
        self.0.insert(id.to_owned());
    }
}

// ── what a person sees ───────────────────────────────────────────────────────────────────────────

/// **A row in the AGENT block's list**, whatever kind it is. AG-06's lessons join these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// A wish: the one tool that would have helped, filed as an issue on [`WISH_REPO`].
    Wish,
}

impl RowKind {
    /// The word the row shows for its kind.
    pub fn label(self) -> &'static str {
        match self {
            RowKind::Wish => "WISH",
        }
    }
}

/// **One row of the AGENT block's shared list.**
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct WishRow {
    /// The wish's id (`wish-1`), which every part of the row names.
    pub id: String,
    pub kind: RowKind,
}

/// **The wish a row part belongs to.** Every part of a row carries one, so a test — and the systems
/// that answer a row — find the part they mean without a payload per marker naming the same id twice.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct WishOf(pub String);

/// The row's one-line title: whose it is, and the tool that would have helped.
#[derive(Component)]
pub struct WishTitle;

/// **The row unfolded**: the wish's fields, the search's answer and the two buttons. There only while
/// the row is unfolded, which is why the rows are rebuilt rather than edited in place.
#[derive(Component)]
pub struct WishBody;

/// One `NAME  words` line of an unfolded row — `TOOL`, `TASK`, `HARDSHIP`, `CALL`, `ABOUT`.
#[derive(Component)]
pub struct WishField {
    pub name: &'static str,
}

/// **The line that says what GitHub's search found**, or why it could not be asked.
#[derive(Component)]
pub struct WishSearchLine;

/// The row's answer, when one was already given: `opened` or `discarded`.
#[derive(Component)]
pub struct WishAnswered(pub WishState);

/// The row's Open on GitHub button.
#[derive(Component)]
pub struct WishOpen;

/// The row's Discard button.
#[derive(Component)]
pub struct WishDiscard;

/// The row's clickable header, which unfolds it.
#[derive(Component)]
pub struct WishUnfold;

/// **The status band's count of waiting rows**, shown while the AGENT block is hidden.
#[derive(Component)]
pub struct WishChip;

/// The chip's words, on the child that draws them (a `StatusItem`'s text is its child, as the jobs
/// item's is).
#[derive(Component)]
pub struct WishChipText;

/// What the chip last said, so the band writes only when the count moves.
#[derive(Component, Clone, PartialEq, Eq)]
struct ChipShown(String);

/// **The list's container inside the AGENT block**: one child per row, rebuilt when the wishes move.
#[derive(Component)]
pub struct WishRows;

// ── the block's list ────────────────────────────────────────────────────────────────────────────

/// **Everything a row shows, in one value** — which is what decides whether the rows are rebuilt.
///
/// A fixed set of pre-spawned rows could not follow this: a body and its fields exist only while the
/// row is unfolded, and the two buttons leave when the person answers, so the set of nodes a row is
/// made of changes with the wish.
#[derive(Component, Clone, PartialEq, Eq)]
struct RowsShown(Vec<RowShown>);

/// **One row's own state**, as the list is rebuilt by it. A `Vec` of these is what `RowsShown` compares,
/// so a frame that changes nothing rebuilds nothing.
///
/// **The client is part of it** (#354's review): a row spawned before `grimoire/agent_hello` said who
/// was talking reads `an agent`, and the row has to be rebuilt when the name arrives — otherwise the
/// block names nobody for the whole sitting.
#[derive(Clone, PartialEq, Eq)]
struct RowShown {
    id: String,
    unfolded: bool,
    state: WishState,
    note: Option<String>,
    client: Option<(String, String)>,
}

impl RowsShown {
    fn of(wishes: &Wishes, open: &WishOpenRows, client: Option<&crate::present::ClientName>) -> RowsShown {
        RowsShown(
            wishes
                .queued
                .iter()
                .map(|q| RowShown {
                    id: q.wish.id.clone(),
                    unfolded: open.is_open(&q.wish.id),
                    state: q.state,
                    // **The search's answer is not here**: it is one line of text, written in place by
                    // `follow_the_search_lines`. Putting it in this list would rebuild every row on the
                    // frame a search came back, and a row is a whole subtree.
                    note: q.note.clone(),
                    client: client.map(|c| (c.name.clone(), c.version.clone())),
                })
                .collect(),
        )
    }
}

/// **A row's one-line title**: whose it is and the tool that would have helped.
///
/// It carries the issue's own title, so the row a person reads and the issue they open cannot say
/// different things about which tool a wish is about.
pub fn title_of(queued: &QueuedWish, client: Option<&crate::present::ClientName>) -> String {
    let who = client.map_or_else(|| "an agent".to_owned(), |c| format!("{} {}", c.name, c.version));
    format!("{who}: {}", queued.wish.title())
}

/// The body of an unfolded row: the agent's four fields in the order a reader fixes them, and the
/// editor's own record of the turn.
pub fn fields_of(queued: &QueuedWish) -> [(&'static str, String); 5] {
    [
        ("TOOL", queued.wish.tool.clone()),
        ("TASK", queued.wish.task.clone()),
        ("HARDSHIP", queued.wish.hardship.clone()),
        ("CALL", queued.wish.example_call.clone()),
        ("ABOUT", queued.wish.context.clone()),
    ]
}

/// **The search's one line follows its answer**, in place: a row that was asking says what it found
/// (or why GitHub could not be asked) without being rebuilt, which matters because a row is a subtree
/// of fields and buttons and a search answers on a frame nobody chose.
fn follow_the_search_lines(wishes: Option<Res<Wishes>>, mut lines: Query<(&mut Text, &WishOf), With<WishSearchLine>>) {
    let Some(wishes) = wishes else { return };
    for (mut text, of) in &mut lines {
        let Some(queued) = wishes.queued.iter().find(|q| q.wish.id == of.0) else { continue };
        let said = queued.search.sentence();
        if text.0 != said {
            text.0 = said;
        }
    }
}

/// **The block's list follows the wishes**: a row per wish, saying everything it is.
fn follow_the_rows(
    mut commands: Commands,
    wishes: Option<Res<Wishes>>,
    open: Option<Res<WishOpenRows>>,
    client: Option<Res<crate::present::AgentClient>>,
    lists: Query<(Entity, Option<&RowsShown>), With<WishRows>>,
    rows: Query<Entity, With<WishRow>>,
) {
    let (Some(wishes), Some(open)) = (wishes, open) else { return };
    let client = client.as_deref().and_then(|c| c.0.as_ref());
    let now = RowsShown::of(&wishes, &open, client);
    for (list, built) in &lists {
        if built.is_some_and(|built| *built == now) {
            continue;
        }
        for row in &rows {
            commands.entity(row).try_despawn();
        }
        commands.entity(list).insert(now.clone()).with_children(|list| {
            for queued in &wishes.queued {
                spawn_a_row(list, queued, &open, client);
            }
        });
    }
}

/// One row: the header (kind, title, unfold), and — unfolded — its fields, the search line and the
/// two answers, or the answer already given.
fn spawn_a_row(
    list: &mut ChildSpawnerCommands,
    queued: &QueuedWish,
    open: &WishOpenRows,
    client: Option<&crate::present::ClientName>,
) {
    let id = queued.wish.id.clone();
    let unfolded = open.is_open(&id);
    let answered = queued.state;
    list.spawn((
        WishRow { id: id.clone(), kind: RowKind::Wish },
        WishOf(id.clone()),
        Name::new(format!("wish {}", queued.wish.id)),
        Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0),
            flex_shrink: 0.0,
            row_gap: Val::Px(crate::chrome::GAP_TIGHT),
            ..default()
        },
    ))
    .with_children(|row| {
        // **The header is the row's own unfold**, which is what a person expects of a list row and
        // keeps the body one press away.
        row.spawn((
            WishUnfold,
            WishOf(id.clone()),
            bevy::ui_widgets::Button,
            Hovered::default(),
            Node {
                flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                row_gap: Val::Px(crate::chrome::GAP_TIGHT),
                ..default()
            },
        ))
        .observe(unfold_this_row)
        .with_children(|header| {
            header
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(crate::chrome::GAP_TIGHT),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|line| {
                    line.spawn((
                        Text::new(RowKind::Wish.label()),
                        TextColor(crate::chrome::DIM),
                        crate::chrome::font(crate::chrome::text::LABEL),
                    ));
                    line.spawn((
                        WishTitle,
                        WishOf(id.clone()),
                        Text::new(title_of(queued, client)),
                        TextColor(crate::chrome::TEXT),
                        crate::chrome::font(crate::chrome::text::BODY),
                        Node { flex_grow: 1.0, min_width: Val::Px(0.0), ..default() },
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                    ));
                });
            // **The tool line is on the row either way**: it is the one field a person reads to
            // decide whether the wish is worth unfolding.
            line_of(header, "TOOL", &id, &queued.wish.tool);
            if answered != WishState::Waiting {
                header.spawn((
                    WishAnswered(answered),
                    WishOf(id.clone()),
                    Text::new(format!("you {} it", answered.as_str())),
                    TextColor(crate::chrome::DIM),
                    crate::chrome::font(crate::chrome::text::LABEL),
                ));
            }
        });
        if !unfolded {
            return;
        }
        row.spawn((
            WishBody,
            WishOf(id.clone()),
            Node { flex_direction: FlexDirection::Column, flex_shrink: 0.0, row_gap: Val::Px(crate::chrome::GAP_TIGHT), ..default() },
        ))
        .with_children(|body| {
            for (name, words) in fields_of(queued) {
                // The tool is already on the header, and saying it twice in one row is a row that
                // teaches nothing.
                if name == "TOOL" {
                    continue;
                }
                line_of(body, name, &id, &words);
            }
            body.spawn((
                WishSearchLine,
                WishOf(id.clone()),
                Text::new(queued.search.sentence()),
                TextColor(crate::chrome::DIM),
                crate::chrome::font(crate::chrome::text::LABEL),
                Node { flex_shrink: 0.0, ..default() },
                TextLayout::new(Justify::Left, LineBreak::WordBoundary),
            ));
            if let Some(note) = &queued.note {
                body.spawn((
                    Text::new(format!("GitHub did not open: {note}")),
                    TextColor(crate::chrome::DANGER),
                    crate::chrome::font(crate::chrome::text::LABEL),
                    Node { flex_shrink: 0.0, ..default() },
                    TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                ));
            }
            if queued.state != WishState::Waiting {
                return;
            }
            body.spawn(Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(crate::chrome::GAP_TIGHT),
                flex_shrink: 0.0,
                ..default()
            })
            .with_children(|answers| {
                crate::chrome::button(answers, (WishOpen, WishOf(id.clone())), "Open on GitHub", crate::chrome::Severity::Plain)
                    .observe(open_this_wish);
                crate::chrome::button(answers, (WishDiscard, WishOf(id.clone())), "Discard", crate::chrome::Severity::Plain)
                    .observe(discard_this_wish);
            });
        });
    });
}

/// One `NAME  words` line of a row, carrying the field's own name so a test can ask for it.
fn line_of(parent: &mut ChildSpawnerCommands, name: &'static str, id: &str, words: &str) {
    parent
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::FlexStart, flex_shrink: 0.0, ..default() })
        .with_children(|line| {
            crate::chrome::row_label(line, crate::editor::READOUT_LABEL_W, name);
            line.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_width: Val::Px(0.0), ..default() })
                .with_children(|col| {
                    col.spawn((
                        Text::new(words.to_owned()),
                        TextColor(crate::chrome::TEXT),
                        crate::chrome::font(crate::chrome::text::BODY),
                        WishField { name },
                        WishOf(id.to_owned()),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                    ));
                });
        });
}

/// **A press on a row unfolds it**, and a second press folds it again.
fn unfold_this_row(activate: On<Activate>, rows: Query<&WishOf, With<WishUnfold>>, mut open: Option<ResMut<WishOpenRows>>) {
    let Ok(row) = rows.get(activate.entity) else { return };
    let Some(open) = open.as_mut() else { return };
    open.toggle(&row.0);
}

/// **Open on GitHub**: the wish's own address, through the editor's one opener.
///
/// **The person's act, not an operation** — like a trust grant, so no agent can press it. A failed
/// open (no opener on the system) leaves the wish `waiting` and names the reason on the row: a page
/// that did not open is not an answer.
fn open_this_wish(
    activate: On<Activate>,
    buttons: Query<&WishOf, With<WishOpen>>,
    wishes: Option<Res<Wishes>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut commands: Commands,
) {
    let Ok(button) = buttons.get(activate.entity) else { return };
    let id = button.0.clone();
    let Some(queued) = wishes.as_deref().and_then(|w| w.queued.iter().find(|q| q.wish.id == id)) else { return };
    if queued.state != WishState::Waiting {
        return;
    }
    let url = queued.url.clone();
    let title = queued.wish.title();
    if let Some(state) = state.as_mut() {
        state.status.note(format!("opening {title} on GitHub"));
    }
    // **Through the one opener**, queued on the world because an opener is a process, not a frame's
    // work — and because a headless run records the address instead of launching a browser.
    commands.queue(move |world: &mut World| {
        let opened = crate::app_verbs::open_in_this_run(world, &url);
        if let Some(mut wishes) = world.get_resource_mut::<Wishes>()
            && let Some(queued) = wishes.queued.iter_mut().find(|q| q.wish.id == id)
        {
            match &opened {
                Ok(()) => queued.state = WishState::Opened,
                Err(reason) => queued.note = Some(reason.clone()),
            }
        }
        if let Some(mut state) = world.get_resource_mut::<crate::editor::EditorState>() {
            match opened {
                Ok(()) => state.status.note(format!("opened {title} on GitHub")),
                Err(reason) => state.status.problem(format!("GitHub did not open: {reason}")),
            }
        }
    });
}

/// **Discard**: the wish is dropped, and nothing is opened.
fn discard_this_wish(
    activate: On<Activate>,
    buttons: Query<&WishOf, With<WishDiscard>>,
    mut wishes: Option<ResMut<Wishes>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Ok(button) = buttons.get(activate.entity) else { return };
    let Some(wishes) = wishes.as_mut() else { return };
    let id = button.0.clone();
    let Some(queued) = wishes.queued.iter_mut().find(|q| q.wish.id == id) else { return };
    if queued.state != WishState::Waiting {
        return;
    }
    queued.state = WishState::Discarded;
    let title = queued.wish.title();
    if let Some(state) = state.as_mut() {
        state.status.note(format!("discarded {title}"));
    }
}

// ── GitHub's search ─────────────────────────────────────────────────────────────────────────────

/// **Ask GitHub what already exists**, once per row the person unfolds.
///
/// Asked here rather than by the button that unfolds the row, so **every** way a row comes to be
/// unfolded — a press on it, or the status band's chip opening the block on it — asks the same
/// question once. A row already asked is not asked again: the line is the record of what was found,
/// and re-asking on every unfold would spend the person's rate limit to say the same thing.
fn ask_the_unfolded(
    mut wishes: Option<ResMut<Wishes>>,
    open: Option<Res<WishOpenRows>>,
    off: Option<Res<WishSearchOff>>,
    mut searches: Option<ResMut<WishSearches>>,
) {
    let (Some(wishes), Some(open)) = (wishes.as_mut(), open) else { return };
    for queued in wishes.queued.iter_mut() {
        if !open.is_open(&queued.wish.id) || queued.search != WishSearch::NotAsked {
            continue;
        }
        let tool = queued.wish.tool.clone();
        // **A run that asks nobody**, which is the harness and every test that has not said otherwise.
        if let Some(off) = off.as_deref() {
            queued.search = match off.0.clone() {
                Ok(matches) => WishSearch::Found(matches),
                Err(reason) => WishSearch::Failed(reason),
            };
            queued.asked.clone_from(&tool);
            continue;
        }
        let Some(searches) = searches.as_mut() else { continue };
        let id = queued.wish.id.clone();
        let url = search_url(&tool);
        queued.search = WishSearch::Asking;
        queued.asked.clone_from(&tool);
        let task = bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { ask_github(&url) });
        searches.0.push(SearchInFlight { id, asked: tool, task });
    }
}

/// **Land the searches that have finished**, and drop the ones whose wish is gone.
fn land_the_searches(mut searches: Option<ResMut<WishSearches>>, mut wishes: Option<ResMut<Wishes>>) {
    let (Some(searches), Some(wishes)) = (searches.as_mut(), wishes.as_mut()) else { return };
    let mut done = Vec::new();
    for (i, flight) in searches.0.iter_mut().enumerate() {
        let Some(answer) = bevy::tasks::futures::check_ready(&mut flight.task) else { continue };
        done.push(i);
        let Some(queued) = wishes.queued.iter_mut().find(|q| q.wish.id == flight.id) else { continue };
        // **An answer to a question the row no longer asks is dropped**, so a tool edited between
        // the ask and the answer cannot be shown one wish's matches against another's words.
        if queued.asked != flight.asked {
            continue;
        }
        queued.search = match answer {
            Ok(matches) => WishSearch::Found(matches),
            Err(reason) => WishSearch::Failed(reason),
        };
    }
    for i in done.into_iter().rev() {
        // **Dropped only because it has answered**, which is what its own slot says; a task still
        // running is never dropped from here, so no search is cancelled mid-flight.
        drop(searches.0.remove(i));
    }
}

/// **Ask GitHub's public search, without a token**, and take the first [`SEARCH_SHOWN`] issues.
///
/// Runs on a task-pool thread, so the timeout is the whole cost of a slow answer and no frame waits
/// for it. `status` is read rather than turned into an error, because GitHub says why it refused in
/// the body — a rate limit is a sentence, not a status code a person can act on.
fn ask_github(url: &str) -> Result<Vec<WishMatch>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(SEARCH_TIMEOUT_S)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent
        .get(url)
        // **GitHub's search API asks for a `User-Agent`** and answers 403 without one.
        .header("User-Agent", &format!("Cauldron/{}", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::Timeout(_) => format!("GitHub did not answer within {SEARCH_TIMEOUT_S} s"),
            other => format!("GitHub could not be reached: {other}"),
        })?;
    let status = response.status();
    let text = response.body_mut().read_to_string().map_err(|e| format!("reading GitHub's answer failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("GitHub answered {status}: {}", first_line(&text)));
    }
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("GitHub's answer is not JSON: {e}"))?;
    let items = value.get("items").and_then(|i| i.as_array()).map_or(&[][..], Vec::as_slice);
    Ok(items
        .iter()
        .filter_map(|item| {
            Some(WishMatch {
                number: item.get("number")?.as_u64()?,
                title: item.get("title")?.as_str()?.to_owned(),
                url: item.get("html_url")?.as_str()?.to_owned(),
            })
        })
        .take(SEARCH_SHOWN)
        .collect())
}

/// The first line of `text`, which is the part of GitHub's complaint a one-line row can carry.
fn first_line(text: &str) -> &str {
    text.lines().find(|line| !line.trim().is_empty()).map(str::trim).unwrap_or("no reason given")
}

// ── the status band's chip ──────────────────────────────────────────────────────────────────────

/// **The chip is spawned here, not in `chrome::spawn_status_band`**, for the reason the jobs item is:
/// everything a wish is — its row, its count, the answers — is one module's, and a band that grew a
/// row per feature would put the second such feature in a second place.
fn spawn_the_chip(mut commands: Commands, frame: Res<crate::chrome::Frame>) {
    let chip = commands
        .spawn((
            WishChip,
            // **Not a `Button`**, deliberately: the editor has twenty-four global `Activate`
            // observers, and this is a quiet status item whose one gesture is a click. `jobs.rs`
            // documents the day a band item on that bus took the application down.
            Node { display: Display::None, ..default() },
            Role::StatusItem,
            Hovered::default(),
            bevy::picking::Pickable::default(),
            ChipShown(String::new()),
            children![(Text::new(String::new()), Role::ControlText, WishChipText, bevy::picking::Pickable::IGNORE)],
        ))
        .observe(open_the_block_on_click)
        .id();
    commands.entity(frame.status).add_child(chip);
}

/// **How many rows are waiting**, in words: `2 waiting for you`.
pub fn chip_says(waiting: usize) -> String {
    match waiting {
        0 => String::new(),
        1 => "1 waiting for you".to_owned(),
        n => format!("{n} waiting for you"),
    }
}

/// **What the band says**: nothing while no row waits, nothing while the block is open (the rows are
/// on screen), and the count otherwise.
fn follow_the_chip(
    wishes: Option<Res<Wishes>>,
    blocks: Query<(), With<crate::familiar::Familiar>>,
    mut chips: Query<(&mut Node, &mut ChipShown), With<WishChip>>,
    mut texts: Query<&mut Text, With<WishChipText>>,
) {
    let Some(wishes) = wishes else { return };
    let hiding = blocks.is_empty();
    let said = if hiding { chip_says(wishes.waiting()) } else { String::new() };
    for (mut node, mut shown) in &mut chips {
        let display = if said.is_empty() { Display::None } else { Display::Flex };
        if node.display != display {
            node.display = display;
        }
        if shown.0 != said {
            shown.0.clone_from(&said);
        }
    }
    for mut text in &mut texts {
        if text.0 != said {
            text.0.clone_from(&said);
        }
    }
}

/// **A click on the chip opens the block on the row it was counting**: the View row is ticked, and the
/// first waiting row is unfolded, because that is the wish the chip was about.
fn open_the_block_on_click(
    click: On<PointerClick>,
    chips: Query<(), With<WishChip>>,
    rows: Query<Entity, With<crate::familiar::FamiliarItem>>,
    wishes: Option<Res<Wishes>>,
    mut open: Option<ResMut<WishOpenRows>>,
    mut commands: Commands,
) {
    if !chips.contains(click.entity) {
        return;
    }
    let Some(first) = wishes.as_deref().and_then(Wishes::first_waiting) else { return };
    let first = first.to_owned();
    if let Some(open) = open.as_mut() {
        open.open(&first);
    }
    // **The View row's own gesture**, so there is one way the block is opened and the tick follows
    // it: `familiar::toggle_the_familiar` is the only code that puts the block there.
    for row in &rows {
        commands.trigger(bevy_gooey::ValueChange { source: row, value: true, is_final: true });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_search_asks_for_this_repositorys_open_wishes() {
        let url = search_url("grimoire_find_piece: searches the library by label and answers the ids.");
        assert!(url.starts_with("https://api.github.com/search/issues?"), "{url}");
        assert!(url.contains("per_page=3"), "{url}");
        assert!(!url.contains(' '), "the query is not encoded: {url}");
        assert!(url.contains("grimoire_find_piece"), "the query does not name the tool: {url}");
    }

    #[test]
    fn the_chip_counts_in_words_a_person_reads() {
        assert_eq!(chip_says(0), "");
        assert_eq!(chip_says(1), "1 waiting for you");
        assert_eq!(chip_says(2), "2 waiting for you");
    }
}
