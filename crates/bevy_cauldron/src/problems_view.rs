//! **PROBLEMS: each broken relation is a row, with a Go to that selects the pieces it names** (UI-54, #240).
//!
//! The inspector's RELATIONS row says how many relations the open map breaks and the first one in words. This is the rest of them:
//! a section after PLAN with a row per unmet or missing relation in `<map>.relations.ron`, worded as that row words it
//! ([`relation_words`] is the one place), and a Go to button that selects the pieces the relation names, the one it is about
//! first, and frames them. With nothing broken the section says `no problems`.
//!
//! What is not a row yet: the checks of `grimoire_state.checks` and the reload verdicts. Both have their own panels; folding them in
//! here is a later slice of #240.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_grimoire_core::map::Placed;
use bevy_grimoire_core::relations::{self, Kind, Outcome, Relation, Relations};

use crate::compose::PanelInk;
use crate::editor::{EditorState, FrameTarget, TargetLock, apply_frame, aspect_of, resolve_frame};
use crate::project::{OpenMap, Project};
use crate::screen::Screen;
use crate::selection::Selection;
use crate::view::{MainCamera, Rig};

/// How often the section looks at the relations file again, in seconds.
const READ_SECS: f32 = 0.5;

/// The section's list, inside the Map door's inspector.
#[derive(Component, Clone, Copy)]
pub struct ProblemsList;

/// A row's Go to: the pieces the broken relation names, the one it is about first.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ProblemGoTo {
    pub pieces: Vec<String>,
}

pub struct ProblemsViewPlugin;

impl Plugin for ProblemsViewPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_problem_go_to).add_systems(Update, refresh_problems_list.run_if(in_state(Screen::Editor)));
    }
}

/// **A relation in words**: what it says and which way it fails. `<of> faces <to>`, with ` (hard)` for a relation that must hold
/// and ` (<by> in the way)` when the check names what blocks it; a piece the map does not have is `<id> is not on the map`.
pub fn relation_words(r: &Relation, outcome: &Outcome) -> String {
    if let Outcome::Missing { id } = outcome {
        return format!("{id} is not on the map");
    }
    let of = &r.of;
    let what = match &r.kind {
        Kind::Faces(to) => format!("{of} faces {to}"),
        Kind::Near { to, .. } => format!("{of} near {to}"),
        Kind::Apart { from, .. } => format!("{of} apart from {from}"),
        Kind::InFrontOf { of: other, .. } => format!("{of} in front of {other}"),
        Kind::Behind { of: other, .. } => format!("{of} behind {other}"),
        Kind::LeftOf { of: other, .. } => format!("{of} left of {other}"),
        Kind::RightOf { of: other, .. } => format!("{of} right of {other}"),
        Kind::ClearInFront { .. } => format!("{of} clear in front"),
        Kind::VisibleFrom { from, .. } => format!("{of} seen from {from}"),
        Kind::Along { path, .. } => format!("{of} along {}", path.join(" → ")),
        Kind::ReachableFrom { from, .. } => format!("{of} reachable from {from}"),
        Kind::AlignedWith(to) => format!("{of} aligned with {to}"),
        Kind::Between { a, b, .. } => format!("{of} between {a} and {b}"),
        Kind::MirrorOf { of: other, about, .. } => format!("{of} mirror of {other} about {about}"),
        Kind::BacksOnto { of: other, .. } => format!("{of} backs onto {other}"),
        Kind::Inside { of: other, .. } => format!("{of} inside {other}"),
        Kind::Balances { with, about, .. } => format!("{of} balances {} about {about}", with.join(", ")),
    };
    let what = if r.hard { format!("{what} (hard)") } else { what };
    match outcome {
        Outcome::Unmet { by: Some(by), .. } => format!("{what} ({by} in the way)"),
        _ => what,
    }
}

/// **The pieces a relation names that are on the map**: the one it is about first, then every placement its kind names, then the one
/// the check says is in the way. Each once, in that order; an id the map does not have is left out.
pub fn relation_pieces(r: &Relation, outcome: &Outcome, placed: &[Placed]) -> Vec<String> {
    let named: Vec<&str> = match &r.kind {
        Kind::Faces(to) | Kind::AlignedWith(to) | Kind::Near { to, .. } => vec![to.as_str()],
        Kind::Apart { from, .. } | Kind::VisibleFrom { from, .. } | Kind::ReachableFrom { from, .. } => vec![from.as_str()],
        Kind::InFrontOf { of, .. }
        | Kind::Behind { of, .. }
        | Kind::LeftOf { of, .. }
        | Kind::RightOf { of, .. }
        | Kind::BacksOnto { of, .. }
        | Kind::Inside { of, .. } => vec![of.as_str()],
        Kind::ClearInFront { .. } => Vec::new(),
        Kind::Along { path, .. } => path.iter().map(String::as_str).collect(),
        Kind::Between { a, b, .. } => vec![a.as_str(), b.as_str()],
        Kind::MirrorOf { of, about, .. } => vec![of.as_str(), about.as_str()],
        Kind::Balances { with, about, .. } => with.iter().map(String::as_str).chain(std::iter::once(about.as_str())).collect(),
    };
    let by = match outcome {
        Outcome::Unmet { by: Some(by), .. } => Some(by.as_str()),
        _ => None,
    };
    let mut pieces: Vec<String> = Vec::new();
    for id in std::iter::once(r.of.as_str()).chain(named).chain(by) {
        if placed.iter().any(|p| p.id == id) && !pieces.iter().any(|p| p == id) {
            pieces.push(id.to_owned());
        }
    }
    pieces
}

/// One drawn row: its words, whether the relation must hold, and what Go to selects.
#[derive(Clone, Debug, PartialEq)]
struct ProblemRow {
    words: String,
    hard: bool,
    pieces: Vec<String>,
}

/// What the section says for the open map: the rows, or the one line it says instead.
#[derive(Clone, Debug, PartialEq)]
enum Said {
    Rows(Vec<ProblemRow>),
    /// `no problems`, or the relations file's name when it does not read.
    Line { words: String, unreadable: bool },
}

fn said_for(open: &OpenMap) -> Said {
    let path = relations::path_for_map(&open.map_path);
    let unreadable = || {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        Said::Line { words: format!("{name} does not read"), unreadable: true }
    };
    let relations = match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Said::Line { words: "no problems".to_owned(), unreadable: false },
        Err(_) => return unreadable(),
        Ok(text) => match Relations::parse(&text) {
            Ok(r) => r,
            Err(_) => return unreadable(),
        },
    };
    let outcomes = relations::check_map(&open.map.placements, &relations);
    let rows: Vec<ProblemRow> = relations
        .relations
        .iter()
        .zip(&outcomes)
        .filter(|(_, outcome)| matches!(outcome, Outcome::Unmet { .. } | Outcome::Missing { .. }))
        .map(|(r, outcome)| ProblemRow {
            words: relation_words(r, outcome),
            hard: r.hard,
            pieces: relation_pieces(r, outcome, &open.map.placements),
        })
        .collect();
    if rows.is_empty() { Said::Line { words: "no problems".to_owned(), unreadable: false } } else { Said::Rows(rows) }
}

/// **Keep the section equal to the open map's broken relations.** The file is read twice a second, so an edit to it, an undo
/// and a moved piece all show; the rows are rebuilt only when what they say changed.
fn refresh_problems_list(
    mut commands: Commands,
    time: Res<Time>,
    open: Option<Res<OpenMap>>,
    skin: Option<Res<bevy_gooey::GooeySkin>>,
    lists: Query<Entity, With<ProblemsList>>,
    unfilled: Query<Entity, (With<ProblemsList>, Without<Children>)>,
    mut local: Local<(Option<Said>, f32)>,
) {
    let (shown, since) = &mut *local;
    *since += time.delta_secs();
    if shown.is_some() && *since < READ_SECS {
        return;
    }
    *since = 0.0;
    let Some(open) = open else { return };
    let now = said_for(&open);
    // A list with no children is one a door change just respawned: it is filled again even when nothing it says changed.
    if shown.as_ref() == Some(&now) && unfilled.is_empty() {
        return;
    }
    let ink = PanelInk::of(skin.as_deref());
    for list in &lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| match &now {
            Said::Line { words, unreadable } => {
                let colour = if *unreadable { ink.danger } else { ink.quiet };
                p.spawn((Text::new(words.clone()), TextColor(colour), crate::chrome::font(crate::chrome::text::LABEL)));
            }
            Said::Rows(rows) => {
                for row in rows {
                    p.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(crate::chrome::GAP_TIGHT), flex_shrink: 0.0, ..default() })
                        .with_children(|line| {
                            line.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, min_width: Val::Px(0.0), ..default() }).with_children(|words| {
                                let colour = if row.hard { ink.danger } else { ink.read };
                                words.spawn((Text::new(row.words.clone()), TextColor(colour), crate::chrome::font(crate::chrome::text::BODY)));
                            });
                            if !row.pieces.is_empty() {
                                crate::chrome::button(line, ProblemGoTo { pieces: row.pieces.clone() }, "Go to", crate::chrome::Severity::Plain);
                            }
                        });
                }
            }
        });
    }
    *shown = Some(now);
}

/// **Go to: select the pieces and frame them**, as a click on the outliner's rows selects (the first is the primary) and as an
/// operation button in the Notes panel frames. With no camera viewport yet the pieces are selected and the status line says the
/// view did not move.
#[allow(clippy::too_many_arguments)]
fn on_problem_go_to(
    activate: On<Activate>,
    buttons: Query<&ProblemGoTo>,
    open: Option<Res<OpenMap>>,
    project: Option<Res<Project>>,
    lock: Option<ResMut<TargetLock>>,
    selection: Option<ResMut<Selection>>,
    state: Option<ResMut<EditorState>>,
    rig: Option<ResMut<Rig>>,
    cameras: Query<&Camera, With<MainCamera>>,
) {
    let Ok(go) = buttons.get(activate.entity) else { return };
    let (Some(open), Some(mut lock), Some(mut selection), Some(mut state)) = (open, lock, selection, state) else { return };
    let ids: Vec<String> = go.pieces.iter().filter(|id| open.map.placements.iter().any(|p| &p.id == *id)).cloned().collect();
    let Some(primary) = ids.first().and_then(|id| open.map.placements.iter().find(|p| p.id == *id)) else {
        state.status.problem("none of the pieces this relation names is on the map".to_owned());
        return;
    };
    lock.hold(&primary.id.clone(), primary.at);
    selection.set(ids.clone());
    let joined = ids.join(", ");
    let (Some(project), Some(mut rig)) = (project, rig) else {
        state.status.note(format!("selected {joined}"));
        return;
    };
    let Some(aspect) = cameras.single().ok().and_then(aspect_of) else {
        state.status.note(format!("selected {joined}; the Map camera has no viewport yet, so the view did not move"));
        return;
    };
    match resolve_frame(&FrameTarget::Placements(ids), &project, &open, None) {
        Ok(frame) => {
            apply_frame(&mut rig, &frame, aspect, &open, crate::view::FRAME_MARGIN);
            state.status.note(format!("went to {joined}"));
        }
        Err(why) => state.status.problem(why),
    }
}
