//! **An unbuilt workspace is refused with the issue that brings it** (NV-08, #385; the vision's *"no placeholder screens"*).
//!
//! Since the Project workspace landed every workspace is built, so `a_workspace_switch_keeps_the_map.rs` can no longer press a chord that is
//! refused. The mechanism is still what lets a *future* workspace's key exist before its panel does, so it is proved here on a table that
//! is not empty: `workspace::waiting_reason` is the lookup `Workspace::missing` makes over `WAITING`, and `commands::workspace_refusal` is
//! what `Command::enabled` answers with first. The three places that consult the table (the command, the chord's `is_offered`, the activity bar) each
//! have a seam that takes one (`enabled_in`, `is_offered_in`, `registered_in`), so the wiring is pinned too: deleting any of the three consults fails a test here.

use bevy_cauldron::activity_bar;
use bevy_cauldron::commands::{self, workspace_refusal};
use bevy_cauldron::keys::{Action, Live};
use bevy_cauldron::workspace::{WAITING, Workspace, waiting_reason};

/// **A table that lists a workspace refuses it with the sentence it was given**, and one that does not list it says nothing.
#[test]
fn a_listed_workspace_is_waiting_with_the_issue_that_brings_it() {
    let table = [(Workspace::Game, "the Game workspace arrives with NV-09 (#386)"), (Workspace::Agent, "the Agent workspace arrives with NV-10 (#387)")];
    assert_eq!(waiting_reason(&table, Workspace::Game), Some("the Game workspace arrives with NV-09 (#386)"));
    assert_eq!(waiting_reason(&table, Workspace::Agent), Some("the Agent workspace arrives with NV-10 (#387)"));
    for built in [Workspace::Project, Workspace::Map, Workspace::World, Workspace::Kit, Workspace::Animation, Workspace::Sound, Workspace::Files] {
        assert_eq!(waiting_reason(&table, built), None, "{} is not in the table and is not waiting", built.title());
    }
    assert_eq!(waiting_reason(&[], Workspace::Game), None, "an empty table waits for nothing");
}

/// **The refusal a chord answers with is that sentence, and it names an issue**, so a person pressing it learns where it comes from.
#[test]
fn the_chord_of_a_waiting_workspace_answers_with_its_sentence() {
    let sentence = waiting_reason(&[(Workspace::Game, "the Game workspace arrives with NV-09 (#386)")], Workspace::Game);
    let refused = workspace_refusal(sentence).expect_err("a waiting workspace is refused");
    assert!(refused.contains('#'), "the refusal names no issue, so a person pressing the chord learns nothing: {refused}");
    assert_eq!(workspace_refusal(None), Ok(()), "a built workspace is not refused");
}

/// **The real table agrees with the rest of the registry**: a workspace is built exactly when `WAITING` does not list it, and every
/// sentence in it names an issue. (Empty today; this holds the day a workspace is added before its panel.)
#[test]
fn the_real_table_and_the_registry_agree() {
    for workspace in Workspace::ALL {
        let listed = WAITING.iter().any(|(w, _)| *w == workspace);
        assert_eq!(workspace.is_built(), !listed, "{} is built or waiting, not both or neither", workspace.title());
        assert_eq!(workspace.missing().is_some(), listed);
    }
    for (workspace, why) in WAITING {
        assert!(why.contains('#') || why.contains("GD-"), "the {} workspace is waiting and names no issue: {why}", workspace.title());
    }
}

/// The one table the wiring tests use: the Game workspace waits.
const GAME_WAITS: [(Workspace, &str); 1] = [(Workspace::Game, "the Game workspace arrives with NV-09 (#386)")];

/// **The command of a waiting workspace is refused with its sentence, and the others are not** (`Command::enabled_in`).
#[test]
fn the_command_of_a_waiting_workspace_is_refused_with_the_issue() {
    let game = commands::by_id("workspace.game").expect("workspace.game is a command");
    assert_eq!(game.enabled_in(&GAME_WAITS, Live::default()), Err("the Game workspace arrives with NV-09 (#386)"), "the waiting workspace's command is not refused with its reason");
    let map = commands::by_id("workspace.map").expect("workspace.map is a command");
    assert_eq!(map.enabled_in(&GAME_WAITS, Live::default()), Ok(()), "a workspace that is not in the table is refused");
    assert_eq!(game.enabled_in(&[], Live::default()), Ok(()), "with nothing waiting the same command is enabled");
}

/// **The chord of a waiting workspace is not offered** (`Action::is_offered_in`): the overlay draws no key the editor refuses.
#[test]
fn the_chord_of_a_waiting_workspace_is_not_offered() {
    assert!(!Action::WorkspaceGame.is_offered_in(&GAME_WAITS), "a waiting workspace's chord is offered");
    assert!(Action::WorkspaceMap.is_offered_in(&GAME_WAITS), "a built workspace's chord is not offered");
    assert!(Action::WorkspaceGame.is_offered_in(&[]), "with nothing waiting the chord is offered");
    assert!(Action::Undo.is_offered_in(&GAME_WAITS), "a verb that goes to no workspace is always offered");
}

/// **The activity bar leaves a waiting workspace out** (`activity_bar::registered_in`), and keeps the others in the order the keys follow.
#[test]
fn the_activity_bar_omits_a_waiting_workspace() {
    let shown = activity_bar::registered_in(&GAME_WAITS);
    assert!(!shown.contains(&Workspace::Game), "the bar draws a tab for a workspace that is waiting");
    let want: Vec<Workspace> = Workspace::ALL.into_iter().filter(|w| *w != Workspace::Game).collect();
    assert_eq!(shown, want, "the bar lists every other workspace, in registry order");
    assert_eq!(activity_bar::registered_in(&[]), Workspace::ALL.to_vec(), "with nothing waiting it lists all of them");
}
