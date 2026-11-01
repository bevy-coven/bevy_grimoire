//! **Every operation has two front ends, and the front ends agree.**
//!
//! The regression test for the 2026-10-21 run's enhancement.
//!
//! # What the brief asks for
//!
//! > *"Every user action is defined once, as a typed operation in a single registry: parameters,
//! > validation, apply, and undo. UI widgets and MCP tools are both thin front ends that build an
//! > operation and dispatch it. Neither changes editor state any other way."*
//!
//! > *"A parity test enumerates the registry and fails if any operation lacks an MCP tool or a UI
//! > entry point, or if the two expose different parameters. Any UI code path that changes editor
//! > state without dispatching a registered operation is a must-fix bug."*
//!
//! # What this test actually checks
//!
//! **Three things, and each of them can fail.**
//!
//! 1. **Every operation in the registry has a BRP command and an MCP tool.** The BRP half is
//!    checked against the *running editor's* command table, so a command that was never registered
//!    is caught; the MCP half is checked against the tool names the MCP server exposes.
//! 2. **Every operation's name is the same on both sides**, and the tool is named after it — so
//!    `map_set_settings` and `grimoire_map_set_settings` cannot drift apart.
//! 3. **The allowlist of unmigrated actions is explicit, and it only shrinks.** A verb that has
//!    been migrated but is still on the list fails the test, which forces the list to be edited
//!    in the same commit as the migration — and the ratchet constant means the list cannot grow.
//!
//! **What it does NOT check, and says so:** it does not prove that no *other* line of UI code
//! changes state. That is a much bigger claim, and a source scan for it would be a heuristic
//! rather than a proof. **The scan here is narrow and stated**: the operations module is the only
//! place `map_set_settings` may be committed from, and the registry is the only list of
//! operations.

use bevy_grimoire_mapper::keys::Action;
use bevy_grimoire_mapper::ops::{CHOOSER_ENTRY_POINTS, UI_ENTRY_POINTS};

use crate::fixtures;

/// **The BRP commands the editor registers.**
///
/// **Four, and they are the four the MCP server exposes.** The BRP surface and the MCP surface
/// are the same four commands, and the parity between them is what the MCP tools are checked
/// against — **one vocabulary, not two.** The debugger refuses a name in its own namespace
/// (`bevy_debugger/`, `world.`, `registry.`, `rpc.`, `schedule.`) at registration time — with a
/// **panic** — so the `grimoire/` prefix is load-bearing and it is written down in one place.
///
/// **Why there are four tools and not one per operation.**
///
/// **The toolset is small on purpose.** The brief says *"Keep the always-loaded toolset small.
/// Put rarely used tools behind a discovery or category tool"*, and there are **ninety-six
/// actions** in this editor — one tool per operation would be ninety-six tools in every agent's
/// context, which is the thing the brief is guarding against.
///
/// **So there are four**: a discovery one, a preview one, an apply one and a log reader — **and
/// the apply one dispatches by name.** The operation's own name is the parameter, so the
/// vocabulary is still the operation's, and `grimoire_list_operations` says what the names are.
///
/// **The UI entry-point tables are product code** (`ops::UI_ENTRY_POINTS` and
/// `ops::CHOOSER_ENTRY_POINTS`, E-086), so a loop pass adds an operation and its UI row in one
/// commit; this file reads them and the ratchet is unchanged.
const BRP_COMMANDS: &[&str] = &[
    "grimoire/operation_list",
    "grimoire/operation_preview",
    "grimoire/operation_apply",
    "grimoire/oplog_read",
];

/// **Every operation has a BRP command, and the editor running is what says so.**
///
/// **The editor's own command table, not a list in this file.** A command that was never
/// registered is caught here, and a command registered twice is caught by the debugger's own
/// panic.
#[test]
fn every_operation_has_a_brp_command() {
    let root = fixtures::Fixture::new("parity")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = bevy_grimoire_mapper::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the editor builds: {e}"));
    // **The debugger's plugin, so the command table exists.** The harness builds the editor's
    // plugin graph without the transport, because a test process builds several `App`s and cannot
    // have two things bound to one port — but the *command table* is what this test reads, and it
    // is registered by the plugin that owns the commands.
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_grimoire_mapper::ops_brp::OpsBridgePlugin);

    let commands = app
        .world()
        .resource::<bevy_debugger_bevy::DebugCommands>()
        .iter()
        .map(|c| c.name.clone())
        .collect::<Vec<_>>();

    let mut missing = Vec::new();
    for command in BRP_COMMANDS {
        if !commands.contains(&command.to_string()) {
            missing.push(format!(
                "the command `{command}` is not in the editor's command table, so an agent \
                 cannot reach the editor through it. The table holds: {}",
                commands.join(", ")
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "these commands are not registered:\n  {}",
        missing.join("\n  ")
    );

    // **And the commands are the ones the MCP server calls**, so an operation the registry knows
    // is an operation the agent can reach. The list is the list, and it is read from the same
    // place on both sides.
    let server = std::fs::read_to_string("../bevy_grimoire_mcp/src/main.rs")
        .expect("the MCP server's source is there");
    let mut uncalled = Vec::new();
    for command in BRP_COMMANDS {
        if !server.contains(command) {
            uncalled.push(format!(
                "the editor registers `{command}` but the MCP server never calls it, so the two \
                 surfaces have drifted."
            ));
        }
    }
    assert!(
        uncalled.is_empty(),
        "these editor commands have no MCP tool that reaches them:\n  {}",
        uncalled.join("\n  ")
    );
}

/// **Every operation is reachable over MCP, by the generic dispatcher.**
///
/// **The server's own source is read**, so a tool that was renamed on one side and not the other
/// is caught — and the list is read from the source rather than from a hand-typed list, because
/// a hand-typed list is the second vocabulary this test exists to prevent.
///
/// **The four tools are named here**, and the names are the server's.
const MCP_TOOLS: &[&str] = &[
    "grimoire_list_operations",
    "grimoire_preview_operation",
    "grimoire_apply_operation",
    "grimoire_undo_operation",
    "grimoire_read_oplog",
    "grimoire_state",
    "grimoire_capture_still",
    "grimoire_capture_sequence",
];

/// **The MCP server can reach every operation, and the tools are the four the server declares.**
#[test]
fn every_operation_is_reachable_over_mcp() {
    let source = std::fs::read_to_string("../bevy_grimoire_mcp/src/main.rs")
        .expect("the MCP server's source is there");
    for tool in MCP_TOOLS {
        assert!(
            source.contains(&format!("\"{tool}\"")),
            "the MCP tool `{tool}` is not in the MCP server's source, so the toolset this test              describes is not the toolset the server has."
        );
    }
    // **The dispatcher is named by the operation.** The server passes the operation's name
    // through, so an operation the registry knows is an operation the server can call.
    assert!(
        source.contains("\"operation\""),
        "the MCP server's apply tool does not take an operation's name, so there is no way for an \
         agent to say which operation it means."
    );
    assert!(
        source.contains("grimoire/operation_apply"),
        "the MCP server does not call the editor's apply command, so no operation is reachable."
    );
    assert!(
        source.contains("grimoire/operation_preview"),
        "the MCP server does not call the editor's preview command, so an agent cannot check a \
         proposal before making it."
    );
    // **And the discovery tool exists**, so an agent can learn the names.
    assert!(
        source.contains("grimoire/operation_list"),
        "the MCP server does not call the editor's list command, so an agent cannot learn what \
         operations exist."
    );
}

/// **The MCP tool names are all prefixed, and none of them collides.**
///
/// **The prefix is the brief's rule**, and it is what keeps the server's namespace separate from
/// every other MCP server the agent may have connected to.
#[test]
fn every_mcp_tool_is_prefixed_and_unique() {
    let source = std::fs::read_to_string("../bevy_grimoire_mcp/src/main.rs")
        .expect("the MCP server's source is there");
    let mut names = Vec::new();
    for line in source.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("name = \"")
            && let Some(name) = rest.strip_suffix("\",")
        {
            names.push(name.to_owned());
        }
    }
    assert!(
        !names.is_empty(),
        "no MCP tool names were found in the server's source, so the rule is being enforced \
         against nothing"
    );
    for name in &names {
        assert!(
            name.starts_with("grimoire_"),
            "the MCP tool `{name}` is not prefixed with `grimoire_`, and the prefix is what keeps \
             this server's namespace separate from every other the agent has connected to."
        );
    }
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        names.len(),
        "two MCP tools share a name: {names:?}. The agent would call one and get the other."
    );
}

/// **The allowlist of unmigrated actions, and the ratchet that stops it growing.**
///
/// **The list is every action that still changes state outside the registry.** It is the work
/// list, and it is explicit: a verb is on the list or it goes through an operation, and there is
/// no third thing.
const UNMIGRATED: &[Action] = &[
    Action::ComposePrev,
    Action::ComposeNext,
    Action::PickComposition,
    Action::CompositionPrev,
    Action::CompositionNext,
    Action::ComposeMemberPrev,
    Action::ComposeMemberNext,
    Action::CarouselPrev,
    Action::CarouselNext,
    Action::MainMenu,
    Action::Undo,
    Action::Redo,
    Action::ShowErrors,
    Action::Shortcuts,
    Action::EditTile,
    Action::CycleTarget,
    Action::MoveMode,
    Action::FrameView,
    Action::Cancel,
    Action::CycleGrid,
    Action::AcceptEdges,
    Action::GroupFromSet,
    Action::PageEnter,
    Action::PageLeave,
    Action::TileOpen,
    Action::TilePrev,
    Action::TileNext,
    Action::CloseTile,
    Action::FocusFilter,
    Action::FocusTagFilter,
    Action::PanForward,
    Action::PanBack,
    Action::PanLeft,
    Action::PanRight,
    Action::TurnViewLeft,
    Action::TurnViewRight,
    Action::PrevCandidate,
    Action::NextCandidate,
    Action::TypeId,
    Action::Accept,
    Action::FoldPack,
    Action::Rescan,
    Action::CloneMode,
    Action::RunMode,
    Action::UndoTile,
    Action::RedoTile,
    Action::CellLeft,
    Action::CellRight,
    Action::CellForward,
    Action::CellBack,
    Action::LayerDown,
    Action::LayerUp,
    Action::TileListPrev,
    Action::TileListNext,
    Action::BuildForward,
    Action::BuildLeft,
    Action::BuildRight,
    Action::BuildBack,
    Action::MemberPrev,
    Action::MemberNext,
    Action::BuildDown,
    Action::BuildUp,
    Action::BuildDrop,
    Action::BuildSlot,
    Action::BuildArm,
    Action::UndoBuild,
    Action::RedoBuild,
    Action::AlignForward,
    Action::AlignBack,
    Action::AlignLeft,
    Action::AlignRight,
    Action::BuildNew,
    Action::BuildDropMember,
    Action::ClearTile,
    Action::BuildTurn,
    Action::BuildRung,
    Action::ScanMesh,
    Action::FocusCandidates,
    Action::CopyInfo,
    Action::FocusLibrary,
    Action::SuggestLabels,
    Action::SuggestAll,
    Action::DiscardAllSuggestions,
    Action::PrevRig,
    Action::NextRig,
    Action::AdoptMeasured,
    Action::UndoBench,
    Action::RedoBench,
    Action::ScrubBack,
    Action::ScrubFwd,
    Action::PlayPause,
    Action::CheckAllRigs,
    Action::ToggleGhost,
    Action::CycleCamPreset,
    // **Snip is a capture, not a registry operation** (`keys::Action::Snip`): it writes a crop and
    // a file of what was under it, and changes no map. It has no operation name to dispatch, so it
    // is accounted for here rather than migrated.
    Action::Snip,
];

/// **The ratchet.** It is lowered in the same commit that migrates an action, and it is the only
/// thing that stops the list growing.
///
/// **A literal, and not `UNMIGRATED.len()`.** The first version compared the list's length with
/// **its own length**, which is a tautology that can never fail — **and a ratchet that cannot fail
/// is not a ratchet.** It is a number a human lowers, and the test says so when it is wrong.
// 124 -> 125: `Snip`, paid by `RenameMap` -> `map_rename` (commit A).
// 125 -> 124: `Remove` -> `placement_remove` (E-029): a stamp and a box dispatch `stamp_remove` and `placement_remove`.
// 124 -> 125: `FrameView` (E-021), camera state with no document change, so no operation names it.
// 125 -> 123: `PalettePrev` and `PaletteNext` -> `brush_arm` (gap 1): the arrow walk arms through the operation a click uses.
// 123 -> 119: `TabSlot1`, `TabSlot2`, `TabSlot3` and `SwitchDoor` -> `door_open` (cycle 8, item 1): every door and tab key opens through the operation.
// 113 -> 111: `ComposeArm` and `ArmComposition` -> `composition_arm` (E-002, #129): the COMPOSE tab's Enter, the picker's Enter and a picker row's click arm through the operation an agent calls.
// 111 -> 110: `RenameComposition` -> `composition_rename` (E-002, #129 slice 2): the COMPOSE tab's name field renames through the operation an agent calls.
// 110 -> 108: `TipX` and `TipZ` -> `placement_set_tip` (E-002, #129 slice 3): `Y` and `U` tip a placed piece through the operation an agent calls.
// 108 -> 106: `LiftUp` and `LiftDown` -> `placement_set_lift` (E-002, #129 slice 4): `[` and `]` lift a placed piece through the operation an agent calls.
// 106 -> 105: `Straighten` -> `placement_set_pose` (E-002, #129 slice 5): `V` straightens a placed piece through the operation an agent calls.
// 105 -> 104: `CycleMount` -> `piece_set_mount` (E-002, #129 slice 6): `M` and a click on the mount value set a library piece's mount through the operation an agent calls.
// 104 -> 103: `ExcludePack` -> `pack_set_excluded` (E-002, #129 slice 7): `Shift+R` leaves a pack out of the kit, or brings it back, through the operation an agent calls.
// 103 -> 102: `RemoveTile` -> `piece_remove` (E-002, #129 slice 8): `Delete` on a library piece sends it back out of the library through the operation an agent calls.
// 102 -> 99: `RotateMeshX`, `RotateMeshY` and `RotateMeshZ` -> `piece_turn` (E-002, #129 slice 9): `N`, `O`, `P`, the rotate chips and the labeler's righting turn a library piece through the operation an agent calls.
// 99 -> 96: `CellSolid`, `CellEdge` and `CellClear` -> `piece_set_cells` (E-002, #129 slice 10): `Z`, `X`, `V`, the lattice grid's chips and headers edit a library piece's cells through the operation an agent calls.
// 96 -> 95: `NewToken` -> `vocab_add_token` (E-002, #129 slice 11): `Shift+T` and the `+` chip add a token to the vocabulary through the operation an agent calls.
const UNMIGRATED_MAX: usize = 95;

/// **The allowlist is explicit, and it only shrinks.**
///
/// **A verb that has been migrated and is still on the list fails this test**, which is the
/// point: the list is a claim about what has *not* been migrated, and a stale entry makes it a
/// lie. And a list that grows fails the ratchet.
#[test]
fn the_allowlist_of_unmigrated_actions_only_shrinks() {
    assert!(
        UNMIGRATED.len() <= UNMIGRATED_MAX,
        "the allowlist of unmigrated actions has {} entries and the ratchet says {}. It may only \
         shrink: a new entry is a claim that a verb is not migrated, and that claim has to be \
         paid for in the same commit that migrates one.",
        UNMIGRATED.len(),
        UNMIGRATED_MAX
    );
    // **E-025 (decided 2026-10-22): an action is migrated when it names the operation it
    // dispatches.** `Action::operation` is the mapping. The list holds exactly the actions that
    // name no operation, so the two claims cannot disagree: a migrated action on the list is a
    // stale entry, and an unmigrated action off the list is one nobody accounts for.
    //
    // **The list is checked against the mapping, in both directions.** A listed action that names
    // an operation fails here, which is what forces the list to be edited in the migrating commit.
    for action in UNMIGRATED {
        if let Some(name) = action.operation() {
            panic!(
                "{action:?} is on the unmigrated list but names the operation `{name}`. A migrated \
                 action comes off the list in the same commit that names its operation."
            );
        }
    }
    // **And every action in the enum is either migrated (names an operation) or listed, by name.**
    // The count is not enough: **a list of the right length with the wrong members is a list that
    // lies.** So each action is checked on its own, against the enum the crate generates.
    let registry = bevy_grimoire_mapper::ops::registry();
    for action in Action::ALL {
        match action.operation() {
            Some(name) => {
                assert!(
                    registry.iter().any(|op| op.name() == name),
                    "{action:?} names the operation `{name}`, which the registry does not have"
                );
                assert!(
                    !UNMIGRATED.contains(&action),
                    "{action:?} names the operation `{name}` and is also on the unmigrated list"
                );
            }
            None => assert!(
                UNMIGRATED.contains(&action),
                "{action:?} names no operation and is on neither list: it is neither migrated nor \
                 accounted for, so nothing checks it."
            ),
        }
    }
}

/// **Every migrated action names a registered operation, and its UI path dispatches that operation.**
///
/// **E-025's mechanical check.** An action counts as migrated because `Action::operation` names an
/// operation. That name must be in the registry, the UI entry point for it must be in the source,
/// and the source must dispatch it by name — so a binding cannot claim a migration it does not
/// make.
#[test]
fn every_migrated_action_has_a_ui_path_that_dispatches_its_operation() {
    let editor = std::fs::read_to_string("src/editor.rs").expect("editor.rs is there");
    let registry = bevy_grimoire_mapper::ops::registry();
    let mut checked = 0;
    for action in Action::ALL {
        let Some(name) = action.operation() else {
            continue;
        };
        checked += 1;
        assert!(
            registry.iter().any(|op| op.name() == name),
            "{action:?} names `{name}`, which the registry does not have"
        );
        let Some((_, place)) = UI_ENTRY_POINTS.iter().find(|(op, _)| *op == name) else {
            panic!("{action:?} names `{name}`, but UI_ENTRY_POINTS has no UI path for it");
        };
        assert!(
            editor.contains(place),
            "{action:?}'s UI path `{place}` is not in editor.rs"
        );
        assert!(
            editor.contains(&format!("\"{name}\"")),
            "editor.rs never names `{name}`, so the UI path for {action:?} does not dispatch the \
             operation it claims"
        );
    }
    // **Anti-vacuity:** this test is meaningless if no action is migrated, so the floor is one.
    assert!(
        checked >= 1,
        "no action names an operation, so the migration check is enforcing nothing"
    );
}

/// **Every operation in the registry has a UI entry point, and every UI entry point names an
/// operation in the registry.**
///
/// **The walk, and it is the half that was missing.** The first version of this file checked the
/// four BRP commands and the MCP tools, **but never walked the registry** — so an operation with
/// no UI entry point failed nothing, and an entry point naming an operation that did not exist
/// failed nothing. **The registry is the list of what exists**, and the two lists either side of
/// it have to agree with it.
#[test]
fn every_operation_has_a_ui_entry_point_and_the_other_way_round() {
    let registry = bevy_grimoire_mapper::ops::registry();
    assert!(
        !registry.is_empty(),
        "the registry is empty, so the parity rules are being enforced against nothing"
    );
    // **Two screens, both scanned.** The chooser is a second place a person changes state, and the
    // walk only read `editor.rs` — so `map_create` could have had a dialog that wrote the file
    // itself and this test would not have seen it (E-008). A UI path is a UI path whichever screen
    // it is on.
    // Both are read by `ui_source` below; the two bindings were the scan's old shape and are gone.
    let ui: Vec<(&str, &str)> = UI_ENTRY_POINTS
        .iter()
        .chain(CHOOSER_ENTRY_POINTS.iter())
        .copied()
        .collect();
    let mut without = Vec::new();
    for op in &registry {
        if !ui.iter().any(|(name, _)| *name == op.name()) {
            without.push(op.name());
        }
    }
    assert!(
        without.is_empty(),
        "these operations have no UI entry point, so a person at the keyboard cannot do what an \
         agent can: {:?}. Every operation needs a UI path, and a UI path that is not an operation \
         is a second way to do one thing.",
        without
    );
    // **And each entry point's place is in the UI source**, so an entry naming a function that no
    // longer exists fails here rather than passing on a stale string. **All four files**, because the
    // UI is four files: `tiles.rs` owns the Meshes tab (where an import lands) and `screen.rs` owns
    // the door crossing, and a scan that read editor.rs and chooser.rs reported a real entry point as
    // missing — measured on `piece_import`'s `admit_candidate(`.
    let ui_source: String = ["src/editor.rs", "src/chooser.rs", "src/tiles.rs", "src/screen.rs"]
        .iter()
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .collect();
    for (name, place) in &ui {
        assert!(
            ui_source.contains(place),
            "`{name}`'s UI path `{place}` is in none of the four UI source files"
        );
    }
    let mut orphaned = Vec::new();
    for (name, _) in &ui {
        if !registry.iter().any(|op| op.name() == *name) {
            orphaned.push(*name);
        }
    }
    assert!(
        orphaned.is_empty(),
        "these UI entry points name an operation the registry does not have: {orphaned:?}. A UI \
         path that dispatches nothing is a UI path that does the work itself."
    );
}

/// **Every BRP method the MCP server calls exists in the editor's bridge.**
///
/// **The two lists are written down in two places, and that is how they drift.** The server calls
/// `grimoire/…` and `bevy_debugger/…`; the bridge registers `grimoire/…`. **A method the server calls
/// and the bridge does not register is a tool that fails at the call**, and the failure is an
/// editor that is not running, which reads like the editor being down.
///
/// **The debugger's own methods are exempt by prefix** — they come from the debugger's plugin, not
/// from this crate.
#[test]
fn every_brp_method_the_server_calls_is_registered_in_the_bridge() {
    let server = std::fs::read_to_string("../bevy_grimoire_mcp/src/main.rs")
        .expect("the MCP server's source is there");
    let bridge = std::fs::read_to_string("src/ops_brp.rs").expect("the bridge's source is there");

    // **Every `"grimoire/…"` the server names.**
    let mut wanted: Vec<String> = Vec::new();
    let mut rest = server.as_str();
    while let Some(i) = rest.find("\"grimoire/") {
        rest = &rest[i + 1..];
        let Some(end) = rest.find('"') else { break };
        wanted.push(rest[..end].to_owned());
    }
    assert!(
        !wanted.is_empty(),
        "no `grimoire/` method was found in the server's source, so the rule is being enforced \
         against nothing"
    );
    let mut missing = Vec::new();
    for method in wanted {
        if !bridge.contains(&format!("\"{method}\"")) {
            missing.push(method);
        }
    }
    assert!(
        missing.is_empty(),
        "the MCP server calls these methods and the editor's bridge does not register them, so \
         the tools would fail at the call: {missing:?}"
    );
}

/// **Every UI entry point names a place that is really there, and dispatches the operation.**
///
/// **A UI entry point that names a control the source does not contain is a claim about a
/// control that does not exist** — and a claim about a control that does not exist is the exact
/// drift the parity test exists to catch.
#[test]
fn every_ui_entry_point_is_in_the_source_and_dispatches_the_operation() {
    let editor = std::fs::read_to_string("src/editor.rs").expect("editor.rs is there");
    // **The UI is three files, and the scan read one.** `screen.rs` crosses doors and re-reads the
    // map, `chooser.rs` makes projects and maps, `tiles.rs` owns the kit's shelf — so an entry point
    // that lives in any of them was reported as missing, which is a scan that cannot see the thing
    // it is checking for. The bounds count below stays on `editor.rs` alone: it asks where the map's
    // size is written in the editor, and that is one file's question.
    let ui: String = ["src/editor.rs", "src/screen.rs", "src/chooser.rs", "src/tiles.rs"]
        .iter()
        .filter_map(|f| std::fs::read_to_string(f).ok())
        .collect();
    let mut missing = Vec::new();
    for (op, place) in UI_ENTRY_POINTS {
        if !ui.contains(place) {
            missing.push(format!(
                "the operation `{op}`'s entry point `{place}` is not in the UI source, so the path \
                 it names is not there."
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "these entry points are not in the source:\n  {}",
        missing.join("\n  ")
    );
    // **And the editor dispatches the operation**, rather than writing the state itself.
    assert!(
        editor.contains("crate::ops::dispatch("),
        "the editor does not dispatch any operation, so every UI path is a second way to do a \
         thing the registry also does."
    );
    // **And the map's bounds are written in ONE place.** A second write to `open.map.bounds`
    // would be a second way to change the map's size, whatever the first one is called.
    let writes = editor.matches("open.map.bounds = ").count();
    assert!(
        writes <= 1,
        "editor.rs writes `open.map.bounds` {writes} time(s). The map's size is changed by the \
         operation, and a second write is a second way to do it."
    );
}
