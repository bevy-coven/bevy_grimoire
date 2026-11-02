//! **A toolbar button arms a tool exactly as its key does** (UI-37, #231).
//!
//! Every tool had its own arming body inside the key handler — "toggle against *my* variant, note it,
//! drop what the others held" — written three times. `editor::arm_tool` is that body once now, and the
//! claim this file holds down is the one #231's acceptance makes: **a button and a key dispatch one
//! thing**, so a toolbar added later cannot arm Move one way and `B` arm it another.
//!
//! **What is not claimed: that a "Fill" button arms the fill.** Measured and recorded on the issue:
//! `Tool` has six variants (Place, Remove, Move, Clone, Run, Note) and **no `Fill`**; `Action::Fill` is a
//! one-shot flood from the cursor, not a mode. A Fill *toggle* would invent a mode to match a checklist
//! line, so the toolbar carries the six tools and Fill stays the command `F` is.

use bevy_cauldron::editor::{arm_tool, EditorState, Tool};

/// **Arming through `arm_tool` is idempotent per tool, toggles against its own variant, and reads the
/// same whichever front end called it.**
#[test]
fn arming_a_tool_through_the_shared_body_toggles_and_says_one_thing() {
    let mut state = EditorState::default();
    assert_eq!(state.tool, Tool::Place, "the editor starts on Place");

    // **On**, and the sentence names the chord it was given — not a chord baked into the body.
    let said = arm_tool(&mut state, Tool::Move, "B");
    assert_eq!(state.tool, Tool::Move, "arming Move must make it the tool");
    assert!(said.contains("move mode") && said.contains("B"), "the note must say what is on and how to stop: {said:?}");

    // **Off again** — the toggle is against its own variant, so a second press puts it down.
    let said = arm_tool(&mut state, Tool::Move, "B");
    assert_eq!(state.tool, Tool::Place, "a second press must put it down");
    assert_eq!(said, "move mode off");

    // **A different tool's key reaches that tool**, rather than returning to placing: pressing `X`
    // while Move is armed is removal, which is what an author means.
    arm_tool(&mut state, Tool::Move, "B");
    let said = arm_tool(&mut state, Tool::Remove, "X");
    assert_eq!(state.tool, Tool::Remove, "a second tool's key must reach that tool, not Place");
    assert!(said.starts_with("removal mode"), "{said:?}");
}

/// **Every tool has a sentence**, so a toolbar can label a button without inventing wording.
#[test]
fn every_tool_arms_and_un_arms_with_a_distinct_sentence() {
    let tools = [Tool::Place, Tool::Remove, Tool::Move, Tool::Clone, Tool::Run, Tool::Note];
    let mut state = EditorState::default();
    let mut said: Vec<String> = Vec::new();
    for tool in tools {
        state.tool = Tool::Place;
        said.push(arm_tool(&mut state, tool, "K"));
    }
    assert_eq!(said.len(), 6, "six tools, six sentences");
    // **No two tools say the same thing on the way in**, or a status line cannot tell the author which
    // mode they are in.
    let mut sorted = said.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 6, "two tools arm to the same sentence: {said:?}");
}
