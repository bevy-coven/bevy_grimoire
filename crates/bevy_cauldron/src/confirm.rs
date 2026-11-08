//! **One question, asked the same way everywhere.**
//!
//! # Why this module exists
//!
//! The editor grew three prompts independently and each invented its own vocabulary. Measured
//! 2026-08-19, before this:
//!
//! | asked by | agree | refuse | third answer |
//! |---|---|---|---|
//! | `chooser` delete / quit | `Y` | `Esc` | — |
//! | `editor` leaving a dirty map | `S` | `Esc` | `D` discards |
//! | `labels` re-label judged pieces | `Enter` | `Esc` | — |
//!
//! Three questions, three agree keys, and `Enter` meaning "yes" in one place while another
//! deliberately refuses to answer to it. An author cannot learn that; they can only read each
//! prompt every time. Reported at the keyboard: *"our prompts keep prompting with all sorts of
//! different keys ... we need to make these uniform."*
//!
//! **`Y` proceeds and `N` stops, everywhere.** `Esc` also refuses, because backing out is what
//! `Esc` means in every other layer of this editor and a modal that ignored it would be the one
//! place it did not work — but `Esc` is a synonym for `N`, never a third outcome.
//!
//! # It is modal, and centred
//!
//! The old prompts were a line of text in the status bar at the bottom-left, which is where this
//! editor puts *commentary*. A question that blocks progress rendered in the same place, in the
//! same colour, as "baked 82 palette thumbnails" is a question an author walks past. This is a
//! gooey `dialog` over the whole window (UI-40, #234): a veil, a card in the middle, and the
//! question as the only thing to answer. It was drawn by hand on `chrome::modal_card`, the shell
//! the three modals shared; the card and the scrim are the library's now.
//!
//! # The mouse answers it too
//!
//! Both buttons are clickable. `docs/ui.md` §4.2 wants everything reachable by mouse reachable by
//! keyboard; this is the same rule read the other way, and it is the rule the chooser broke by
//! being keyboard-*only* rather than keyboard-first.
//!
//! # How a caller uses it
//!
//! Asking and answering are deliberately separate frames, and the answer is addressed:
//!
//! ```ignore
//! // somewhere a destructive verb is pressed
//! confirm.ask(Asked::DeleteEntry, format!("Delete `{name}`?"), "This cannot be undone.", "Delete", "Keep");
//!
//! // in that feature's own system, every frame
//! if let Some(true) = confirm.answer(Asked::DeleteEntry) { /* do it */ }
//! ```
//!
//! [`Asked`] is a census of every question the application can raise — the same shape
//! `keys::Action` takes, and for the same reason: a question that is not in the list cannot be
//! asked, so the audit is a `match` rather than a search.

use bevy::input_focus::AutoFocus;
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_gooey::{button, dialog, heading, label, menu, panel};

/// **Every question this application can raise.**
///
/// A census rather than a free-form string, so "which prompts exist" is answerable by reading one
/// enum, and so the answer can be routed back to exactly the feature that asked without that
/// feature and this module sharing anything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// The chooser is about to delete the highlighted kit or map.
    DeleteEntry,
    /// The application is about to quit with a map's unsaved edits (`app_verbs.rs`): save and quit, discard, or stay.
    QuitApp,
    /// The editor is leaving a map with unsaved edits.
    LeaveMap,
    /// **The editor is crossing to the other door with unsaved edits.**
    ///
    /// A question of its own rather than a reuse of [`Asked::LeaveMap`], because the two answers
    /// go to **different places**: leaving goes to the menu, and crossing goes to the other door.
    /// One question with two destinations would need the answer to know which question it was
    /// answering, and the panel does not know that.
    SwitchDoor,
    /// A labelling walk found pieces in scope that already carry labels.
    RelabelJudged,
    /// **The agent has finished a turn and is handing the result back** (S1.7).
    ///
    /// Its answers are the two the brief asks for: **Keep** (the default, and what `Esc` gives) or
    /// **Undo all** (every operation the presentation listed, newest first). The affirmative is the
    /// destructive one, so the button is red and `Enter` deliberately answers nothing — the same
    /// rule the leave question follows, because a turn's worth of changes must not be discarded by
    /// reflex.
    Presentation,
    /// **A project's `agent.ron` or `play.ron` wants to start a program no person has allowed.**
    ///
    /// The one question with three answers: **allow this command** (the exact program, arguments and
    /// directory; an edit to any of them asks again), **trust the whole project**, or **not now**. The
    /// answer is a person's alone: nothing an agent can call reaches [`Confirm::settle_choice`].
    TrustCommand,
}

impl Asked {
    /// **Does this question own the keyboard while it is up?** When it does, the census is quiet, like an
    /// open text field, so the keys that answer it are not also verbs on the tab behind it *while it is
    /// up* (the trust question's `Y`, `P` and `N` are all census keys).
    ///
    /// **Only the trust question, because it is the only one whose answer must not have a side effect
    /// on any frame it is up.** The others are left as they were: the agent's presentation lets Cmd+Z
    /// through by design (a person may take the turn back while the card is up). The *answering* press
    /// is a different matter and is every question's: [`take`] consumes it, so no verb behind a
    /// question reads the `N` that settled it (#286).
    pub fn owns_keyboard(self) -> bool {
        matches!(self, Asked::TrustCommand)
    }

    /// **Does saying yes throw work away?** The only thing the affirmative button's severity
    /// depends on, and the only place this application decides what red means (`chrome::DANGER`:
    /// *"destructive, and only destructive"*).
    ///
    /// Every question used to render its agreeing button filled `DANGER`, whichever question it
    /// was — so `Quit bevy_cauldron?`, whose own body says *"anything already saved stays
    /// saved"*, was as red as deleting a kit directory. That is how an author learns to ignore red.
    fn destructive(self) -> bool {
        match self {
            // The whole kit directory, or the map file.
            Asked::DeleteEntry | Asked::Presentation => true,
            // **Yes overwrites labels a human confirmed** with what a model proposes. Nothing is
            // deleted from disk, and judgement is work: the affirmative here is `All 732`, which
            // is the one keystroke in the labelling walk that cannot be got back.
            Asked::RelabelJudged => true,
            // Quitting keeps everything saved; leaving a map saves it on the way out. Both are
            // ordinary, and the affirmative is simply the obvious thing to do.
            //
            // **And crossing saves the map on the way out too** — the same rule, the same reason:
            // nothing is thrown away, so the affirmative is not destructive and must not look it.
            Asked::QuitApp | Asked::LeaveMap | Asked::SwitchDoor | Asked::TrustCommand => false,
        }
    }
}

/// **Which button answered.** `Alt` exists only for a question asked with [`Confirm::ask_three`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Yes,
    Alt,
    No,
}

/// **The middle answer of a three-way question**: its button's words and the key that answers it.
struct Alt {
    label: String,
    /// Answered by this key, as `Y` and `N` answer the other two. A key per question rather than one
    /// for all, because the middle answer means a different thing in each (the trust question's whole
    /// project is `P`, the leave question's discard is `D`).
    key: KeyCode,
}

/// The question on screen, if there is one.
struct Question {
    of: Asked,
    /// Which asking this is, so a dialog knows whether it shows this question or the last one.
    serial: u64,
    title: String,
    body: String,
    yes: String,
    /// The middle button of a three-way question.
    alt: Option<Alt>,
    no: String,
}

/// **The one prompt, and the one answer.**
///
/// `answer` is read by the feature that asked and cleared by the read, so an answer is delivered
/// exactly once — a question answered `yes` twice would run a destructive verb twice, which is the
/// failure the chooser's own `Pending` was built to prevent.
#[derive(Resource, Default)]
pub struct Confirm {
    question: Option<Question>,
    answer: Option<(Asked, Choice)>,
    /// Questions asked so far; the next one's `Question::serial`.
    serial: u64,
    /// **A question that owns the keyboard was answered in this frame.** It still owns it until the
    /// frame ends, so the key that answered it is not also a verb whichever of the answer and the census
    /// runs first. Cleared in `Last`.
    owner_answered: bool,
}

impl Confirm {
    /// **Raise a question.** A second `ask` while one is up is refused rather than stacked: two
    /// modals cannot both be the only thing to answer, and the one underneath would be answered by
    /// a keystroke aimed at the one on top.
    pub fn ask(
        &mut self,
        of: Asked,
        title: impl Into<String>,
        body: impl Into<String>,
        yes: impl Into<String>,
        no: impl Into<String>,
    ) {
        self.raise(of, title.into(), body.into(), yes.into(), None, no.into());
    }

    /// **Raise a question with a middle answer**, `alt_key` or its button: yes, alt or no.
    #[allow(clippy::too_many_arguments)]
    pub fn ask_three(
        &mut self,
        of: Asked,
        title: impl Into<String>,
        body: impl Into<String>,
        yes: impl Into<String>,
        alt: impl Into<String>,
        alt_key: KeyCode,
        no: impl Into<String>,
    ) {
        let alt = Alt { label: alt.into(), key: alt_key };
        self.raise(of, title.into(), body.into(), yes.into(), Some(alt), no.into());
    }

    fn raise(&mut self, of: Asked, title: String, body: String, yes: String, alt: Option<Alt>, no: String) {
        if self.question.is_some() {
            return;
        }
        self.serial += 1;
        self.question = Some(Question { of, serial: self.serial, title, body, yes, alt, no });
    }

    /// Is this question the one on screen?
    pub fn asking(&self, of: Asked) -> bool {
        self.question.as_ref().is_some_and(|q| q.of == of)
    }

    /// Is the question on screen one that owns the keyboard ([`Asked::owns_keyboard`])?
    pub fn owns_keyboard(&self) -> bool {
        self.owner_answered || self.question.as_ref().is_some_and(|q| q.of.owns_keyboard())
    }

    /// Is any question on screen? Callers gate their own keyboard on this, the way they used to
    /// gate on their own `leaving`/`ask` flag.
    pub fn is_open(&self) -> bool {
        self.question.is_some()
    }

    /// **Take this question's answer, once.** `Some(true)` agreed, `Some(false)` refused.
    pub fn answer(&mut self, of: Asked) -> Option<bool> {
        match self.answer {
            Some((asked, choice)) if asked == of => {
                self.answer = None;
                Some(choice == Choice::Yes)
            }
            _ => None,
        }
    }

    /// **Take this question's answer, once, with the middle button told apart.** For a question asked
    /// with [`Confirm::ask_three`].
    pub fn answer_choice(&mut self, of: Asked) -> Option<Choice> {
        match self.answer {
            Some((asked, choice)) if asked == of => {
                self.answer = None;
                Some(choice)
            }
            _ => None,
        }
    }

    /// Answer whatever is up. Used by the keys and the buttons, and by nothing else.
    fn settle(&mut self, yes: bool) {
        self.settle_choice(if yes { Choice::Yes } else { Choice::No });
    }

    /// The same, for any of the three. An `Alt` answer to a question with no middle button is dropped:
    /// it cannot happen from the keys or the buttons, which only offer what the question has.
    fn settle_choice(&mut self, choice: Choice) {
        if choice == Choice::Alt && self.question.as_ref().is_some_and(|q| q.alt.is_none()) {
            return;
        }
        if let Some(q) = self.question.take() {
            self.owner_answered |= q.of.owns_keyboard();
            self.answer = Some((q.of, choice));
        }
    }
}

/// The two answers, as gooey buttons in the dialog. `true` is the affirmative.
#[derive(Component, Clone, Copy)]
struct ConfirmButton(Choice);

/// **The dialog on screen, and the question it was built for.** The serial is `Question::serial`:
/// a question asked in the frame after another was answered can carry the same [`Asked`] and other
/// words, and a dialog keyed by `Asked` would go on showing the old ones.
#[derive(Component)]
struct ConfirmDialog(u64);

/// **The system that reads the answer keys.** A reader of [`Confirm::answer`] that must see the answer
/// in the frame the key was pressed orders itself `.after(ConfirmKeys)`; left unordered, the executor
/// chose, and an answer arrived a frame late on the runs that chose wrong.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConfirmKeys;

pub struct ConfirmPlugin;

impl Plugin for ConfirmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Confirm>()
            // **Before every phase of the keys** (`Sense`, `Text`, `Act`), so the frame a question is
            // answered is not also a frame in which the answer key does its ordinary job. `Y` is not
            // bound in the editor's census today, but `N` is `RotateMeshX` on the Meshes tab and
            // `RenameMap` on the Map tab (a `Phase::Text` field opens on it) — so without this
            // ordering, refusing a prompt would also turn the focused mesh or open the name field.
            // `Before(Act)` alone left the text fields unordered against the answer: a race the
            // multithreaded executor lost about one frame in three (#286).
            .add_systems(
                Update,
                (answer_by_key.in_set(ConfirmKeys), sync_dialog)
                    .chain()
                    .before(crate::keys::Phase::Sense),
            )
            .add_systems(Last, |mut confirm: ResMut<Confirm>| confirm.owner_answered = false)
            .add_observer(answer_by_activate);
    }
}

/// **`Y` proceeds, `N` stops, `Esc` is a synonym for `N`.**
///
/// Read raw rather than through `keys::just_pressed`: this is not a tab's verb and it must answer
/// identically on the menu, where the editor's key census does not exist at all. gooey's navigation
/// plugin is added (menus need it, UI-36) and sends the dialog a `Cancel` on `Esc`, which nothing here
/// observes; `Esc` is read here too, and is the answer.
///
/// **The key that answers is taken, not only read** (#286). `just_pressed` stays true for the whole
/// frame, and the editor's verbs read the same `ButtonInput` later in it: measured at the keyboard,
/// `N` to stay on a map also opened *Rename the map* (`N`), and `Esc` to refuse also reached whatever
/// `Esc` backs out of. [`take`] clears the press, and this runs before `keys::Phase::Act`, so the verbs
/// that run after see a key nobody pressed.
fn answer_by_key(mut keyboard: ResMut<ButtonInput<KeyCode>>, mut confirm: ResMut<Confirm>) {
    if confirm.question.is_none() {
        return;
    }
    let alt_key = confirm.question.as_ref().and_then(|q| q.alt.as_ref()).map(|alt| alt.key);
    if take(&mut keyboard, KeyCode::KeyY) {
        confirm.settle(true);
    } else if alt_key.is_some_and(|key| take(&mut keyboard, key)) {
        confirm.settle_choice(Choice::Alt);
    } else if take(&mut keyboard, KeyCode::KeyN) | take(&mut keyboard, KeyCode::Escape) {
        confirm.settle(false);
    }
}

/// **Was `key` pressed this frame? If so, nobody else sees it pressed.** One name for the rule every
/// modal that closes on a key follows, so "read" and "consumed" cannot drift apart.
pub fn take(keyboard: &mut ButtonInput<KeyCode>, key: KeyCode) -> bool {
    let pressed = keyboard.just_pressed(key);
    if pressed {
        keyboard.clear_just_pressed(key);
    }
    pressed
}

/// **The letter a letter key is printed as**, for the line that names the keys. A question's middle
/// answer is a letter key (`P`, `D`); anything else names itself by its `Debug` spelling, which is
/// still the key's own name and not a blank.
fn key_letter(key: KeyCode) -> String {
    let name = format!("{key:?}");
    match name.strip_prefix("Key") {
        Some(letter) => letter.to_owned(),
        None => name,
    }
}

/// The same two answers, by pointer, by Enter or Space on the focused button, or by the pad: a
/// gooey button says so with an `Activate`.
fn answer_by_activate(
    activate: On<Activate>,
    buttons: Query<&ConfirmButton>,
    mut confirm: ResMut<Confirm>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    confirm.settle_choice(button.0);
}

/// **One dialog while a question is up, and none otherwise.** It is built when the question is
/// asked, from the question's own words, and taken down when the question is gone (answered, or the
/// screen's sweep took it, in which case a question still waiting gets a new one).
///
/// # What the dialog is
///
/// gooey's [`dialog`]: the scrim, the centring, the opaque framed card and the modal tab group are
/// the library's, in the skin's colours. The card used to be `chrome::modal_card`, which the modals
/// shared so that none of them could dim, pad or stack differently; gooey's `dialog` is that one
/// contract now, and the editor's skin (`src/editor_skin.ron`) is what colours it. Only the z-order
/// is the editor's own: [`crate::chrome::MODAL_Z`], in front of every panel, which gooey's default
/// of 100 is not.
///
/// **The card carries `Hovered`**, as `modal_card`'s did, so the question "is the pointer over UI"
/// still answers yes over an open dialog: without it, scrolling over a visible question zoomed the
/// world behind it.
///
/// **The least destructive answer is focused.** A question whose yes throws work away
/// ([`Asked::destructive`]) focuses `no`; any other focuses `yes`. A destructive button names what
/// goes (`Delete map `x``, never `Yes`), because the label is all of the warning a reflex key leaves
/// time to read.
fn sync_dialog(
    mut commands: Commands,
    confirm: Res<Confirm>,
    standing: Query<(Entity, &ConfirmDialog)>,
) {
    let want = confirm.question.as_ref();
    let mut built = false;
    for (entity, dialog) in &standing {
        if want.is_some_and(|q| q.serial == dialog.0) {
            built = true;
        } else {
            commands.entity(entity).try_despawn();
        }
    }
    let Some(q) = want else {
        return;
    };
    if built {
        return;
    }
    let destructive = q.of.destructive();
    commands
        .spawn((dialog(), ConfirmDialog(q.serial)))
        .insert(GlobalZIndex(crate::chrome::MODAL_Z))
        .with_children(|scrim| {
            scrim
                .spawn((panel(), Hovered::default()))
                .with_children(|card| {
                    card.spawn(heading(q.title.clone()));
                    if !q.body.is_empty() {
                        card.spawn(label(q.body.clone()));
                    }
                    card.spawn(menu()).with_children(|answers| {
                        let mut yes = answers.spawn((button(q.yes.clone()), ConfirmButton(Choice::Yes)));
                        if destructive {
                            // **A destructive answer says so** (UI-R03, #333): the danger colour, and the safe answer is focused.
                            yes.insert(bevy_gooey::ButtonVariant::Danger);
                        } else {
                            yes.insert(AutoFocus);
                        }
                        if let Some(alt) = &q.alt {
                            answers.spawn((button(alt.label.clone()), ConfirmButton(Choice::Alt)));
                        }
                        let mut no = answers.spawn((button(q.no.clone()), ConfirmButton(Choice::No)));
                        if destructive {
                            no.insert(AutoFocus);
                        }
                    });
                    card.spawn(label(match &q.alt {
                        Some(alt) => format!("Y {}  ·  {} {}  ·  N {}", q.yes, key_letter(alt.key), alt.label, q.no),
                        None => "Y proceeds  ·  N stops".to_owned(),
                    }));
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **An answer is delivered once, and only to the question that asked.**
    #[test]
    fn an_answer_is_addressed_and_taken_once() {
        let mut c = Confirm::default();
        c.ask(Asked::DeleteEntry, "Delete?", "gone for good", "Delete", "Keep");
        assert!(c.is_open() && c.asking(Asked::DeleteEntry));

        c.settle(true);
        assert_eq!(
            c.answer(Asked::LeaveMap),
            None,
            "another feature must not be able to read an answer meant for the one that asked"
        );
        assert_eq!(c.answer(Asked::DeleteEntry), Some(true));
        assert_eq!(
            c.answer(Asked::DeleteEntry),
            None,
            "taken once — a destructive verb read twice would run twice, which is what `Pending` \
             exists to stop"
        );
        assert!(!c.is_open(), "settling takes the question down");
    }

    /// **A second question does not stack on the first.**
    #[test]
    fn one_question_at_a_time() {
        let mut c = Confirm::default();
        c.ask(Asked::DeleteEntry, "first", "", "yes", "no");
        c.ask(Asked::QuitApp, "second", "", "yes", "no");
        assert!(
            c.asking(Asked::DeleteEntry) && !c.asking(Asked::QuitApp),
            "the first question owns the screen until it is answered; two modals cannot both be \
             the only thing to answer, and the one underneath would be answered by a keystroke \
             aimed at the one on top"
        );
    }
}
