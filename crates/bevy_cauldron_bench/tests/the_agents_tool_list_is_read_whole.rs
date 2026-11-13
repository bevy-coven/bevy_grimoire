//! **What an agent was offered is read whole, in one spelling, and an unknowable list is an error**
//! (BM-02, #412).
//!
//! `omp` spells the editor's tools `mcp__<server>_grimoire_frame_view`; the scripted agents' MCP server says
//! `grimoire_frame_view`. A `hide_tools` arm is judged by whether the second name is in the list, so a list
//! left in the first spelling answers "absent" with no `--hide-tools` at all  -  and an omp trial whose list
//! came back empty would record `offered 0` and pass the same arm. These tests run
//! [`parse_tools_offered`] on the registry shape measured from a real omp 18.18.9 trial (2026-11-13: twenty
//! built-ins and the editor's twenty-six tools), with no editor and no model, so the gate holds them.

use bevy_cauldron_bench::run::{parse_tools_offered, BUILTIN_TOOLS};

/// The MCP server's name in the config the editor hands out.
const SERVER: &str = "cauldron";

/// **The editor's tools as the omp registry names them**, a subset large enough to show the spelling.
const EDITOR_TOOLS: &[&str] = &[
    "grimoire_apply_operation",
    "grimoire_frame_view",
    "grimoire_guide",
    "grimoire_present",
    "grimoire_state",
];

/// One sidecar line, as the trial's extension writes it.
fn line(tools: &[String]) -> String {
    format!("{}\n", serde_json::json!({"why": "before_agent_start", "tools": tools}))
}

/// **The registry of a trial**: the harness's built-ins and the editor's tools in omp's spelling, less `hide`.
fn registry(hide: &[&str]) -> String {
    let mut tools: Vec<String> = BUILTIN_TOOLS.iter().map(|t| (*t).to_owned()).collect();
    tools.extend(
        EDITOR_TOOLS
            .iter()
            .filter(|t| !hide.contains(t))
            .map(|t| format!("mcp__{SERVER}_{t}")),
    );
    line(&tools)
}

/// **A bare trial HAS `grimoire_frame_view` and the arm LACKS it, in the same spelling**  -  the property a
/// `hide_tools` arm is judged by. Before the names were normalised the bare list held only
/// `mcp__cauldron_grimoire_frame_view`, so a check for `grimoire_frame_view` was vacuously absent in both.
#[test]
fn a_bare_omp_list_has_the_tool_and_the_arm_lacks_it_in_one_spelling() {
    let bare = parse_tools_offered(&registry(&[]), SERVER).unwrap_or_else(|e| panic!("{e}"));
    let arm = parse_tools_offered(&registry(&["grimoire_frame_view"]), SERVER).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        bare.iter().any(|t| t == "grimoire_frame_view"),
        "the bare list holds `grimoire_frame_view` as the server spells it: {bare:?}"
    );
    assert!(
        !arm.iter().any(|t| t == "grimoire_frame_view" || t.contains("frame_view")),
        "the arm's list lacks it in either spelling: {arm:?}"
    );
    assert_eq!(
        bare.iter().filter(|t| t.as_str() != "grimoire_frame_view").collect::<Vec<_>>(),
        arm.iter().collect::<Vec<_>>(),
        "the arm is the bare list less exactly that name"
    );
    assert!(
        bare.iter().all(|t| !t.starts_with("mcp__")),
        "no name keeps omp's server prefix: {bare:?}"
    );
}

/// **A list that was never recorded, or recorded empty, is an error and not `offered: []`.**
#[test]
fn an_unknowable_tool_list_is_an_error() {
    let missing = parse_tools_offered("", SERVER).expect_err("no line is no list");
    assert!(missing.contains("recorded no tool list"), "says why: {missing}");

    let empty = parse_tools_offered(&line(&[]), SERVER).expect_err("an empty registry is not a list");
    assert!(empty.contains("none of the editor's tools"), "says the server never connected: {empty}");

    let threw = parse_tools_offered("{\"why\":\"before_agent_start\",\"error\":\"boom\"}\n", SERVER)
        .expect_err("a registry that threw is not a list");
    assert!(threw.contains("boom"), "carries the harness's own error: {threw}");

    let garbled = parse_tools_offered("not json\n", SERVER).expect_err("a garbled line is not a list");
    assert!(garbled.contains("not JSON"), "says it could not be read: {garbled}");
}

/// **An agent offered only the harness's built-ins never reached the editor's server.**
#[test]
fn a_list_with_no_editor_tool_means_the_server_never_connected() {
    let only: Vec<String> = BUILTIN_TOOLS.iter().map(|t| (*t).to_owned()).collect();
    let err = parse_tools_offered(&line(&only), SERVER).expect_err("no editor tool is a harness error");
    assert!(err.contains(&format!("mcp__{SERVER}_")), "names the prefix it looked for: {err}");
}

/// **A tool from outside the Dark Factory's set is a harness error naming it**  -  the developer's own
/// MCP server that loaded into every trial before the isolated `HOME`.
#[test]
fn a_tool_from_outside_the_set_is_a_harness_error() {
    let mut tools: Vec<String> = BUILTIN_TOOLS.iter().map(|t| (*t).to_owned()).collect();
    tools.push(format!("mcp__{SERVER}_grimoire_state"));
    tools.push("mcp__home-still_search".to_owned());
    let err = parse_tools_offered(&line(&tools), SERVER).expect_err("a foreign tool is refused");
    assert!(err.contains("mcp__home-still_search"), "names the tool that leaked: {err}");
    assert!(!err.contains("grimoire_state"), "does not name the editor's own: {err}");
}

/// **A turn the model provider refused is a harness error quoting the provider**, never a `fail` with no
/// calls and no tokens. These are the three events omp wrote on this host (2026-11-13) when the provider's
/// weekly usage limit was spent: the trial's agent never ran, and the grader scored an untouched project.
#[test]
fn a_turn_the_provider_refused_is_a_harness_error_not_an_agents_failure() {
    use bevy_cauldron_bench::run::{read_transcript, tools_offered_path};
    let dir = std::env::temp_dir().join(format!("a_turn_the_provider_refused-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    let refused = serde_json::json!({"type": "message_end", "message": {
        "role": "assistant", "content": [], "stopReason": "error", "errorStatus": 429,
        "usage": {"input": 0, "output": 0, "totalTokens": 0},
        "errorMessage": "HTTP 429 from https://ollama.com/api/chat\nyou have reached your weekly usage limit"}});
    let transcript = dir.join("transcript.jsonl");
    std::fs::write(&transcript, format!("{refused}\n")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(tools_offered_path(&dir), registry(&[])).unwrap_or_else(|e| panic!("{e}"));
    let err = read_transcript(&transcript, &dir, SERVER).expect_err("a refused turn is not an agent's result");
    assert!(
        err.contains("HTTP 429") && err.contains("weekly usage limit"),
        "quotes the provider's own words: {err}"
    );
    assert!(err.contains("the provider"), "says whose failure it is: {err}");

    // The same transcript with a turn that answered is read as a result.
    let answered = serde_json::json!({"type": "message_end", "message": {
        "role": "assistant", "content": [{"type": "text", "text": "ok"}], "stopReason": "stop",
        "usage": {"input": 5, "output": 7, "totalTokens": 12}}});
    std::fs::write(&transcript, format!("{answered}\n")).unwrap_or_else(|e| panic!("{e}"));
    let run = read_transcript(&transcript, &dir, SERVER).unwrap_or_else(|e| panic!("an answered turn reads: {e}"));
    assert_eq!(run.tokens, Some(12), "an answered turn's tokens are counted");
    let _ = std::fs::remove_dir_all(&dir);
}
