# 2026-11-01 ui: an agent runs a command by id (UI-90, #256)

Focus: UI series · Commits: b70184a · Machine: big · Outcome: shipped

## Summary
#201 (H4) and #202 (H5) still wait on re-pinning existing assertions; every other open `ui` issue waits on an open gooey G-NN or on UI-10/UI-30, except UI-90 (no Depends open, no Needs). `grimoire/run_command {id}` looks the id up in `commands.rs`, refuses it with the registry's own `enabled()` reason, and otherwise queues the command's chord (`command_run.rs`): modifier and key go down together in one frame's `PreUpdate`, before Bevy's keyboard system, and come up the next. So the verb runs through the one dispatcher, with its gates, receipt and undo stack. `grimoire_state.ui` reports `screen` (`launcher`/`workbench`), `workspace`, `panels` and `focus`.

## Baseline
Gate: `main` as pushed by pass 197. Load ~12, free disk 264 GB, available RAM 13 GB.

## Enhancement: commands over MCP
Goal: an MCP call to `edit.undo` undoes exactly as Cmd+Z; `ui.screen` reads `workbench` · Files: `mapper/src/command_run.rs` (new), `ops_brp.rs`, `lib.rs`, `mcp/src/main.rs`, `docs.rs`, `docs/mcp-tools.md`, `docs/guide/agents.md`, `keys.md`.
Proof: `tests/a_command_runs_by_id_through_the_agents_door.rs` (4): failed first with `no method grimoire/run_command is registered` and `ui: null`; passes after. A twin editor takes Cmd+Z and the two maps are equal; redo puts the run back; a disabled command is refused with the registry's reason and the map is unchanged; an unknown id names `edit.undo`. `cargo test -p bevy_grimoire_mcp` green (tool table says sixteen, `docs_are_current`).
MCP: one tool, `grimoire_run_command` (16 tools); not an operation, so not in the oplog; the undo it triggers is the editor's own stack.

## Test diffs
`tests/all.rs`: one `mod` line. mcp `tests/a_tools_body_names_every_field.rs`: one `FORWARDING` row (`grimoire/run_command`, `RunCommandParams`), an assumed owner exception (an addition). No assertion removed or loosened.

## Release
None: the mapper and mcp crates are `publish = false`; no publishable crate changed.

## Next focus
The next ready `ui` issue; none is ready until a gooey G-NN closes (G-05 menu bar, G-16 searchable select, G-25 field validation, G-27 testing helpers, G-29 named tones).
