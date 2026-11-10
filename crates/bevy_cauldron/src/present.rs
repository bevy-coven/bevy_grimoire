//! **The agent hands its result back, and the person answers** — the beat Mixar ends a turn with.
//!
//! A tool that only reports leaves the person to work out what changed. The brief's fourth step asks
//! for four things in one move: what the agent did, **what was specified versus inferred**, the one
//! to three places most worth checking, and a way to take the whole thing back in one action.
//!
//! # Why a question rather than a panel
//!
//! `docs/loop/borrowing-from-mixar.md`'s catalogue row for this beat maps Mixar's Keep / Undo all to
//! a yes/no the person already knows how to answer: this editor has one confirm modal, and a second
//! surface for one decision would be a second dialect. **`Y` undoes, `N` or `Esc` keeps**, which is
//! the safe default — a key pressed by reflex must not throw away a turn's work.
//!
//! # The two halves
//!
//! - `grimoire/present` (this module's [`PresentParams`]) puts the question up and returns at once,
//!   because an MCP call that blocked on a person would be a deadlock with the editor's own frame.
//! - [`answer_presentation`] runs in the editor, takes the answer, and either keeps the result or
//!   undoes every operation the presentation listed, newest first.
//!
//! **This module changes no map data.** Undoing goes through the oplog's own `Undo::Logged` path, so
//! it is the same undo `Cmd+Z` takes and it lands in the person's history.

use bevy::prelude::*;

use crate::confirm::Asked;

/// **What the person is being asked about**, for `grimoire/state` and for [`answer_presentation`].
#[derive(Resource, Default)]
pub struct Presenting(pub Option<Presented>);

/// One presentation: who it is, what it covers, and how it was answered.
#[derive(Clone, Debug, PartialEq)]
pub struct Presented {
    /// `present-1`, `present-2`, … — monotonic for the life of the process.
    pub id: String,
    /// The title the agent gave it.
    pub title: String,
    /// **The operations it covers**, in the order the agent listed them. Undo walks these newest
    /// first, because that is the order the map's state can be rewound in.
    pub operation_ids: Vec<String>,
    /// **The person's answer**: `None` until they give one, then `"kept"`, `"dismissed"`, `"undone"`, or why the
    /// undo stopped. An agent reads this from `grimoire_state`, which is the only way it learns what
    /// happened after the call returned.
    pub answer: Option<String>,
    /// What was made, in the agent's words.
    pub summary: String,
    /// What the person asked for, as the agent quotes it back.
    pub specified: Vec<String>,
    /// What the agent decided on its own.
    pub inferred: Vec<String>,
    /// The places it says are worth a look.
    pub check: Vec<String>,
    /// The question as the modal asks it, kept so it can be asked again if the AGENT block that held it closes.
    pub body: String,
    /// **Asked in the AGENT block, not in the modal.** With the block open the hand-back is answered there, where the
    /// agent's other work is (Accept, Undo, Dismiss); with it closed the modal asks as it always did.
    pub inline: bool,
}

/// **How the person answered a hand-back in the AGENT block.** Set by its buttons, taken by [`answer_presentation`].
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationReply(pub Option<Reply>);

/// The three answers the block offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    /// Keep the result.
    Accept,
    /// Take every operation it covers back, newest first.
    Undo,
    /// Close it and keep the result, saying it was not looked at.
    Dismiss,
}

/// **One thing worth a look**, as the agent states it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckItem {
    /// What to look at, in the agent's own words.
    pub what: String,
    /// Placement ids to frame in the viewport first, if any. Framing is the editor's existing
    /// `grimoire/frame_view`, not a second mechanism.
    #[serde(default)]
    pub placements: Vec<String>,
}

/// **The parameters of `grimoire/present`.**
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentParams {
    /// What the result is called. Required — a presentation without a title is a mystery.
    pub title: String,
    /// One paragraph: what was made.
    pub summary: String,
    /// **What the person asked for**, quoted back to them.
    #[serde(default)]
    pub specified: Vec<String>,
    /// **What the agent decided on its own.** The distinction the brief asks for by name, because a
    /// person who cannot see the difference cannot correct the right half of it.
    #[serde(default)]
    pub inferred: Vec<String>,
    /// Up to three places worth checking. More than three is not a short list.
    #[serde(default)]
    pub check: Vec<CheckItem>,
    /// The operations this covers, in log order. Undo all walks them newest first.
    #[serde(default)]
    pub operation_ids: Vec<String>,
}

/// **How many places to check is still a short list.** Three, and the refusal says why.
pub const MAX_CHECKS: usize = 3;

/// **The body the person reads**, built in one place so a test can assert the shape rather than a
/// string literal.
///
/// The order is the brief's own: what it is, what changed, what you asked for, what I assumed, and
/// where to look — then one line saying how to change it, because "accept or reject" is not a
/// vocabulary for a result that is nearly right.
pub fn body(params: &PresentParams, steps: &[String]) -> String {
    let mut out = String::new();
    out.push_str(&params.summary);
    out.push_str("\n\n");
    if steps.is_empty() {
        out.push_str("Steps: none recorded\n");
    } else {
        out.push_str("Steps:\n");
        for line in steps {
            out.push_str(&format!("• {line}\n"));
        }
    }
    if !params.specified.is_empty() {
        out.push_str(&format!("You said: {}\n", params.specified.join("; ")));
    }
    if !params.inferred.is_empty() {
        out.push_str(&format!("I assumed: {}\n", params.inferred.join("; ")));
    }
    if !params.check.is_empty() {
        out.push_str("Worth checking: \n");
        for (i, item) in params.check.iter().enumerate() {
            out.push_str(&format!("{}. {}\n", i + 1, item.what));
        }
    }
    out.push('\n');
    out.push_str("To change it, tell the agent.");
    out
}

/// **The editor's half of the presentation.** One plugin, one system: the answer.
pub struct PresentPlugin;

impl Plugin for PresentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Presenting>()
            .init_resource::<PresentationReply>()
            .init_resource::<HandBacks>()
            .init_resource::<Wishes>()
            .init_resource::<AgentClient>()
            .add_systems(
                Update,
                (
                    (answer_presentation, ask_again_when_the_block_closes).chain(),
                    // **After the notes have been read this frame**, so a resolve made a moment ago is
                    // already in the file `NotesView` holds and is recorded in the same frame.
                    record_handed_back_notes.after(crate::notes_view::sync_notes_view),
                )
                    .in_set(crate::keys::Phase::Act),
            );
    }
}

/// **The wishes an agent has made this session, in the order they arrived** (#353).
///
/// A wish is the one tool that would most have helped with the task just handed back. **The editor
/// holds them and the MCP server holds none**: a wish is about a hand-back, and hand-backs live
/// here, so this is the only place that can answer "has this one a wish already". Nothing in the
/// queue is written to the project — it is the session's record, not the game's.
///
/// **`Session`, not `Door`, and that is #353's own rule** (review M4). The queue is what the editor
/// has learned about its own tools in this sitting, and the hand-backs it cites can be on any door:
/// carried across, a wish made about the Kit door's work stays readable beside the Map door's. The
/// *ids* are what a door change used to restart, which is why they are numbered from
/// [`HandBacks::next`] — a count that no door change touches — rather than from the question on
/// screen.
#[derive(Resource, Default)]
pub struct Wishes {
    /// **Every wish this session made**, oldest first, with the answer the person gave it (#354).
    ///
    /// **A wish is kept after it is answered** (AW-02, #354): the person reads `opened` or
    /// `discarded` back from `grimoire_state.wishes`, and a row the person has read is still worth
    /// reading again. `waiting` is what a wish is until one of those two answers arrives.
    pub queued: Vec<QueuedWish>,
    /// The next wish's number. Monotonic for the life of the process, so ids are never reused.
    next: u32,
}

/// **What the person did with a wish** (#354).
///
/// `Opened` means the issue page was opened, **not that an issue exists**: the person submits on
/// GitHub, and the editor never posts, so it cannot know more than this.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WishState {
    /// Nobody has answered it yet. A wish waits for the person until the session ends.
    #[default]
    Waiting,
    /// The person opened its issue page.
    Opened,
    /// The person dropped it.
    Discarded,
}

impl WishState {
    /// The word `grimoire_state.wishes` reports.
    pub fn as_str(self) -> &'static str {
        match self {
            WishState::Waiting => "waiting",
            WishState::Opened => "opened",
            WishState::Discarded => "discarded",
        }
    }
}

/// **What GitHub's public search found for a wish** (#354): the issues a person should see before
/// filing a fourth copy of one, or why the search could not be asked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum WishSearch {
    /// Not asked yet: the row is folded, so no one has needed the answer.
    #[default]
    NotAsked,
    /// Asked, and the answer is not back.
    Asking,
    /// The issues that match, up to `wish_view::SEARCH_SHOWN`.
    Found(Vec<crate::wish_view::WishMatch>),
    /// GitHub did not answer, and this is why. The row says it in one line.
    Failed(String),
}

impl WishSearch {
    /// **The row's one line about the search**, or nothing while it has not been asked.
    ///
    /// A found list is links, so the line carries each issue's address: a person who wants to read
    /// one can, and the wish is still theirs to file.
    pub fn sentence(&self) -> String {
        match self {
            WishSearch::NotAsked => String::new(),
            WishSearch::Asking => "asking GitHub for wishes like this one...".to_owned(),
            WishSearch::Found(matches) if matches.is_empty() => {
                "GitHub knows no open wish like this one. Filing it adds something.".to_owned()
            }
            WishSearch::Found(matches) => {
                let mut line = format!("{} open wish(es) like this one: ", matches.len());
                for (i, m) in matches.iter().enumerate() {
                    if i > 0 {
                        line.push_str(", ");
                    }
                    line.push_str(&format!("#{} {} {}", m.number, m.title, m.url));
                }
                line
            }
            WishSearch::Failed(reason) => format!("GitHub could not be asked ({reason}); opening still works."),
        }
    }
}

/// **One wish, with the address it will be filed at.**
pub struct QueuedWish {
    pub wish: bevy_grimoire_core::wish::Wish,
    /// **What the one-wish rule compares**: the hand-back's [`HandBack::key`], which no door change
    /// can make ambiguous. `wish.after` stays exactly as the agent wrote it, so `present-3` reads
    /// back the same as it was sent.
    pub key: String,
    /// **The issue address, built when the wish was made**, so an address GitHub would not take is
    /// refused while the agent is still there to shorten what it wrote.
    pub url: String,
    /// **What the person has done with it** (#354). `Waiting` until they answer.
    pub state: WishState,
    /// **What GitHub's search said**, asked when the row is unfolded.
    pub search: WishSearch,
    /// **The last search asked, as the tool words it was asked with**: an answer to a question the
    /// row no longer asks is dropped rather than shown against a different wish.
    pub asked: String,
    /// **Why the issue page did not open**, when an open failed. The wish stays `waiting`, because a
    /// page that did not open is not an answer.
    pub note: Option<String>,
}

impl QueuedWish {
    /// The word the row shows for its kind, so a lesson of AG-06's can name its own.
    pub fn kind_label(&self) -> &'static str {
        crate::wish_view::RowKind::Wish.label()
    }
}

/// **Every hand-back this session has put in front of the person**, oldest first (#353).
///
/// A presentation is a question with an answer, and the answer is the last thing the editor keeps
/// about it — so a wish citing a hand-back that has since been answered, or superseded by the next
/// one, needs this record to be told it existed. **A wish cites a hand-back, not the current
/// question**, which is why the record is separate from [`Presenting`].
///
/// **`Session`, not `Door`** (review M4): a wish cites a hand-back that has since been answered, or
/// one made before the author walked to the other door, and a record thrown away with the door would
/// refuse both — with the very sentence that is supposed to mean "that never happened".
#[derive(Resource, Default)]
pub struct HandBacks {
    /// **Presentations and resolved notes, oldest first.**
    pub hands: Vec<HandBack>,
    /// **How many presentations this session has asked**, which is what numbers them.
    ///
    /// **Not the question on screen's number** (review M4): `Presenting` is `Door` state, so a
    /// number read off it restarts at `present-1` on the next door and two different hand-backs end
    /// up sharing an id — which the one-wish rule would then silently treat as one hand-back.
    pub asked: u32,
}

/// **Which kind of hand-back a record is** (#353's review, must-fix 1).
///
/// **A wish's `after` says its kind** — `{presentation: …}` or `{note: …}` — so a citation is
/// resolved against that kind and nothing else. They used to be interchangeable: a note's id was
/// searched among every record, so `{note: "present-1"}` found the presentation and
/// `{presentation: "n1"}` fell through to the note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandBackKind {
    /// A question `grimoire/present` asked: `present-1`, `present-2`, …
    Presentation,
    /// A note an agent resolved: `n1`, `n2`, …
    Note,
}

/// **One hand-back**, as a wish needs it: what it was called and what it covered.
#[derive(Clone, Debug, PartialEq)]
pub struct HandBack {
    pub kind: HandBackKind,
    /// **The id the agent sends**: `present-3`, or a note's `n3`.
    pub id: String,
    /// **The key no door change and no name can make ambiguous**, which the one-wish rule compares.
    ///
    /// A presentation's key is its id, because [`HandBacks::asked`] numbers them for the whole
    /// session. A note's is `<the map file's path>:<note id>` — **the path, not the map's name**
    /// (#353's review, must-fix 2): two projects both hold a map called `m`, and a key made of the
    /// name would let one project's `n1` answer the other's.
    pub key: String,
    /// **The project the hand-back was made in.** A citation from another project is refused: the
    /// operations it names are that project's log, and its map is that project's map.
    pub root: std::path::PathBuf,
    /// The title the agent gave it, which the wish carries as its context.
    pub title: String,
    /// The operations the turn listed, whose names the wish carries as its context.
    pub operation_ids: Vec<String>,
}

/// **A note's key**: the map file it stands on, and its id, which is unique on that file alone.
fn note_key(map_path: &std::path::Path, id: &str) -> String {
    format!("{}:{id}", map_path.display())
}

impl HandBacks {
    /// **Record a presentation**, numbering it for the session, and say what the agent will cite.
    pub fn ask(&mut self, root: &std::path::Path, title: String, operation_ids: Vec<String>) -> String {
        self.asked += 1;
        let id = format!("present-{}", self.asked);
        self.hands.push(HandBack {
            kind: HandBackKind::Presentation,
            // **A presentation's key is its id**, and the id is a session-wide count, so the two
            // cannot disagree.
            key: id.clone(),
            id: id.clone(),
            root: root.to_path_buf(),
            title,
            operation_ids,
        });
        id
    }

    /// **The hand-back a wish cites**, or nothing.
    ///
    /// **The kind is part of the question**, so `{note: "present-1"}` and
    /// `{presentation: "n1"}` both find nothing — a citation names one kind of hand-back and the
    /// record answers for that kind. There is **no fallback to another map or another project**: a
    /// note's key carries its map file's whole path, and every record carries the project it was
    /// made in, so a citation either names exactly one hand-back the editor watched or it names
    /// none (review of #353, must-fix 1 and 2).
    pub fn cited(
        &self,
        kind: HandBackKind,
        id: &str,
        root: &std::path::Path,
        map: Option<&std::path::Path>,
    ) -> Option<&HandBack> {
        let key = match kind {
            HandBackKind::Presentation => id.to_owned(),
            HandBackKind::Note => note_key(map?, id),
        };
        self.hands
            .iter()
            .rev()
            .find(|h| h.kind == kind && h.key == key && h.root == root)
    }

    /// **The hand-back this citation would have found in another project**, for the refusal: the
    /// id exists, and it is not this project's.
    pub fn elsewhere(&self, kind: HandBackKind, id: &str, root: &std::path::Path) -> Option<&HandBack> {
        self.hands
            .iter()
            .rev()
            .find(|h| h.kind == kind && h.id == id && h.root != root)
    }

    /// **Record a note handed back**, replacing the record this note already has.
    ///
    /// **Replaced, not added**: a note resolved, reopened, and resolved again this session is one
    /// note with one id, and `after` cannot tell the turns apart — so the record keeps the newest
    /// work, and the one-wish rule still sees one hand-back.
    pub fn record_note(
        &mut self,
        root: &std::path::Path,
        map_path: &std::path::Path,
        id: &str,
        title: String,
        operation_ids: Vec<String>,
    ) {
        let key = note_key(map_path, id);
        match self.hands.iter_mut().find(|h| h.kind == HandBackKind::Note && h.key == key) {
            Some(held) => {
                held.title = title;
                held.operation_ids = operation_ids;
            }
            None => self.hands.push(HandBack {
                kind: HandBackKind::Note,
                id: id.to_owned(),
                key,
                root: root.to_path_buf(),
                title,
                operation_ids,
            }),
        }
    }
}


impl Wishes {
    /// **The wishes as `grimoire_state` reports them**: `{id, after, state}` each, oldest first.
    ///
    /// **`state` is the person's answer** (AW-02, #354): `"waiting"` until they open the issue page or
    /// discard the wish, then `"opened"` or `"discarded"`. `opened` means the page was opened — the
    /// editor never posts, so it does not claim an issue exists.
    pub fn state_json(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.queued
                .iter()
                .map(|q| {
                    serde_json::json!({
                        "id": q.wish.id,
                        "after": q.wish.after,
                        "state": q.state.as_str(),
                    })
                })
                .collect(),
        )
    }

    /// **How many wishes are waiting for the person**: the count the status band's chip shows.
    pub fn waiting(&self) -> usize {
        self.queued.iter().filter(|q| q.state == WishState::Waiting).count()
    }

    /// **The first wish still waiting**, which is the row the chip opens the block on.
    pub fn first_waiting(&self) -> Option<&str> {
        self.queued.iter().find(|q| q.state == WishState::Waiting).map(|q| q.wish.id.as_str())
    }

    /// **Whether this hand-back has a wish already** — the rule that keeps wishes worth reading.
    ///
    /// Asked with [`HandBack::key`] rather than with the words the agent sent, so two hand-backs that
    /// a door change once made share an id are still two hand-backs. **An answered wish still counts**:
    /// the person has read that hand-back's wish, so a second one would be the same wish twice.
    pub fn has_wish_for(&self, key: &str) -> bool {
        self.queued.iter().any(|q| q.key == key)
    }

    /// **Take a wish**, numbering it and keeping it waiting behind the ones already there.
    ///
    /// The id counts up from `wish-1`; the order the queue is read in is the order wishes arrived, so
    /// several arriving at once are not lost and are read as they were written.
    pub fn take(&mut self, mut wish: bevy_grimoire_core::wish::Wish, key: String, url: String) -> String {
        self.next += 1;
        let id = format!("wish-{}", self.next);
        wish.id = id.clone();
        self.queued.push(QueuedWish {
            wish,
            key,
            url,
            state: WishState::Waiting,
            search: WishSearch::NotAsked,
            asked: String::new(),
            note: None,
        });
        id
    }
}

/// **Whether a note is handed back at all**, whichever of the two states says so.
///
/// **The state, not the time** is what decides this (review of #353, must-fix 3): a note back in the
/// queue, claimed, or rewritten is not handed back, and that is the fact that takes its id out of the
/// baseline. Whether the hand-back was *this session's* is the baseline's question, which the
/// hand-back's own time could not answer — one editor is one process, and two sittings can start in
/// the same second.
fn handed_back(note: &bevy_grimoire_core::notes::Note) -> bool {
    matches!(
        note.state,
        bevy_grimoire_core::notes::State::Resolved { .. } | bevy_grimoire_core::notes::State::Kept { .. }
    )
}

/// **The notes a map's file already held handed back** the first time this session looked at it.
///
/// **`pub`, because the system that holds it in a `Local` is**: a `Local` of a private type makes the
/// system's own signature private, which `private_interfaces` refuses.
#[derive(Default)]
pub struct HandedBackBefore {
    path: std::path::PathBuf,
    ids: std::collections::BTreeSet<String>,
}

/// **Record every note handed back in this session**, so a wish can cite one (#353, review M3).
///
/// # What makes a hand-back this session's
///
/// **The map's file, as this session first read it.** The first pass over a notes file records the
/// notes that were already handed back — they belong to an earlier sitting — and every pass after
/// that records a note the file hands back which was not in that set. An id **leaves** the set on the
/// pass that sees the note is no longer handed back (reopened, sent again, or taken back), so a note
/// resolved last week, reopened and resolved again here is this session's (review of #353,
/// must-fix 3).
///
/// **The note's own time is not the test**, and cannot be: one editor is one process, so two
/// sittings share a start to the second, while a baseline read from the file is exact however fast
/// the reopen and the second resolve follow each other.
///
/// **One reader, so one path.** The notes come from [`crate::notes_view::NotesView`], the editor's
/// own reader of the file, so an agent's `note_resolve` and a person's own are recorded by one code.
pub fn record_handed_back_notes(
    open: Option<Res<crate::project::OpenMap>>,
    project: Option<Res<crate::project::Project>>,
    view: Option<Res<crate::notes_view::NotesView>>,
    mut handbacks: Option<ResMut<HandBacks>>,
    mut seen: Local<Option<HandedBackBefore>>,
) {
    // **Every one of these is `Option`**: a build without the editor's own plugins has no notes
    // view and no open map, and in Bevy a missing `Res<T>` panics its system rather than skipping it.
    let (Some(open), Some(project), Some(view), Some(handbacks)) = (open, project, view, handbacks.as_mut())
    else {
        return;
    };
    // **Not looked at yet, so nothing is known.** A file the view has not read is not a map with no
    // notes, and a baseline taken here would be a guess.
    let path = bevy_grimoire_core::notes::path_for_map(&open.map_path);
    if view.looked_at() != Some(path.as_path()) {
        return;
    }
    // **This map's own baseline**, made on the first pass over its file: another map's set would
    // make one map's old notes this map's. The first pass only records what was already handed back.
    let fresh = seen.as_ref().is_none_or(|taken| taken.path != path);
    if fresh {
        let ids = view
            .notes()
            .notes
            .iter()
            .filter(|note| handed_back(note))
            .map(|note| note.id.clone())
            .collect();
        *seen = Some(HandedBackBefore { path: path.clone(), ids });
        return;
    }
    // **The set the arm above just guaranteed**, taken by value so the loop has no impossible arm.
    let Some(HandedBackBefore { ids: mut before, .. }) = seen.take() else {
        return;
    };
    for note in &view.notes().notes {
        if !handed_back(note) {
            // **Not handed back any more**: reopened, sent again, or taken back. Whatever it is next
            // is not an earlier session's, so it leaves the baseline.
            before.remove(&note.id);
            continue;
        }
        // **The earlier sitting's**, until this session sees the note stop being handed back — which
        // is the arm above, and is why an id leaves the set.
        if before.contains(&note.id) {
            continue;
        }
        let ops = match &note.state {
            bevy_grimoire_core::notes::State::Resolved { ops, .. }
            | bevy_grimoire_core::notes::State::Kept { ops, .. } => ops.clone(),
            // [`handed_back`] is the test above; this arm says so rather than panicking.
            _ => continue,
        };
        // **The key is the map file, not the notes file.** A wish cites the open map
        // (`OpenMap::map_path`), so a record keyed on `m.notes.yaml` could never be found by it.
        handbacks.record_note(&project.root, &open.map_path, &note.id, note.text.clone(), ops);
        // **It is this session's now**, so the baseline cannot claim it again if the clock is read
        // again at the same second.
        before.remove(&note.id);
    }
    // **And it goes back**, with whatever the passes learned.
    *seen = Some(HandedBackBefore { path, ids: before });
}

/// **Who is talking to the editor**, as the MCP client's handshake said: its name and version.
///
/// The editor records it so a wish can carry it without the agent retyping it, and so a wish reads
/// as what one client found hard rather than as an anonymous complaint.
#[derive(Resource, Default)]
pub struct AgentClient(pub Option<ClientName>);

/// One client, as it introduced itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientName {
    pub name: String,
    pub version: String,
}


/// **Take the person's answer**, and keep the result or undo every operation it listed.
///
/// **`false` (N, or Esc) keeps.** That is the safe default for a key that can arrive by reflex, and
/// it is the one answer that changes nothing — so a presentation the person never read costs them
/// nothing.
fn answer_presentation(
    mut confirm: ResMut<crate::confirm::Confirm>,
    mut reply: ResMut<PresentationReply>,
    mut presenting: ResMut<Presenting>,
    mut project: Option<ResMut<crate::project::Project>>,
    mut open: Option<ResMut<crate::project::OpenMap>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut due: Option<ResMut<crate::watch::Due>>,
) {
    // The modal's answer (Yes undoes, No keeps) or the block's button, one or the other.
    let replied = match confirm.answer(Asked::Presentation) {
        Some(true) => Some(Reply::Undo),
        Some(false) => Some(Reply::Accept),
        None => reply.0.take(),
    };
    let Some(replied) = replied else {
        return;
    };
    let Some(presented) = presenting.0.as_mut().filter(|presented| presented.answer.is_none()) else {
        return;
    };
    if replied != Reply::Undo {
        let (answer, said) = match replied {
            Reply::Dismiss => ("dismissed", "dismissed the agent's hand-back, keeping its result"),
            _ => ("kept", "kept the agent's result"),
        };
        presented.answer = Some(answer.to_owned());
        if let Some(state) = state.as_mut() {
            state.status.note(said.to_owned());
        }
        return;
    }

    // **Undo all, newest first.** The ids are in the order the agent listed them, and the map's
    // state can only be rewound from its newest change backwards — so the list is reversed, and a
    // refusal stops the walk with the id it stopped at rather than pretending the rest happened.
    let (Some(project), Some(open), Some(state)) = (project.as_mut(), open.as_mut(), state.as_mut())
    else {
        presented.answer = Some("undone — but there is no map open to undo in".to_owned());
        return;
    };
    let mut undone = 0usize;
    let mut stopped: Option<String> = None;
    for id in presented.operation_ids.iter().rev() {
        let mut ctx = crate::ops::Ctx {
            project: Some(project.clone()),
            open: Some(std::mem::replace(
                &mut **open,
                crate::project::OpenMap::new(
                    bevy_grimoire_core::map::Map::default(),
                    std::path::PathBuf::new(),
                ),
            )),
            source: crate::ops::Source::Human,
            session: crate::ops::session_id(),
            ..crate::ops::Ctx::default()
        };
        let result = crate::ops::undo(&mut ctx, Some(id));
        if let Some(fresh) = ctx.open.take() {
            **open = fresh;
        }
        crate::editor::carry_changes(project, &mut ctx);
        match result {
            Ok((_, crate::ops::Applied::Done { outcome, operation_id, .. })) => {
                undone += 1;
                state.record_logged(operation_id, outcome.summary, false);
            }
            Ok((_, crate::ops::Applied::Refused(e))) | Err(e) => {
                stopped = Some(format!("undo stopped at `{id}`: {}", e.0));
                break;
            }
        }
    }
    // **The screen is drawn again**, through the one flag that means "the map was replaced" — the
    // same path every other undo takes, so an undone turn reaches the viewport.
    if let Some(due) = due.as_mut() {
        due.map_replaced = true;
    }
    if let Some(why) = stopped {
        presented.answer = Some(why.clone());
        state.status.problem(why);
        return;
    }
    presented.answer = Some("undone".to_owned());
    state.status.note(format!("undid the agent's {undone} change(s)"));
}

/// **A hand-back asked in the AGENT block whose block was closed is asked in the modal instead**, so a question never
/// waits where nobody can see it. The words are the ones the block held.
fn ask_again_when_the_block_closes(
    mut confirm: ResMut<crate::confirm::Confirm>,
    mut presenting: ResMut<Presenting>,
    block: Query<(), With<crate::familiar::Familiar>>,
) {
    let Some(presented) = presenting.0.as_mut() else { return };
    if !presented.inline || presented.answer.is_some() || !block.is_empty() || confirm.is_open() {
        return;
    }
    confirm.ask(Asked::Presentation, presented.title.clone(), presented.body.clone(), undo_label(presented.operation_ids.len()), "Keep");
    presented.inline = false;
}

/// What the answer that takes a turn back is called, in the modal and in the block.
pub fn undo_label(operations: usize) -> String {
    format!("Undo {operations} operation{}", if operations == 1 { "" } else { "s" })
}
