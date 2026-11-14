//! **An unbuilt workspace is refused with the issue that brings it** (NV-08, #385; the vision's *"no placeholder screens"*).
//!
//! Since the Project workspace landed every workspace is built, so `a_workspace_switch_keeps_the_map.rs` can no longer press a chord that is
//! refused. The mechanism is still what lets a *future* workspace's key exist before its panel does, so it is proved here on a table that
//! is not empty: `workspace::waiting_reason` is the lookup `Workspace::missing` makes over `WAITING`, and `commands::workspace_refusal` is
//! what `Command::enabled` answers with first.

use bevy_cauldron::commands::workspace_refusal;
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
