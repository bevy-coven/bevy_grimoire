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
    /// **The wishes waiting to be read**, oldest first. A wish leaves the queue when it is opened or
    /// discarded, which is the review that follows this one.
    pub queued: Vec<QueuedWish>,
    /// The next wish's number. Monotonic for the life of the process, so ids are never reused.
    next: u32,
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

/// **One hand-back**, as a wish needs it: what it was called and what it covered.
#[derive(Clone, Debug, PartialEq)]
pub struct HandBack {
    /// **The id the agent sends**: `present-3`, or a note's `n3`.
    pub id: String,
    /// **The id no door change can make ambiguous**, which the one-wish rule compares.
    ///
    /// A presentation's key is its id, because [`HandBacks::asked`] numbers them for the whole
    /// session. A note's is `<map>:<note id>`, because a note id is only unique **on its own map** —
    /// so a `n3` on one door and the `n3` of a different map are two hand-backs, as they are.
    pub key: String,
    /// The title the agent gave it, which the wish carries as its context.
    pub title: String,
    /// The operations the turn listed, whose names the wish carries as its context.
    pub operation_ids: Vec<String>,
}

impl HandBacks {
    /// **Record a presentation**, numbering it for the session, and say what the agent will cite.
    pub fn ask(&mut self, title: String, operation_ids: Vec<String>) -> String {
        self.asked += 1;
        let id = format!("present-{}", self.asked);
        self.hands.push(HandBack {
            id: id.clone(),
            // **A presentation's key is its id**, and the id is a session-wide count, so the two
            // cannot disagree.
            key: id.clone(),
            title,
            operation_ids,
        });
        id
    }

    /// **The hand-back a wish cites**, found by the id the agent sent.
    ///
    /// A presentation's id is unique for the session, so the last one naming it is the one. A note's
    /// id is only unique on its own map, so `on` — the open map's name — picks between two maps that
    /// both have a note `n3`; with no map open, or two maps' worth recorded and none open to choose
    /// by, the newest is the one the agent most likely means, and it is the one whose turn just
    /// ended.
    pub fn cited(&self, id: &str, on: Option<&str>) -> Option<&HandBack> {
        self.hands
            .iter()
            .rev()
            .find(|h| h.id == id && h.key == id)
            .or_else(|| {
                let named: Vec<&HandBack> = self.hands.iter().filter(|h| h.id == id).collect();
                named
                    .iter()
                    .rev()
                    .find(|h| on.is_some_and(|map| h.key == format!("{map}:{id}")))
                    .copied()
                    .or_else(|| named.last().copied())
            })
    }
}

impl Wishes {
    /// **The wishes waiting**, as `grimoire_state` reports them: `{id, after, state}` each.
    ///
    /// `state` is `"waiting"` for every one of them, because a wish leaves this list the moment it is
    /// opened or discarded: there is one queue, and a wish is in it or it is not.
    pub fn state_json(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.queued
                .iter()
                .map(|q| {
                    serde_json::json!({
                        "id": q.wish.id,
                        "after": q.wish.after,
                        "state": "waiting",
                    })
                })
                .collect(),
        )
    }

    /// **Whether this hand-back has a wish already** — the rule that keeps wishes worth reading.
    ///
    /// Asked with [`HandBack::key`] rather than with the words the agent sent, so two hand-backs that
    /// a door change once made share an id are still two hand-backs.
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
        self.queued.push(QueuedWish { wish, key, url });
        id
    }
}

/// **A note an agent has handed back**, whichever of the two states says so.
fn handed_back(note: &bevy_grimoire_core::notes::Note) -> bool {
    matches!(
        note.state,
        bevy_grimoire_core::notes::State::Resolved { .. } | bevy_grimoire_core::notes::State::Kept { .. }
    )
}

/// The baseline [`record_handed_back_notes`] took when it first looked at a map's notes file.
///
/// **`pub`, because the system that holds it in a `Local` is**: a `Local` of a private type makes
/// the system's own signature private, which `private_interfaces` refuses.
#[derive(Default)]
pub struct NotesBaseline {
    path: std::path::PathBuf,
    /// **The notes already handed back when this map was first looked at.**
    ///
    /// They belong to an earlier session: a note resolved last week reads as `done` on disk, and it
    /// is not a hand-back anyone watched here, so a wish may not cite it.
    before: std::collections::BTreeSet<String>,
}

/// **Record every note handed back in this session**, so a wish can cite one (#353, review M3).
///
/// **`after` names a hand-back from this editor session**, and the notes file cannot say which of
/// its `done` notes this session produced — a note resolved last week looks exactly like one resolved
/// a minute ago. So the record is kept here: the state of every note the first time this map's file
/// was read is the baseline, and any note handed back after that is this session's.
///
/// **One reader, so one path.** The notes come from [`crate::notes_view::NotesView`], which is the
/// editor's own reader of the file, so an agent's `note_resolve` and a person's own are recorded by
/// the same code — and a resolve undone and redone records once, because the id is already there.
pub fn record_handed_back_notes(
    open: Option<Res<crate::project::OpenMap>>,
    view: Option<Res<crate::notes_view::NotesView>>,
    mut handbacks: Option<ResMut<HandBacks>>,
    mut baseline: Local<Option<NotesBaseline>>,
) {
    // **Every one of these is `Option`**: a build without the editor's own plugins has no notes
    // view and no open map, and in Bevy a missing `Res<T>` panics its system rather than skipping it.
    let (Some(open), Some(view), Some(handbacks)) = (open, view, handbacks.as_mut()) else {
        return;
    };
    let path = bevy_grimoire_core::notes::path_for_map(&open.map_path);
    // **Not looked at yet, so nothing is known.** Taking a baseline here would make the file's own
    // `done` notes look as though they had just been handed back.
    if view.looked_at() != Some(path.as_path()) {
        return;
    }
    if baseline.as_ref().is_none_or(|taken| taken.path != path) {
        let before = view
            .notes()
            .notes
            .iter()
            .filter(|note| handed_back(note))
            .map(|note| note.id.clone())
            .collect();
        *baseline = Some(NotesBaseline { path, before });
        return;
    }
    let before = &baseline.as_ref().expect("just established").before;
    for note in view.notes().notes.iter().filter(|note| handed_back(note)) {
        let key = format!("{}:{}", open.map.name, note.id);
        if before.contains(&note.id) || handbacks.hands.iter().any(|h| h.key == key) {
            continue;
        }
        let ops = match &note.state {
            bevy_grimoire_core::notes::State::Resolved { ops, .. }
            | bevy_grimoire_core::notes::State::Kept { ops, .. } => ops.clone(),
            // [`handed_back`] is the filter above; this arm says so rather than panicking.
            _ => continue,
        };
        handbacks.hands.push(HandBack {
            id: note.id.clone(),
            // **A note id is unique on its own map and nowhere else**, so the map is part of the key:
            // two doors' `n3` are two hand-backs, which is what the one-wish rule needs.
            key,
            title: note.text.clone(),
            operation_ids: ops,
        });
    }
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
