//! **The shipped guide's examples run, and name operations that exist** (#346, AG-01).
//!
//! The guide is `crates/bevy_cauldron_mcp/src/guide.md`, compiled into the MCP server and returned by
//! `grimoire_guide`. It carries one fenced `json` block of operations, which is what the person's
//! Send and the agent's work actually dispatch. **An example that names an operation the editor no
//! longer has is a guide that lies**, and a guide is read by an agent that cannot tell.
//!
//! **Two tests, because there are two ways it can be wrong.** One runs every line against a real
//! project and map, through `ops::dispatch` — the same door `grimoire_apply_operation` reaches — so an
//! example that no longer parses or is refused fails here. The other checks the same function on
//! doctored text, so the first test's pass is a fact about the guide rather than about a checker that
//! agrees with everything.
//!
//! **No `App`, and that is deliberate.** These tests open the project and the map and dispatch
//! operations, which is what the example needs and all it needs: a headless editor builds every
//! render type and waits on a device even with `backends: None`, and on this machine it does not
//! come back at all (an untouched test in `send_starts_the_notes_agent` hangs the same way).

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_cauldron::project::{OpenMap, Project};
use serde_json::{Value, json};

use crate::fixtures;

/// **The guide, as it ships.**
const GUIDE: &str = include_str!("../../bevy_cauldron_mcp/src/guide.md");

/// **The one fenced `json` block of the guide**, its lines parsed one operation each.
fn example_lines(guide: &str) -> Result<Vec<Value>, String> {
    let block = guide
        .split("```json\n")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .ok_or_else(|| "the guide has no fenced json example".to_owned())?;
    let lines: Result<Vec<Value>, String> = block
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|e| format!("{line}: {e}")))
        .collect();
    let lines = lines?;
    if lines.len() < 5 {
        return Err(format!(
            "the example is {} operations; the notes workflow is at least five",
            lines.len()
        ));
    }
    Ok(lines)
}

/// **Every operation an example names, checked against the editor's own registry.**
///
/// **The registry, not the guide**: `ops::find` is the same list `grimoire_list_operations` reports and
/// the same list `grimoire_apply_operation` dispatches through, so an example cannot name a verb that
/// only ever existed in prose.
fn check_operations_exist(lines: &[Value]) -> Result<(), String> {
    let missing: Vec<String> = lines
        .iter()
        .map(|line| line["operation"].as_str().unwrap_or_default().to_owned())
        .filter(|name| ops::find(name).is_none())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the guide's example names {missing:?}, which are not operations this editor has"
        ))
    }
}

/// **Every line of the guide's example runs against the real project and map**, the agent's as the
/// agent.
#[test]
fn the_guides_example_runs_against_the_editors_operations() {
    let lines = example_lines(GUIDE).unwrap_or_else(|e| panic!("{e}"));
    check_operations_exist(&lines).unwrap_or_else(|e| panic!("{e}"));

    let root = fixtures::Fixture::new(&format!("guide_runs_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let project = Project::open(&root, None).unwrap_or_else(|e| panic!("{e}"));
    let open = OpenMap::open(&project, "m").unwrap_or_else(|e| panic!("{e}"));
    let mut ctx = Ctx {
        project: Some(project),
        open: Some(open),
        session: "guide".to_owned(),
        ..Ctx::default()
    };
    for line in &lines {
        // **Who did it is what the fence is checked against**, and the example says so per line: a line
        // carrying `agent` is the agent's, and one without is the person's.
        ctx.source = if line["parameters"]["agent"].is_string() || line["agent"].is_string() {
            ops::Source::Agent
        } else {
            ops::Source::Human
        };
        let op = line["operation"].as_str().unwrap_or_default().to_owned();
        match ops::dispatch(&mut ctx, &op, &line["parameters"])
            .unwrap_or_else(|e| panic!("{op}: {}", e.0))
        {
            Applied::Done { .. } => {}
            Applied::Refused(e) => panic!("{op} refused: {}", e.0),
        }
    }
    let open = ctx.open.as_ref().unwrap_or_else(|| panic!("the example closed the map"));
    let notes = bevy_grimoire_core::notes::Notes::load(&bevy_grimoire_core::notes::path_for_map(&open.map_path))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        matches!(
            notes.notes.first().map(|n| &n.state),
            Some(bevy_grimoire_core::notes::State::Resolved { .. })
        ),
        "the example ends with a resolved note: {:?}",
        notes.notes.first().map(|n| &n.state)
    );
}

/// **An example naming an operation that does not exist is caught**, so the check above means
/// something.
///
/// **The same function, on doctored text**: one line's operation is replaced with a name no registry
/// holds. If this passed, the check would be a check of nothing, and the guide could name a verb the
/// editor does not have without a test noticing.
#[test]
fn an_example_naming_an_operation_that_does_not_exist_is_caught() {
    let lines = example_lines(GUIDE).unwrap_or_else(|e| panic!("{e}"));
    check_operations_exist(&lines).expect("the shipped example names only operations that exist");

    let mut doctored = lines.clone();
    doctored[1]["operation"] = json!("note_send_now_which_is_not_an_operation");
    let complaint = check_operations_exist(&doctored).expect_err("a doctored example must be refused");
    assert!(
        complaint.contains("note_send_now_which_is_not_an_operation"),
        "the complaint names the operation the example invented: {complaint}"
    );
}
