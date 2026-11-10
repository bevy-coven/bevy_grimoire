use crate::fixtures;

use bevy_cauldron::terminal_agent::SELF_TEST_PROMPT;
use bevy_grimoire_core::note_agent::{AgentSettings, DEFAULT_PROMPT};

/// **The notes agent's prompt asks for a wish, after the hand-back** (#355, AW-03).
///
/// The prompt an agent started by Send receives is where it learns the workflow, and the wish is the
/// last step of it: a task that ends at `note_resolve` and says nothing about what it cost teaches
/// every agent to leave the person with the same missing tool. **Both halves are checked** — the
/// constant, and the argument a launch actually carries, so this cannot pass on a prompt the editor
/// never passes to a program.
#[test]
fn the_notes_agents_prompt_asks_for_a_wish_after_the_handback() {
    let resolve_at = DEFAULT_PROMPT.find("note_resolve").unwrap_or_else(|| {
        panic!("the notes agent's prompt no longer names `note_resolve`: {DEFAULT_PROMPT}")
    });
    let wish_at = DEFAULT_PROMPT.find("grimoire_wish").unwrap_or_else(|| {
        panic!(
            "the notes agent's prompt never names `grimoire_wish`, so an agent started by Send \
             never learns to reflect: {DEFAULT_PROMPT}"
        )
    });
    assert!(
        wish_at > resolve_at,
        "the notes agent's prompt names `grimoire_wish` before `note_resolve`, so the wish comes \
         before there is a hand-back to cite: {DEFAULT_PROMPT}"
    );

    // **The prompt a launch carries**, not the constant on its own: `argv` is what the editor hands
    // the program, and the wish is only asked for if it is in that argument.
    let root = fixtures::Fixture::new(&format!("a_wish_is_asked_for_{}", std::process::id())).build("wish");
    std::fs::write(root.join("agent.yaml"), "version: 1\ncommand: omp\n")
        .unwrap_or_else(|e| panic!("{e}"));
    let settings = AgentSettings::load(&root).unwrap_or_else(|e| panic!("{e}"));
    let argv = settings.argv(&root);
    let prompt = argv.last().unwrap_or_else(|| panic!("the launch passes no prompt: {argv:?}"));
    assert!(
        prompt.contains("grimoire_wish"),
        "the program Send starts is given a prompt that never names `grimoire_wish`: {prompt}"
    );
}

/// **The terminal's self test makes no wish** (#355).
///
/// The self test runs before any task: it calls `grimoire_self_test` once, says what it sees and
/// stops. There is no hand-back to cite, so there is nothing to wish for — and a wish from an agent
/// that has not worked anything is a row about nothing.
#[test]
fn the_self_test_prompt_makes_no_wish() {
    for (name, prompt) in [("SELF_TEST_PROMPT", SELF_TEST_PROMPT), ("DEFAULT_PROMPT", DEFAULT_PROMPT)] {
        let names_wish = prompt.contains("grimoire_wish");
        if name == "SELF_TEST_PROMPT" {
            assert!(
                !names_wish,
                "the terminal's self test never hands a task back, so it asks for no wish: {prompt}"
            );
        } else {
            assert!(names_wish, "{name} is where the wish is asked for: {prompt}");
        }
    }
}
