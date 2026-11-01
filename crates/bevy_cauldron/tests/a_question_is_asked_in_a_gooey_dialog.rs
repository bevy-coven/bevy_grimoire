//! **The editor's questions are gooey dialogs** (UI-40, #234).
//!
//! `confirm.rs` and `token_prompt.rs` each built a card by hand (`chrome::modal_card`). They are now
//! `bevy_gooey`'s own `dialog()`: one open exists while a question is up, its heading and body are the
//! question's words, its two buttons are the question's two answers, and it is gone when the question is
//! answered. The answers themselves (`Confirm::ask` and `answer`, `Y`, `N`, `Esc`) are unchanged and are
//! pinned by `one_prompt_one_vocabulary`, `headless` and the chooser's tests.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use bevy_cauldron::confirm::{Asked, Confirm};
use bevy_cauldron::token_prompt::{self, Axis, TokenPrompt};
use bevy_gooey::{Dialog, Role};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app
}

fn dialogs(app: &mut App) -> usize {
    app.world_mut().query::<&Dialog>().iter(app.world()).count()
}

/// Every `Text` in the world whose entity has `role`.
fn texts(app: &mut App, role: Role) -> Vec<String> {
    let mut q = app.world_mut().query::<(&Role, &Text)>();
    let mut out: Vec<String> = q.iter(app.world()).filter(|(r, _)| **r == role).map(|(_, t)| t.0.clone()).collect();
    out.sort();
    out
}

/// A keystroke the way a keyboard sends it: a press and a release, then a frame.
fn tap(app: &mut App, logical: bevy::input::keyboard::Key, code: KeyCode) {
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

fn tap_letter(app: &mut App, letter: &str, code: KeyCode) {
    tap(app, bevy::input::keyboard::Key::Character(letter.into()), code);
}

#[test]
fn a_question_is_one_gooey_dialog_that_says_what_each_answer_does_and_goes_when_answered() {
    let mut app = editor("question_dialog");
    assert_eq!(dialogs(&mut app), 0, "no question, no dialog");

    app.world_mut().resource_mut::<Confirm>().ask(
        Asked::DeleteEntry,
        "Delete the map `harbour`?",
        "The map file is removed. This cannot be undone.",
        "Delete map `harbour`",
        "Keep it",
    );
    app.update();
    app.update();

    assert_eq!(dialogs(&mut app), 1, "one question is one dialog");
    assert_eq!(texts(&mut app, Role::Heading), vec!["Delete the map `harbour`?".to_owned()]);
    assert!(
        texts(&mut app, Role::Label).iter().any(|t| t == "The map file is removed. This cannot be undone."),
        "the body is the question's own sentence"
    );
    assert_eq!(
        texts(&mut app, Role::ButtonText),
        vec!["Delete map `harbour`".to_owned(), "Keep it".to_owned()],
        "each button says what it does, and the destructive one names what goes"
    );

    // A second question does not stack a second dialog on the first.
    app.world_mut().resource_mut::<Confirm>().ask(Asked::QuitApp, "Quit?", "", "Quit", "Stay");
    app.update();
    assert_eq!(dialogs(&mut app), 1, "two questions cannot both be the only thing to answer");

    tap_letter(&mut app, "n", KeyCode::KeyN);
    assert_eq!(dialogs(&mut app), 0, "an answered question takes its dialog down");
    assert_eq!(app.world_mut().resource_mut::<Confirm>().answer(Asked::DeleteEntry), Some(false));
}

#[test]
fn the_next_question_is_its_own_dialog_not_the_last_ones_words() {
    let mut app = editor("question_dialog_next");
    app.world_mut().resource_mut::<Confirm>().ask(Asked::DeleteEntry, "First?", "one", "Go", "Stay");
    app.update();
    app.update();
    tap_letter(&mut app, "y", KeyCode::KeyY);
    assert_eq!(app.world_mut().resource_mut::<Confirm>().answer(Asked::DeleteEntry), Some(true));

    // Asked again, in the frame after the answer, with the same `Asked` and other words.
    app.world_mut().resource_mut::<Confirm>().ask(Asked::DeleteEntry, "Second?", "two", "Go on", "Wait");
    app.update();
    app.update();
    assert_eq!(dialogs(&mut app), 1);
    assert_eq!(texts(&mut app, Role::Heading), vec!["Second?".to_owned()], "the dialog shows the question now up");
    assert_eq!(texts(&mut app, Role::ButtonText), vec!["Go on".to_owned(), "Wait".to_owned()]);
}

#[test]
fn activating_a_dialogs_button_answers_the_question() {
    let mut app = editor("question_dialog_activate");
    for (yes, want) in [(true, "Go"), (false, "Stay")] {
        app.world_mut().resource_mut::<Confirm>().ask(Asked::DeleteEntry, "Leave?", "", "Go", "Stay");
        app.update();
        app.update();
        let button = {
            let world = app.world_mut();
            let mut q = world.query::<(Entity, &Children)>();
            let mut texts = world.query::<(&Role, &Text)>();
            let found: Vec<Entity> = q
                .iter(world)
                .filter_map(|(e, kids)| {
                    kids.iter().find_map(|k| texts.get(world, k).ok().filter(|(r, t)| **r == Role::ButtonText && t.0 == want).map(|_| e))
                })
                .collect();
            assert_eq!(found.len(), 1, "one button says `{want}`");
            found[0]
        };
        app.world_mut().commands().trigger(Activate { entity: button });
        app.update();
        assert_eq!(app.world_mut().resource_mut::<Confirm>().answer(Asked::DeleteEntry), Some(yes), "`{want}` answers {yes}");
        app.update();
        assert_eq!(dialogs(&mut app), 0);
    }
}

#[test]
fn the_vocabulary_prompt_is_a_gooey_dialog_that_shows_what_is_typed() {
    let mut app = editor("token_dialog");
    assert_eq!(dialogs(&mut app), 0);

    token_prompt::open(&mut app.world_mut().resource_mut::<TokenPrompt>(), Axis::Look);
    app.update();
    app.update();
    assert_eq!(dialogs(&mut app), 1, "an open prompt is one dialog");
    assert_eq!(texts(&mut app, Role::Heading), vec!["ADD A VOCABULARY TOKEN".to_owned()]);

    app.world_mut().resource_mut::<TokenPrompt>().open.as_mut().unwrap().name = "mossy".to_owned();
    app.update();
    app.update();
    assert!(
        texts(&mut app, Role::Label).iter().any(|t| t == "name  mossy_"),
        "the dialog shows the draft as typed: {:?}",
        texts(&mut app, Role::Label)
    );
    assert!(texts(&mut app, Role::Label).iter().any(|t| t == "axis  LOOKS   ← → cycles"));

    app.world_mut().resource_mut::<TokenPrompt>().open = None;
    app.update();
    app.update();
    assert_eq!(dialogs(&mut app), 0, "a closed prompt takes its dialog down");
}
