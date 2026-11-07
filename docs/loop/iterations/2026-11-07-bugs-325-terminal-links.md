# 2026-11-07 terminal: links open on Cmd+click (#325), CAULDRON_PROJECT (#324), #327 closed; no ui item could be taken

Focus: terminal and the ui series · Commits: e2005635..HEAD (pass 472) · Machine: big · Outcome: shipped

## Summary
UI-R03 to R06 (#333 to #336) wait on open bevy_gooey issues (#58, #66, #60, #64, #65, #67 to #72). The panel series (#236 to #260) was recorded as blocked on #210 through #224; #224 is closed, so they are ready, but each is an integration into a hand-built shell, not a slice. The pass took the terminal's open issues instead.

## Baseline
Gate: green on `main` at start (pass 469's push). Load 17 to 35 (CI containers), 46 GB free, 2 GB RAM available at the start.

## Cluster
- **#325** (terminal links): done, `cf6a8b42`, `Fixes #325`. `terminal/links.rs` finds an `http(s)://` address under a cell over the rows the pane draws; `a_primary_click_opens_a_link` opens it through `app_verbs::open_link`. 7 tests (`a_link_in_the_terminal_opens_on_a_primary_click`); they did not compile before. Not tested: that Primary+click sends the program no mouse report (the press observer returns before it writes; `mouse_down` is private). Found while testing: a wrapped row's trailing blanks are trimmed from `Line::cells`, so joining rows without padding to the pane's columns merged two words into one address; the link now pads to the column count.
- **#324**: slice 5, `CAULDRON_PROJECT` (`ec36b43e`). Left open: a Settings-screen row for the shell and the agent command needs `Value::Text(String)`, and `Value` is `Copy`.
- **#327**: closed. Re-recorded the terminal-settings storyboard with the `editor.ron` write in the middle of a capture: the frames after it show 1.4x text and a bar caret.
- **#322**: the cell-edge flake is fixed (pass 468's `4f706ad8`); the updater-refusal flake did not reproduce in 25 runs of both tests under 30 busy loops. Left open.
- **#236**: the multi-selection's mixed value needs a gooey change; filed **bevy-coven/bevy_gooey#77** (G-67).
- **Skipped:** #333 to #336 (gooey Needs open); #237 to #260 (integration, see Summary and the comment on #208); #113 (the remaining step, removing `vlm.rs`, is blocked by the protected evaluator, as pass 369 found); #129 (stalled behind the protected `Camera` list).

## Demos
hero re-recorded (118 frames, check.py PASS) after pass 469's clip made it stale; place-and-undo is owed: three takes under load 28 each failed one change moment of `check.py` (3.5 to 3.8 % against 5 %), the click's event stamp landing after the frame it changed. The pass-467 GIF stays.

## Test diffs
Two new files, `a_link_in_the_terminal_opens_on_a_primary_click` and `the_shell_knows_the_project_it_runs_beside`, one `mod` line each in `all.rs` (the owner's exception for a new editor test file; the first commit's message says both lines are in it, but they are in the second). `BODY_INSET` became `pub` so a test can aim at a cell.

## Review
No reviewer subagent. Self-review: (1) Can a link's `col` index past a trimmed row? `link_at` returns `None` when `col >= cells.len()`. (2) A wrapped link whose break falls on a space? Padded to the columns, so the space is kept (the failing case). (3) Does Primary+click change selection or keyboard focus? No: the other observers still run, as for a plain click. (4) Two links joined by a comma? Each ends at the next scheme. (5) Windows? `open_in_os` uses `explorer`, as the launcher's links do.

## Release
None: no `publish = true` crate changed.

## Next focus
UI-R03 when gooey#58 and #66 close; the outliner (#237) as a left-dock region once someone decides where the hand-built frame gives it room.
