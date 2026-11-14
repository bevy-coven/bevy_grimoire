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
use bevy_gooey::Activate;
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
                (ask_the_unfolded, land_the_searches, follow_the_rows, follow_the_search_lines, follow_the_chip.before(bevy_gooey::GooeySystems::Style))
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
    /// A lesson: one line for the project's guide, which the person keeps or discards (#351).
    Lesson,
    /// **A proposal to a style page** an agent may not write itself, which the person keeps or discards (SG-07, #406).
    Style,
}

impl RowKind {
    /// The word the row shows for its kind.
    pub fn label(self) -> &'static str {
        match self {
            RowKind::Wish => "WISH",
            RowKind::Lesson => "LESSON",
            RowKind::Style => "STYLE",
        }
    }
}

/// **A row of the AGENT block's list, whatever kind it is**: what the rebuild despawns, so a row of
/// either kind is taken out before the list is made again.
#[derive(Component)]
pub struct RowOf;

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

// ── a lesson's row (AG-06, #351) ─────────────────────────────────────────────────────────────────

/// **A lesson row's one-line title**: whose it is and the line it would add.
#[derive(Component)]
pub struct LessonTitle;

/// One `NAME  words` line of a lesson row: `LESSON` or `WHY`.
#[derive(Component)]
pub struct LessonField {
    pub name: &'static str,
}

/// **The row's Keep button**: the person's own act, and the only thing that appends the line.
#[derive(Component)]
pub struct LessonKeep;

/// The row's Discard button.
#[derive(Component)]
pub struct LessonDiscard;

/// The row's answer, when one was already given: `kept` or `discarded`.
#[derive(Component)]
pub struct LessonAnswered(pub crate::present::LessonState);

// ── a style proposal's row (SG-07, #406) ─────────────────────────────────────────────────────────

/// **A style row's one-line title**: whose it is and the page it would change.
#[derive(Component)]
pub struct StyleTitle;

/// One `NAME  words` line of a style row: `WHY`, `PAGE`, and for each edit `FROM` and `TO`.
#[derive(Component)]
pub struct StyleField {
    pub name: &'static str,
}

/// **The row's Keep button**: the person's own act, and the only thing that writes the change.
#[derive(Component)]
pub struct StyleKeep;

/// The row's Discard button.
#[derive(Component)]
pub struct StyleDiscard;

/// The row's button that shows the page in the Design workspace, at the first line the change touches.
#[derive(Component)]
pub struct StyleShow;

/// The row's answer, when one was already given: `kept` or `discarded`.
#[derive(Component)]
pub struct StyleAnswered(pub crate::style_proposals::StyleState);

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

/// **Everything the list shows, in one value** — which is what decides whether the rows are rebuilt.
///
/// A fixed set of pre-spawned rows could not follow this: a body and its fields exist only while the
/// row is unfolded, and the two buttons leave when the person answers, so the set of nodes a row is
/// made of changes with the wish. **Both kinds are here**, because a rebuild has to know about every
/// row in the list: a wish arriving or a lesson being answered must not leave the other kind half
/// rebuilt.
#[derive(Component, Clone, PartialEq, Eq)]
struct RowsShown {
    wishes: Vec<RowShown>,
    lessons: Vec<LessonShown>,
    styles: Vec<StyleShown>,
}

/// **One wish row's own state**, as the list is rebuilt by it.
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

/// **One lesson row's own state**, as [`RowsShown`] compares it. A lesson's row is shorter than a
/// wish's — one line and its reason — so what decides the rebuild is smaller too. **Whether it is
/// unfolded is part of it**: the reason and the two answers exist only in the unfolded body, so a
/// row whose fold changed is a different subtree.
#[derive(Clone, PartialEq, Eq)]
struct LessonShown {
    id: String,
    unfolded: bool,
    state: crate::present::LessonState,
    note: Option<String>,
}

/// **One style row's own state**, as [`RowsShown`] compares it.
#[derive(Clone, PartialEq, Eq)]
struct StyleShown {
    id: String,
    unfolded: bool,
    state: crate::style_proposals::StyleState,
    note: Option<String>,
}

impl RowsShown {
    fn of(
        wishes: &Wishes,
        lessons: &crate::present::Lessons,
        styles: &crate::style_proposals::StyleProposals,
        open: &WishOpenRows,
        client: Option<&crate::present::ClientName>,
    ) -> RowsShown {
        RowsShown {
            wishes: wishes
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
            lessons: lessons
                .queued
                .iter()
                .map(|q| LessonShown {
                    id: q.lesson.id.clone(),
                    unfolded: open.is_open(&q.lesson.id),
                    state: q.state,
                    note: q.note.clone(),
                })
                .collect(),
            styles: styles
                .queued
                .iter()
                .map(|q| StyleShown { id: q.id.clone(), unfolded: open.is_open(&q.id), state: q.state, note: q.note.clone() })
                .collect(),
        }
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
#[allow(clippy::too_many_arguments)]
fn follow_the_rows(
    mut commands: Commands,
    wishes: Option<Res<Wishes>>,
    lessons: Option<Res<crate::present::Lessons>>,
    styles: Option<Res<crate::style_proposals::StyleProposals>>,
    open: Option<Res<WishOpenRows>>,
    client: Option<Res<crate::present::AgentClient>>,
    lists: Query<(Entity, Option<&RowsShown>), With<WishRows>>,
    rows: Query<Entity, With<RowOf>>,
) {
    let (Some(wishes), Some(open)) = (wishes, open) else { return };
    // **A build with no lesson queue still lists the wishes**: the queue is a resource the editor
    // adds, and a harness that did not is a build where no lesson can be proposed, not one where the
    // block is empty.
    let empty = crate::present::Lessons::default();
    let lessons = lessons.as_deref().unwrap_or(&empty);
    let client = client.as_deref().and_then(|c| c.0.as_ref());
    let no_styles = crate::style_proposals::StyleProposals::default();
    let styles = styles.as_deref().unwrap_or(&no_styles);
    let now = RowsShown::of(&wishes, lessons, styles, &open, client);
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
            for queued in &lessons.queued {
                spawn_a_lesson_row(list, queued, &open);
            }
            for queued in &styles.queued {
                spawn_a_style_row(list, queued, &open);
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
        RowOf,
        WishRow { id: id.clone(), kind: RowKind::Wish },
        WishOf(id.clone()),
        Name::new(format!("wish {}", queued.wish.id)),
        bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
    ))
    .with_children(|row| {
        // **The header is the row's own unfold**, which is what a person expects of a list row and
        // keeps the body one press away.
        row.spawn((
            WishUnfold,
            WishOf(id.clone()),
            // CHROME-OK: the row is a fold, not a chip or a list row: its own header is the whole unfold target and wears no rest/hover/selected states of its own.
            bevy::ui_widgets::Button,
            Hovered::default(),
            bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
        ))
        .observe(unfold_this_row)
        .with_children(|header| {
            crate::chrome::rigid_row(header, crate::chrome::GAP_TIGHT_STEPS, AlignItems::Center)
                .with_children(|line| {
                    line.spawn((
                        Text::new(RowKind::Wish.label()),
                        TextColor(crate::chrome::ink().dim),
                        crate::chrome::typed(crate::chrome::text::LABEL),
                    ));
                    line.spawn((
                        WishTitle,
                        WishOf(id.clone()),
                        Text::new(title_of(queued, client)),
                        TextColor(crate::chrome::ink().text),
                        crate::chrome::typed(crate::chrome::text::BODY),
                        crate::chrome::text_narrows(),
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
                    TextColor(crate::chrome::ink().dim),
                    crate::chrome::typed(crate::chrome::text::LABEL),
                ));
            }
        });
        if !unfolded {
            return;
        }
        row.spawn((
            WishBody,
            WishOf(id.clone()),
            bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
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
                TextColor(crate::chrome::ink().dim),
                crate::chrome::typed(crate::chrome::text::LABEL),
                crate::chrome::text_keeps_its_width(),
                TextLayout::new(Justify::Left, LineBreak::WordBoundary),
            ));
            if let Some(note) = &queued.note {
                body.spawn((
                    Text::new(format!("GitHub did not open: {note}")),
                    TextColor(crate::chrome::ink().danger),
                    crate::chrome::typed(crate::chrome::text::LABEL),
                    crate::chrome::text_keeps_its_width(),
                    TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                ));
            }
            if queued.state != WishState::Waiting {
                return;
            }
            crate::chrome::rigid_row(body, crate::chrome::GAP_TIGHT_STEPS, AlignItems::Stretch)
            .with_children(|answers| {
                crate::chrome::button(answers, (WishOpen, WishOf(id.clone())), "Open on GitHub", crate::chrome::Severity::Plain)
                    .observe(open_this_wish);
                crate::chrome::button(answers, (WishDiscard, WishOf(id.clone())), "Discard", crate::chrome::Severity::Plain)
                    .observe(discard_this_wish);
            });
        });
    });
}

/// **A lesson's row**: on one line, whose it is and the line it would add; unfoldable to its reason
/// and the two answers (AG-06, #351).
///
/// **It reuses the list's own shapes** — `RowOf`, `WishOf`, `WishUnfold`, `WishTitle`, `WishField`,
/// `WishBody` — so the block's fold, its row title and its fields are one implementation for both
/// kinds. Only the kind's word, the answer's own names and the two buttons are a lesson's.
fn spawn_a_lesson_row(list: &mut ChildSpawnerCommands, queued: &crate::present::QueuedLesson, open: &WishOpenRows) {
    let id = queued.lesson.id.clone();
    let answered = queued.state;
    let unfolded = open.is_open(&id);
    list.spawn((
        RowOf,
        WishRow { id: id.clone(), kind: RowKind::Lesson },
        WishOf(id.clone()),
        Name::new(format!("lesson {}", queued.lesson.id)),
        bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
    ))
    .with_children(|row| {
        row.spawn((
            WishUnfold,
            WishOf(id.clone()),
            // CHROME-OK: the row is a fold, not a chip or a list row: its own header is the whole unfold target and wears no rest/hover/selected states of its own.
            bevy::ui_widgets::Button,
            Hovered::default(),
            bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
        ))
        .observe(unfold_this_row)
        .with_children(|header| {
            crate::chrome::rigid_row(header, crate::chrome::GAP_TIGHT_STEPS, AlignItems::Center)
                .with_children(|line| {
                    line.spawn((
                        Text::new(RowKind::Lesson.label()),
                        TextColor(crate::chrome::ink().dim),
                        crate::chrome::typed(crate::chrome::text::LABEL),
                    ));
                    line.spawn((
                        LessonTitle,
                        WishOf(id.clone()),
                        Text::new(lesson_title(queued)),
                        TextColor(crate::chrome::ink().text),
                        crate::chrome::typed(crate::chrome::text::BODY),
                        crate::chrome::text_narrows(),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                    ));
                });
            if answered != crate::present::LessonState::Waiting {
                header.spawn((
                    LessonAnswered(answered),
                    WishOf(id.clone()),
                    Text::new(lesson_answer_says(answered)),
                    TextColor(crate::chrome::ink().dim),
                    crate::chrome::typed(crate::chrome::text::LABEL),
                ));
            }
        });
        // **The body is there only while the row is unfolded**, as a wish's is: the reason and the two
        // answers are one press away, and the block stays a list of one-line rows until a person asks
        // for more.
        if !unfolded {
            return;
        }
        row.spawn((
            WishBody,
            WishOf(id.clone()),
            bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
        ))
        .with_children(|body| {
            lesson_line_of(body, "WHY", &id, &queued.lesson.why);
            if let Some(note) = &queued.note {
                body.spawn((
                    Text::new(note.clone()),
                    TextColor(crate::chrome::ink().danger),
                    crate::chrome::typed(crate::chrome::text::LABEL),
                    crate::chrome::text_keeps_its_width(),
                    TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                ));
            }
            if queued.state != crate::present::LessonState::Waiting {
                return;
            }
            crate::chrome::rigid_row(body, crate::chrome::GAP_TIGHT_STEPS, AlignItems::Stretch)
            .with_children(|answers| {
                crate::chrome::button(answers, (LessonKeep, WishOf(id.clone())), "Keep", crate::chrome::Severity::Plain)
                    .observe(keep_this_lesson);
                crate::chrome::button(answers, (LessonDiscard, WishOf(id.clone())), "Discard", crate::chrome::Severity::Plain)
                    .observe(discard_this_lesson);
            });
        });
    });
}

/// **A lesson row's one-line title**: whose it is and the line it would add. The line is the whole of
/// what a person decides by, so it is the title, not the reason.
pub fn lesson_title(queued: &crate::present::QueuedLesson) -> String {
    let who = if queued.lesson.client.trim().is_empty() { "an agent".to_owned() } else { queued.lesson.client.trim().to_owned() };
    format!("{who}: {}", queued.lesson.lesson.trim())
}

/// The words a lesson's answer is written in, so the row and the file's own line agree about what the
/// person did.
pub fn lesson_answer_says(state: crate::present::LessonState) -> String {
    match state {
        crate::present::LessonState::Waiting => String::new(),
        crate::present::LessonState::Kept => "you kept it, so the next agent reads it".to_owned(),
        crate::present::LessonState::Discarded => "you discarded it".to_owned(),
    }
}

/// One `NAME  words` line of a lesson row, its own name on it so a test can ask for it.
fn lesson_line_of(parent: &mut ChildSpawnerCommands, name: &'static str, id: &str, words: &str) {
    crate::chrome::rigid_row(parent, 0.0, AlignItems::FlexStart)
        .with_children(|line| {
            crate::chrome::row_label(line, crate::editor::READOUT_LABEL_W, name);
            line.spawn(bevy_gooey::fill_column(0.0))
                .with_children(|col| {
                    col.spawn((
                        Text::new(words.to_owned()),
                        TextColor(crate::chrome::ink().text),
                        crate::chrome::typed(crate::chrome::text::BODY),
                        LessonField { name },
                        WishOf(id.to_owned()),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                    ));
                });
        });
}

/// **A style proposal's row** (SG-07, #406): on one line, whose it is and the page it would change; unfolded, why, the page and lines, each edit's span and
/// what replaces it, and the three buttons — Show the page in the Design workspace, Keep and Discard.
///
/// **It reuses the list's own shapes** (`RowOf`, `WishOf`, `WishUnfold`, `WishBody`), as the lesson's row does.
fn spawn_a_style_row(list: &mut ChildSpawnerCommands, queued: &crate::style_proposals::QueuedStyle, open: &WishOpenRows) {
    use crate::style_proposals::StyleState;
    let id = queued.id.clone();
    let answered = queued.state;
    let unfolded = open.is_open(&id);
    list.spawn((
        RowOf,
        WishRow { id: id.clone(), kind: RowKind::Style },
        WishOf(id.clone()),
        Name::new(format!("style {}", queued.id)),
        bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
    ))
    .with_children(|row| {
        row.spawn((
            WishUnfold,
            WishOf(id.clone()),
            // CHROME-OK: the row is a fold, not a chip or a list row: its own header is the whole unfold target and wears no rest/hover/selected states of its own.
            bevy::ui_widgets::Button,
            Hovered::default(),
            bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
        ))
        .observe(unfold_this_row)
        .with_children(|header| {
            crate::chrome::rigid_row(header, crate::chrome::GAP_TIGHT_STEPS, AlignItems::Center)
                .with_children(|line| {
                    line.spawn((Text::new(RowKind::Style.label()), TextColor(crate::chrome::ink().dim), crate::chrome::typed(crate::chrome::text::LABEL)));
                    crate::chrome::fill_text(line, style_title(queued), crate::chrome::ink().text, crate::chrome::text::BODY).insert((StyleTitle, WishOf(id.clone())));
                });
            if answered != StyleState::Waiting {
                header.spawn((
                    StyleAnswered(answered),
                    WishOf(id.clone()),
                    Text::new(style_answer_says(answered)),
                    TextColor(crate::chrome::ink().dim),
                    crate::chrome::typed(crate::chrome::text::LABEL),
                ));
            }
        });
        if !unfolded {
            return;
        }
        row.spawn((WishBody, WishOf(id.clone()), bevy_gooey::column_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() })))
            .with_children(|body| {
                style_line_of(body, "WHY", &id, &queued.why);
                style_line_of(body, "PAGE", &id, &style_page_says(queued));
                for edit in &queued.edits {
                    style_line_of(body, "FROM", &id, &edit.find);
                    style_line_of(body, "TO", &id, if edit.replace.is_empty() { "(taken out)" } else { &edit.replace });
                }
                if let Some(note) = &queued.note {
                    crate::chrome::fill_text(body, note.clone(), crate::chrome::ink().danger, crate::chrome::text::LABEL);
                }
                crate::chrome::rigid_row(body, crate::chrome::GAP_TIGHT_STEPS, AlignItems::Stretch)
                    .with_children(|answers| {
                        crate::chrome::button(answers, (StyleShow, WishOf(id.clone())), "Show the page", crate::chrome::Severity::Plain).observe(show_this_style_page);
                        if queued.state != StyleState::Waiting {
                            return;
                        }
                        crate::chrome::button(answers, (StyleKeep, WishOf(id.clone())), "Keep", crate::chrome::Severity::Plain).observe(keep_this_style_proposal);
                        crate::chrome::button(answers, (StyleDiscard, WishOf(id.clone())), "Discard", crate::chrome::Severity::Plain).observe(discard_this_style_proposal);
                    });
            });
    });
}

/// **A style row's one-line title**: whose it is and the page it would change.
pub fn style_title(queued: &crate::style_proposals::QueuedStyle) -> String {
    let who = if queued.client.trim().is_empty() { "an agent" } else { queued.client.trim() };
    format!("{who}: {}", queued.path)
}

/// The words a style proposal's answer is written in.
pub fn style_answer_says(state: crate::style_proposals::StyleState) -> String {
    match state {
        crate::style_proposals::StyleState::Waiting => String::new(),
        crate::style_proposals::StyleState::Kept => "you kept it, so the page says it now".to_owned(),
        crate::style_proposals::StyleState::Discarded => "you discarded it, and the page is as it was".to_owned(),
    }
}

/// **The row's `PAGE` line**: the page and the lines the change touches.
fn style_page_says(queued: &crate::style_proposals::QueuedStyle) -> String {
    match queued.lines {
        Some([first, last]) if first == last => format!("{}, line {first}", queued.path),
        Some([first, last]) => format!("{}, lines {first} to {last}", queued.path),
        None => queued.path.clone(),
    }
}

/// One `NAME  words` line of a style row, its own name on it so a test can ask for it.
fn style_line_of(parent: &mut ChildSpawnerCommands, name: &'static str, id: &str, words: &str) {
    crate::chrome::rigid_row(parent, 0.0, AlignItems::FlexStart).with_children(|line| {
        crate::chrome::row_label(line, crate::editor::READOUT_LABEL_W, name);
        line.spawn(bevy_gooey::fill_column(0.0)).with_children(|col| {
            col.spawn((
                Text::new(words.to_owned()),
                TextColor(crate::chrome::ink().text),
                crate::chrome::typed(crate::chrome::text::BODY),
                StyleField { name },
                WishOf(id.to_owned()),
                TextLayout::new(Justify::Left, LineBreak::WordBoundary),
            ));
        });
    });
}

/// One `NAME  words` line of a row, carrying the field's own name so a test can ask for it.
fn line_of(parent: &mut ChildSpawnerCommands, name: &'static str, id: &str, words: &str) {
    crate::chrome::rigid_row(parent, 0.0, AlignItems::FlexStart)
        .with_children(|line| {
            crate::chrome::row_label(line, crate::editor::READOUT_LABEL_W, name);
            line.spawn(bevy_gooey::fill_column(0.0))
                .with_children(|col| {
                    col.spawn((
                        Text::new(words.to_owned()),
                        TextColor(crate::chrome::ink().text),
                        crate::chrome::typed(crate::chrome::text::BODY),
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
    handbacks: Option<Res<crate::present::HandBacks>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut commands: Commands,
) {
    let Ok(button) = buttons.get(activate.entity) else { return };
    let id = button.0.clone();
    let Some(queued) = wishes.as_deref().and_then(|w| w.queued.iter().find(|q| q.wish.id == id)) else { return };
    if queued.state != WishState::Waiting {
        return;
    }
    // **A benchmark task drafted from this wish's hand-back is named in the issue** (BM-07, #417): the tool the wish asks for can then be
    // measured on the task that motivated it. The address is built again with the line in its context; one GitHub would not take keeps the
    // address the wish was queued with, which is the wish as the agent made it — and **the status line says the line was left out**, so
    // a person who wanted the task named in the issue knows to add it by hand.
    let mut left_out = String::new();
    let url = match handbacks.as_deref().and_then(|h| h.draft_for_key(&queued.key)) {
        Some(path) => {
            let mut wish = queued.wish.clone();
            wish.context = format!("{}\nBenchmark task: {path}", wish.context);
            wish.issue_url().unwrap_or_else(|_| {
                left_out = format!(" (without the line naming the benchmark task `{path}`: the address would be too long for GitHub; add it by hand)");
                queued.url.clone()
            })
        }
        None => queued.url.clone(),
    };
    let title = queued.wish.title();
    if let Some(state) = state.as_mut() {
        state.status.note(format!("opening {title} on GitHub{left_out}"));
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
                Ok(()) => state.status.note(format!("opened {title} on GitHub{left_out}")),
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

// ── a lesson's two answers (AG-06, #351) ────────────────────────────────────────────────────────

/// **Keep**: the person's own act, and the only thing that appends a lesson to the guide.
///
/// **The editor writes the file, once, here.** The line is built by
/// [`bevy_grimoire_core::lesson::Lesson::line`] and put under `LESSONS_HEADING` by
/// [`bevy_grimoire_core::lesson::append`], so both are the core's rules and this is only the write.
///
/// **The grant follows what was already allowed.** If the guide was allowed before the append — by
/// its exact content or by project trust — the new content is allowed too, because the person chose
/// the one line that changed. **If it was withheld, it stays withheld**: keeping one line must not
/// allow text the person never reviewed, which is the rule AG-03 exists for.
///
/// **A failed write is not an answer.** The lesson stays waiting and the row says why, because a
/// lesson marked kept whose line is not in the file is a claim about a file that is wrong.
fn keep_this_lesson(
    activate: On<Activate>,
    buttons: Query<&WishOf, With<LessonKeep>>,
    mut lessons: Option<ResMut<crate::present::Lessons>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    store_root: Option<Res<crate::trusted::StoreRoot>>,
) {
    let Ok(button) = buttons.get(activate.entity) else { return };
    let id = button.0.clone();
    let Some(lessons) = lessons.as_mut() else { return };
    let Some(queued) = lessons.queued.iter_mut().find(|q| q.lesson.id == id) else { return };
    if queued.state != crate::present::LessonState::Waiting {
        return;
    }
    let root = queued.root.clone();
    let guide = crate::agent_guide::project_file(&root);
    let store = state.as_deref().and_then(|s| s.trust.file.clone());
    let root_res = store_root.as_deref();
    // **What the guide allowed before the append decides whether the new text is allowed**: read the
    // file as it is now, and the store's answer for that content.
    let before = match crate::project_page::guide_on_disk(&guide) {
        Ok(text) => text,
        Err(e) => {
            queued.note = Some(format!("{} could not be read: {e}. Nothing was written.", guide.display()));
            if let Some(state) = state.as_mut() {
                state.status.problem(queued.note.clone().unwrap_or_default());
            }
            return;
        }
    };
    let was_allowed = crate::project_page::guide_is_allowed(root_res, store.as_deref(), &root, &before);
    let line = queued.lesson.line(&crate::labels::date_today());
    let after = bevy_grimoire_core::lesson::append(&before, &line);
    match bevy_grimoire_core::atomic_file::write(&guide, &after) {
        Ok(()) => {
            queued.state = crate::present::LessonState::Kept;
            queued.note = None;
            if let Some(state) = state.as_mut() {
                state.status.note(if was_allowed {
                    "kept the lesson, and allowed the guide it is in"
                } else {
                    "kept the lesson; the guide is still waiting to be allowed"
                });
            }
            // **Only content that was already allowed is granted.** A withheld guide the person has
            // not read stays withheld however many lines they keep.
            if was_allowed
                && let Err(e) = crate::project_page::grant_in_store(root_res, store.as_deref(), &root, &after)
                && let Some(state) = state.as_mut()
            {
                state.status.problem(format!("the lesson was kept, but the guide was not allowed: {e}"));
            }
        }
        Err(e) => {
            queued.note = Some(format!("the lesson was not written: {e}"));
            if let Some(state) = state.as_mut() {
                state.status.problem(queued.note.clone().unwrap_or_default());
            }
        }
    }
}

/// **Discard**: the lesson is dropped, and the file is not touched.
fn discard_this_lesson(
    activate: On<Activate>,
    buttons: Query<&WishOf, With<LessonDiscard>>,
    mut lessons: Option<ResMut<crate::present::Lessons>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Ok(button) = buttons.get(activate.entity) else { return };
    let Some(lessons) = lessons.as_mut() else { return };
    let id = button.0.clone();
    let Some(queued) = lessons.queued.iter_mut().find(|q| q.lesson.id == id) else { return };
    if queued.state != crate::present::LessonState::Waiting {
        return;
    }
    queued.state = crate::present::LessonState::Discarded;
    let title = queued.lesson.lesson.clone();
    if let Some(state) = state.as_mut() {
        state.status.note(format!("discarded `{title}`"));
    }
}

// ── a style proposal's three answers (SG-07, #406) ──────────────────────────────────────────────

/// **Show the page**: the Design workspace on the proposal's page, at the first line the change touches.
fn show_this_style_page(
    activate: On<Activate>,
    buttons: Query<&WishOf, With<StyleShow>>,
    proposals: Option<Res<crate::style_proposals::StyleProposals>>,
    mut commands: Commands,
) {
    let Ok(button) = buttons.get(activate.entity) else { return };
    let Some(queued) = proposals.as_deref().and_then(|p| p.queued.iter().find(|q| q.id == button.0)) else { return };
    let path = queued.path.clone();
    let line = queued.lines.map(|[first, _]| first as usize);
    commands.queue(move |world: &mut World| crate::design_view::show_file(world, &path, line));
}

/// **Keep**: the person's own act, and the only thing that writes a proposed change.
///
/// **It dispatches `style_edit` as the person's** ([`crate::style_proposals::keep_this_style_change`]), so the change is one log line and one undo step and the new text is allowed.
/// A page that changed since the proposal was made, or any other refusal, is the row's note and the proposal stays waiting: a change that was not written is not an answer.
fn keep_this_style_proposal(
    activate: On<Activate>,
    buttons: Query<&WishOf, With<StyleKeep>>,
    mut proposals: Option<ResMut<crate::style_proposals::StyleProposals>>,
    project: Option<Res<crate::project::Project>>,
    store_root: Option<Res<crate::trusted::StoreRoot>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Ok(button) = buttons.get(activate.entity) else { return };
    let id = button.0.clone();
    let (Some(proposals), Some(project)) = (proposals.as_mut(), project.as_deref()) else { return };
    let Some(queued) = proposals.queued.iter_mut().find(|q| q.id == id) else { return };
    if queued.state != crate::style_proposals::StyleState::Waiting {
        return;
    }
    let trust_file = crate::trusted::resolve(store_root.as_deref(), state.as_deref().and_then(|s| s.trust.file.as_deref())).ok();
    match crate::style_proposals::keep_this_style_change(queued, project, trust_file) {
        Ok((line, summary)) => {
            queued.state = crate::style_proposals::StyleState::Kept;
            queued.note = None;
            if let Some(state) = state.as_mut() {
                state.record_logged(line, summary.clone(), false);
                state.status.note(format!("kept the change: {summary}"));
            }
        }
        Err(why) => {
            queued.note = Some(why.clone());
            if let Some(state) = state.as_mut() {
                state.status.problem(why);
            }
        }
    }
}

/// **Discard**: the proposal is dropped, and the page is not touched.
fn discard_this_style_proposal(
    activate: On<Activate>,
    buttons: Query<&WishOf, With<StyleDiscard>>,
    mut proposals: Option<ResMut<crate::style_proposals::StyleProposals>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Ok(button) = buttons.get(activate.entity) else { return };
    let Some(proposals) = proposals.as_mut() else { return };
    let Some(queued) = proposals.queued.iter_mut().find(|q| q.id == button.0) else { return };
    if queued.state != crate::style_proposals::StyleState::Waiting {
        return;
    }
    queued.state = crate::style_proposals::StyleState::Discarded;
    if let Some(state) = state.as_mut() {
        state.status.note(format!("discarded the change to {}", queued.path));
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
            // **Not a `Button`**, deliberately: the editor has twenty-four global `Activate` observers, and this is a quiet status item whose one gesture is a click.
            // `jobs.rs` documents the day a band item on that bus took the application down; gooey's `status_item_quiet` stays off it and `Shown` hides it until a row waits.
            bevy_gooey::status_item_quiet(String::new()),
            bevy_gooey::Shown(false),
            bevy::picking::Pickable::default(),
            ChipShown(String::new()),
        ))
        .observe(open_the_block_on_click)
        .id();
    crate::chrome::the_words_of_a_quiet_item_are_a_control(&mut commands, chip, WishChipText);
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
    lessons: Option<Res<crate::present::Lessons>>,
    styles: Option<Res<crate::style_proposals::StyleProposals>>,
    blocks: Query<(), With<crate::familiar::Familiar>>,
    mut chips: Query<(&mut bevy_gooey::Shown, &mut ChipShown), With<WishChip>>,
    mut texts: Query<&mut Text, With<WishChipText>>,
) {
    let Some(wishes) = wishes else { return };
    let hiding = blocks.is_empty();
    // **Wishes and lessons are one count**: both are rows in the block, and both wait for the person.
    let waiting = crate::activity_bar::waiting_on_you(Some(&wishes), lessons.as_deref(), styles.as_deref());
    let said = if hiding { chip_says(waiting) } else { String::new() };
    for (mut visible, mut shown) in &mut chips {
        let on = !said.is_empty();
        if visible.0 != on {
            visible.0 = on;
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
#[allow(clippy::too_many_arguments)]
fn open_the_block_on_click(
    click: On<PointerClick>,
    chips: Query<(), With<WishChip>>,
    rows: Query<Entity, With<crate::familiar::FamiliarItem>>,
    wishes: Option<Res<Wishes>>,
    lessons: Option<Res<crate::present::Lessons>>,
    styles: Option<Res<crate::style_proposals::StyleProposals>>,
    mut open: Option<ResMut<WishOpenRows>>,
    mut commands: Commands,
) {
    if !chips.contains(click.entity) {
        return;
    }
    let first = wishes
        .as_deref()
        .and_then(Wishes::first_waiting)
        .or_else(|| lessons.as_deref().and_then(crate::present::Lessons::first_waiting))
        .or_else(|| styles.as_deref().and_then(crate::style_proposals::StyleProposals::first_waiting));
    let Some(first) = first else { return };
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
