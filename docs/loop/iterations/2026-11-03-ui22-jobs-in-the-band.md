# 2026-11-03 UI-22: a background job shows in the band, can be stopped, and an agent sees it too

Focus: the editor's UI on bevy_gooey · Commits: 4d798b0..9ebfc0b · Machine: big · Outcome: shipped

## Summary

`ui` issue #223 (UI-22) is the only one whose every `Depends on` and gooey `Needs` is closed, so it was
the pass's item. A long job now has one record, a line in the status band, a list with a Stop row, and a
door each for the pointer, the keyboard-less agent and the MCP tool — and the palette's thumbnail bake,
which had been invisible and uninterruptible, is the first producer on it.

Also fixed, both found on the way: `record.py`'s `kind_of` never listed `click_label`, so every such
step silently became a caption; and the band's item claimed `Role::ButtonText`, which the workspace
gate caught because a dialogs test counts the interface's buttons by that role.

## Baseline

Gate: green before (4d798b0 passed in 193 s). `accept.py`: not run — this pass's change is editor UI
only, and `accept.py` scores the solver path from `assets/`; no scoreboard file changed.
Load 1.1, free disk 122 GB, available RAM 9 GB.

## Bugs fixed

### #277: the status band's job item does not answer a click in the running editor

Must-fix: blocks n, common y, no workaround n — but it is the *gesture* UI-22 names, so it was measured
and filed rather than swallowed. Filed this pass with the measurement: injected cursor at the band's own
reported centre, `Hovered` lands on a panel at `(226, 397)` instead of the band.

Root cause: **not established.** Ruled out by measurement: the observer, the popover and the Stop row all
work (the headless test sends a real `PointerClick` and the list opens); and it is not the job record
(`grimoire/jobs_stop` sets the same flag the row sets). What is left is the UI picking chain for the
bottom band. A second measurement this pass narrows it: five different logical coordinates aimed at the
palette's `sofa` row all left `armed_piece` unchanged, so map and palette clicks alike are missing —
which is a bigger fault than #277 says, and is why #277 stays open rather than being worked around.
Regression check: none yet — the fix is not found.

### `kind_of` did not carry `click_label` (found while recording the clip)

Must-fix: blocks y (the clip cannot be recorded without it), common n, no workaround y.
Root cause: the step-kind list was written before `click_label` existed and the recorder's own tests
drive `widget_centre` directly, so no storyboard ever went through `kind_of` with one. A `click_label`
step matched nothing and fell through to `caption` — the words were printed on the take and no click
was sent, which from `events.json` reads as a storyboard that only had captions.
Regression check: `tools/demo/test_record.py::test_a_click_label_step_is_a_click_and_not_a_caption`
and `::test_a_prefix_finds_a_widget_whose_words_move` (12 pass; both fail before).

### The band's job item called itself a button

Must-fix: blocks y (the full workspace gate refused the push), common n, no workaround n.
Root cause: the item's words carried `Role::ButtonText`, copied from gooey's `status_item`, and
`a_question_is_asked_in_a_gooey_dialog` counts the interface's buttons by that role. With a bake running
its list read `["Baking previews 0 of 1", "Delete map \`harbour\`", "Keep it"]`. The item is deliberately
not a `Button` (twenty-four global `Activate` observers), so `Role::ControlText` is the honest role.
`StatusKind::Info` now rides the item too, because `Role::StatusMark` hides itself unless the parent's
kind is a warning or an error.
Regression check:
`crates/bevy_cauldron/tests/background_jobs_show_and_can_be_stopped.rs::the_bands_item_does_not_call_itself_a_button`.

## Enhancement: background jobs in the status band (UI-22, #223)

Goal: work that takes longer than a second says so, and can be stopped — from the band, from an agent,
or from MCP.
Files: `crates/bevy_cauldron/src/jobs.rs` (new), `thumbs.rs`, `harness.rs`, `lib.rs`, `screen.rs`,
`ops_brp.rs`; `crates/bevy_cauldron_mcp/src/{main,docs}.rs`; `docs/mcp-tools.md`.
Proof: `background_jobs_show_and_can_be_stopped` — 7 tests. With the `Thumbnails::stopped` guard removed,
`stopping_the_bake_stops_it_and_it_stays_stopped` fails on *"a stopped job leaves the band in the same
frame the producer gives up"*; restored, all 7 pass.

MCP: `grimoire_state.jobs` (read: the band's own words, each key, fraction, whether a stop was asked) and
`grimoire_jobs_stop { key }`. Undo: none — a stop is not a map change and is not logged, which the tool's
description says. Preview: none; the answer says `asked` rather than `stopped`, because the producer stops
at its own next seam. The band's item is not on the `Activate` bus, so no command parity is owed for it.

## Borrowed from Mixar

none.

## Performance

`perf.py` not measured: no display on this machine for its live timing, and this change adds one
change-gated system over a query of one entity.

## Acceptance

`accept.py` not run; `reached` unchanged by construction (no solver path touched).

## Review

Reviewer could not run this pass (no subagent was spawned before the budget's prove phase). The five
questions, answered by me:

1. **Does it do what the issue asks?** Both acceptance items, measured — the record in `grimoire/state`
   live at `14 of 90`, the stop answered `asked`, the record empty three frames later; and the clip.
2. **Is the cancel flag safe across frames?** Yes, and that is the test: the `Arc` is reused across
   reports; re-making it per report was the first draft's bug and `a_cancel_request_outlives_the_next_report`
   is what would catch a regression.
3. **Can a stopped bake come back?** Not without a library change: `unfinished` refuses while `stopped`,
   and `prune` clears the flag when the project changes — because a person who stops the bake and then
   imports a piece meant that piece to have a portrait.
4. **Does the agent door say the same thing as the band?** Asserted equal, not merely present:
   `state.jobs.line == band_said()`.
5. **What does this break?** It did break two dialog tests via the role, caught by the gate and fixed;
   and the media went stale, re-recorded.

## Test diffs

- `crates/bevy_cauldron/tests/all.rs`: one `mod background_jobs_show_and_can_be_stopped;` line (the
  owner's standing exception for a new editor test file).
- `tools/demo/test_record.py`: two new tests, nothing changed or removed.
- `crates/bevy_cauldron/tests/background_jobs_show_and_can_be_stopped.rs`: new.
- No protected assertion was edited, skipped or loosened.

## Release

None: `bevy_cauldron` and `bevy_cauldron_mcp` are `publish = false`, and no publishable crate's files
changed (`git diff --name-only 4d798b0..HEAD -- crates/bevy_grimoire_core crates/bevy_grimoire_rng
crates/bevy_grimoire_solver crates/bevy_oracle` is empty), so there was nothing to release this pass.

## Next focus

#210's first acceptance item needs the owner's word (comment on the issue, `assumed-decision`
recommending the re-pin exception); every other `ui` issue waits on #224, which waits on #210. The next
ready work is therefore #277, the click chain that reaches neither the palette nor the band.
