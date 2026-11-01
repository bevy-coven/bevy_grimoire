//! **The vocabulary prompt** — one modal, two entry points, one code path.
//!
//! The tag block draws the project's whole vocabulary and every chip was a mouse target and nothing
//! else, so growing the vocabulary was a hand edit to `vocab.ron` — the one file in the project
//! whose comments are content. This is the editor's half of [`bevy_grimoire_core::vocab::append_token`]:
//! a centred prompt asking for a name and a note, committing through the comment-preserving splice
//! and then re-reading the file so the running editor adopts the new token.
//!
//! # Two ways in, one code path
//!
//! - **Mouse:** a `+` chip on each of the four axis rows in the tag block opens the prompt with
//!   that axis preset.
//! - **Keyboard:** `Shift+T` (`Action::NewToken`) opens it with the axis on `kind`, cycled
//!   left/right.
//!
//! Both set [`TokenPrompt::open`] and nothing else; the keys system and the paint read the same
//! resource, so the two entries cannot drift into two behaviours.
//!
//! # The typing guard is the load-bearing half
//!
//! While the prompt is open, the Meshes tab's own key handlers must not act — or typing a name
//! containing `n` would rotate the mesh. [`TokenPrompt`] is named in `editor::not_typing` and its
//! mirror `sense_context`, the one list every field in this crate is added to, so the context reads
//! `Typing` and every tab verb stands down.
//!
//! # A refusal keeps you in the field
//!
//! `Enter` on the NOTE field commits; a refusal from `append_token` or `reload_vocab` sets
//! [`Draft::problem`] and keeps the prompt open with the text intact — the chooser's rule that a
//! refusal keeps you in the field. `Esc` closes without writing.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy_gooey::{dialog, heading, label, panel};

use crate::keys;
use crate::project::Project;

/// **The four axes the tag block draws.** `capabilities`, `edge` and `slot` are drawn by no UI and
/// stay hand-authored, which is why they are not here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Kind,
    Effects,
    Look,
    Surfaces,
}

impl Axis {
    pub const ALL: [Axis; 4] = [Axis::Kind, Axis::Effects, Axis::Look, Axis::Surfaces];

    /// The heading the tag block uses, so the prompt and the block agree about what an axis is
    /// called.
    pub fn label(self) -> &'static str {
        match self {
            Axis::Kind => "KIND",
            Axis::Effects => "DOES",
            Axis::Look => "LOOKS",
            Axis::Surfaces => "OFFERS",
        }
    }

    /// The field name in `vocab.ron` — what [`bevy_grimoire_core::vocab::append_token`] splices into.
    pub fn name(self) -> &'static str {
        match self {
            Axis::Kind => "kind",
            Axis::Effects => "effects",
            Axis::Look => "look",
            Axis::Surfaces => "surfaces",
        }
    }

    pub fn next(self) -> Axis {
        match self {
            Axis::Kind => Axis::Effects,
            Axis::Effects => Axis::Look,
            Axis::Look => Axis::Surfaces,
            Axis::Surfaces => Axis::Kind,
        }
    }

    pub fn prev(self) -> Axis {
        match self {
            Axis::Kind => Axis::Surfaces,
            Axis::Effects => Axis::Kind,
            Axis::Look => Axis::Effects,
            Axis::Surfaces => Axis::Look,
        }
    }
}

/// Which of the two fields the arrows are on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    Note,
}

/// What is being typed, before it is committed.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub axis: Axis,
    pub name: String,
    pub note: String,
    pub field: Field,
    /// A refusal from the last commit attempt, shown until the next keystroke. Never a substituted
    /// value — the text stays exactly as typed.
    pub problem: Option<String>,
}

/// The prompt's state. `open: None` is the normal state; the modal is hidden and the keys system
/// drains the stream, the `xseam` guard every field in this crate carries.
#[derive(Resource, Default)]
pub struct TokenPrompt {
    pub open: Option<Draft>,
}

/// Open the prompt on `axis`. The one entry point both the `+` chip and `Shift+T` call.
pub fn open(prompt: &mut TokenPrompt, axis: Axis) {
    prompt.open = Some(Draft {
        axis,
        name: String::new(),
        note: String::new(),
        field: Field::Name,
        problem: None,
    });
}

// ---------------------------------------------------------------------------------------------
// The modal
// ---------------------------------------------------------------------------------------------

/// The dialog on screen while the prompt is open: gooey's `dialog`, built when the prompt opens and
/// taken down when it closes (UI-40, #234).
#[derive(Component)]
struct TokenPromptRoot;

/// Which line of the dialog a `Text` is, so one query can fill all of them.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Line {
    Title,
    Axis,
    Name,
    Note,
    Problem,
    Hint,
}

/// The words of one line of the dialog, for the draft as it stands.
fn line_text(line: Line, draft: &Draft) -> String {
    match line {
        Line::Title => "ADD A VOCABULARY TOKEN".to_owned(),
        Line::Axis => format!("axis  {}   ← → cycles", draft.axis.label()),
        Line::Name => {
            let caret = if draft.field == Field::Name { "_" } else { "" };
            format!("name  {}{caret}", draft.name)
        }
        Line::Note => {
            let caret = if draft.field == Field::Note { "_" } else { "" };
            format!("note  {}{caret}", draft.note)
        }
        Line::Problem => draft.problem.clone().unwrap_or_default(),
        Line::Hint => "Enter on note keeps it    Esc cancels".to_owned(),
    }
}

/// **One dialog while the prompt is open, none otherwise, and its lines are the draft.** Guarded,
/// because it runs every frame.
///
/// The card is gooey's `dialog` and its `panel`, in the skin's colours, where this prompt carried
/// its own scrim, padding and fills on `chrome::modal_card`. What stays the editor's own is what
/// made it a question rather than a floating box:
///
/// - **The scrim eats the click.** A click reaching the map underneath and placing a piece nobody
///   asked for, in the one moment the keyboard is captured, is precisely the bug the typing guard
///   (`editor::not_typing`) exists to prevent, and gooey's scrim is pickable.
/// - **The card carries `Hovered`**, so "is the pointer over UI" still answers yes over an open
///   dialog; without it, scrolling on a visible prompt zoomed the world behind it.
/// - **The z-order is [`crate::chrome::MODAL_Z`]**, in front of every panel.
///
/// The keys are not gooey's: this editor owns the arrows, `Enter` and `Esc` ([`token_keys`]), and
/// gooey's `Cancel` on `Esc` is not observed.
fn sync_token_prompt(
    mut commands: Commands,
    prompt: Res<TokenPrompt>,
    standing: Query<Entity, With<TokenPromptRoot>>,
    mut lines: Query<(&Line, &mut Text)>,
) {
    let Some(draft) = &prompt.open else {
        for entity in &standing {
            commands.entity(entity).try_despawn();
        }
        return;
    };
    if standing.is_empty() {
        commands
            .spawn((dialog(), TokenPromptRoot))
            .insert(GlobalZIndex(crate::chrome::MODAL_Z))
            .with_children(|scrim| {
                scrim.spawn((panel(), Hovered::default())).with_children(|card| {
                    card.spawn((heading(line_text(Line::Title, draft)), Line::Title));
                    for line in [Line::Axis, Line::Name, Line::Note, Line::Problem, Line::Hint] {
                        card.spawn((label(line_text(line, draft)), line));
                    }
                });
            });
        return;
    }
    for (line, mut text) in &mut lines {
        let want = line_text(*line, draft);
        if text.0 != want {
            text.0 = want;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// The keys
// ---------------------------------------------------------------------------------------------

/// **The keystrokes of the prompt.**
///
/// `Phase::Text`, and it drains the stream when the prompt is shut — the `xseam` guard every field
/// in this crate carries, so the `Shift+T` that opens the prompt cannot become its first character.
///
/// `Enter` on the NOTE field commits: `append_token`, then `Project::reload_vocab`, then close. A
/// refusal from either sets `Draft::problem` and keeps the prompt open with the text intact. `Esc`
/// closes without writing. Left/right cycle the axis; up/down move between the fields.
fn token_keys(
    mut events: MessageReader<KeyboardInput>,
    mut prompt: ResMut<TokenPrompt>,
    mut project: ResMut<Project>,
    mut state: ResMut<crate::tiles::ImportState>,
) {
    if prompt.open.is_none() {
        events.clear();
        return;
    }
    for event in events.read() {
        if !event.state.is_pressed() {
            continue;
        }
        match &event.logical_key {
            Key::Enter => {
                // **Enter on the NOTE field commits.** On the NAME field it moves down, the same
                // shape the chooser's settings use — a commit that also moved would be the same
                // key doing two jobs.
                let Some(draft) = prompt.open.clone() else {
                    return;
                };
                if draft.field != Field::Note {
                    if let Some(d) = prompt.open.as_mut() {
                        d.field = Field::Note;
                    }
                    return;
                }
                let path = project.root.join("assets/grimoire/vocab.ron");
                let name = draft.name.trim().to_owned();
                let note = draft.note.trim().to_owned();
                match bevy_grimoire_core::vocab::append_token(&path, draft.axis.name(), &name, &note) {
                    Ok(()) => match project.reload_vocab() {
                        Ok(()) => {
                            // **The status write is the repaint.** `rebuild_detail` is gated on
                            // `resource_changed::<ImportState>`, and `state.status` is a field of
                            // it — so this one line is what makes the new chip appear.
                            state.status.note(format!(
                                "added `{name}` to {}",
                                draft.axis.label()
                            ));
                            prompt.open = None;
                        }
                        Err(e) => {
                            if let Some(d) = prompt.open.as_mut() {
                                d.problem = Some(e);
                            }
                        }
                    },
                    Err(e) => {
                        if let Some(d) = prompt.open.as_mut() {
                            d.problem = Some(e);
                        }
                    }
                }
            }
            Key::Escape => {
                prompt.open = None;
            }
            Key::Backspace => {
                if let Some(d) = prompt.open.as_mut() {
                    match d.field {
                        Field::Name => {
                            d.name.pop();
                        }
                        Field::Note => {
                            d.note.pop();
                        }
                    }
                }
            }
            Key::ArrowLeft => {
                if let Some(d) = prompt.open.as_mut() {
                    d.axis = d.axis.prev();
                }
            }
            Key::ArrowRight => {
                if let Some(d) = prompt.open.as_mut() {
                    d.axis = d.axis.next();
                }
            }
            Key::ArrowUp => {
                if let Some(d) = prompt.open.as_mut() {
                    d.field = Field::Name;
                }
            }
            Key::ArrowDown => {
                if let Some(d) = prompt.open.as_mut() {
                    d.field = Field::Note;
                }
            }
            Key::Character(c) => {
                if let Some(d) = prompt.open.as_mut() {
                    match d.field {
                        Field::Name => d.name.push_str(c),
                        Field::Note => d.note.push_str(c),
                    }
                }
            }
            _ => {}
        }
    }
}

/// **The keyboard entry point** — `Shift+T` opens the prompt with the axis on `kind`.
///
/// `Phase::Act`, so it runs before the text fields; the prompt is not open yet, so the same
/// frame's keystroke is drained by `token_keys` rather than typed.
fn open_token_prompt(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    mut prompt: ResMut<TokenPrompt>,
) {
    if keys::just_pressed(&keyboard, *live, keys::Action::NewToken) {
        open(&mut prompt, Axis::Kind);
    }
}

pub struct TokenPromptPlugin;

impl Plugin for TokenPromptPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TokenPrompt>()
            .add_systems(
                Update,
                (open_token_prompt.in_set(keys::Phase::Act))
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(
                Update,
                (token_keys.in_set(keys::Phase::Text))
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            // After `Phase::Text`, so the field's keystroke is already in the draft when the dialog
            // is filled in — the same ordering `chrome::paint_name_box` states.
            .add_systems(
                Update,
                (sync_token_prompt.after(keys::Phase::Text))
                    .run_if(in_state(crate::screen::Screen::Editor)),
            );
    }
}
