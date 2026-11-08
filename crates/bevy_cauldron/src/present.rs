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
        app.init_resource::<Presenting>().init_resource::<PresentationReply>().add_systems(
            Update,
            (answer_presentation, ask_again_when_the_block_closes).chain().in_set(crate::keys::Phase::Act),
        );
    }
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
