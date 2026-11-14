//! **Every operation this editor registers has a row in the agent guide's table** (E-002, #129).
//!
//! `docs/guide/agents.md` is where a person learns what an agent can do and how to take it back. An operation the table
//! does not name is one the guide says nothing about, and the Undo column is where the answer to "can I take that back" lives.
//!
//! **70 operations that were registered before this test have no row yet.** They are listed in [`NO_ROW_YET`], a work
//! list that can only shrink: an operation that is not on it must have a row, and one that is on it and gains a row must come
//! off. A new operation is therefore documented in the commit that registers it.

/// Operations registered before this test existed that the guide's table does not yet name. Delete a name when its row lands.
const NO_ROW_YET: &[&str] = &[
    "board_set",
    "composition_define",
    "composition_set_envelope",
    "data_set",
    "family_set",
    "file_edit",
    "files_replace",
    "fill",
    "forest_scatter",
    "game_build",
    "kit_create",
    "kit_set_authoring",
    "label_apply",
    "label_suggest",
    "layout_accept",
    "layout_discard",
    "layout_propose",
    "map_create",
    "map_open",
    "map_save",
    "note_add",
    "note_answer",
    "note_ask",
    "note_claim",
    "note_edit",
    "note_keep",
    "note_release",
    "note_remove",
    "note_renew",
    "note_resolve",
    "note_send",
    "note_undo_work",
    "pack_set_excluded",
    "people_run",
    "person_set",
    "piece_import",
    "piece_remove",
    "piece_set_cells",
    "piece_set_mount",
    "piece_set_note",
    "piece_set_width",
    "piece_turn",
    "placement_add",
    "placement_set_family",
    "placements_to_run",
    "plan_set",
    "play",
    "play_stop",
    "relations_add",
    "relations_solve",
    "repetition_vary",
    "rig_adopt_measured",
    "rules_set",
    "run_add",
    "run_remove",
    "run_set",
    "snip_arm",
    "stamp_add",
    "terrain_import",
    "terrain_stroke",
    "tool_arm",
    "tree_edit",
    "trigger_set",
    "view_cycle_grid",
    "view_toggle_ghost",
    "view_toggle_journal",
    "vocab_add_token",
    "volume_edit",
    "water_place",
    "water_run",
];

/// **The ratchet on [`NO_ROW_YET`]**, a literal and not `NO_ROW_YET.len()`: a list compared with its own length cannot fail.
/// Lower it in the commit that gives an operation its row. It is never raised: a new operation gets its row, it does not join
/// the list.
const NO_ROW_YET_MAX: usize = 70;

fn has_row(guide: &str, name: &str) -> bool {
    guide.lines().any(|l| l.starts_with(&format!("| `{name}` |")))
}

#[test]
fn every_registered_operation_has_a_row_in_the_agent_guide() {
    let guide = include_str!("../../../docs/guide/agents.md");
    let missing: Vec<&str> = bevy_cauldron::ops::registry()
        .iter()
        .map(|o| o.name())
        .filter(|name| !has_row(guide, name) && !NO_ROW_YET.contains(name))
        .collect();
    assert!(
        missing.is_empty(),
        "operations with no row in docs/guide/agents.md: {missing:?}. Add a row with what it does and how it is undone."
    );
}

/// **The work list only shrinks.** Without this, appending a new operation's name to [`NO_ROW_YET`] would satisfy the test above
/// and "a new operation is documented in the commit that registers it" would be a hope.
#[test]
fn the_work_list_only_shrinks() {
    assert!(
        NO_ROW_YET.len() <= NO_ROW_YET_MAX,
        "NO_ROW_YET has {} names and the ratchet says {NO_ROW_YET_MAX}. The list only shrinks: a new operation gets its row in \
         docs/guide/agents.md in the commit that registers it, and is never added here.",
        NO_ROW_YET.len()
    );
}

#[test]
fn the_work_list_names_only_operations_that_still_lack_a_row() {
    let guide = include_str!("../../../docs/guide/agents.md");
    let registered: Vec<&str> = bevy_cauldron::ops::registry().iter().map(|o| o.name()).collect();
    for name in NO_ROW_YET {
        assert!(registered.contains(name), "`{name}` is on NO_ROW_YET and is not a registered operation: delete it from the list");
        assert!(!has_row(guide, name), "`{name}` has a row now: delete it from NO_ROW_YET, the list only shrinks");
    }
}
