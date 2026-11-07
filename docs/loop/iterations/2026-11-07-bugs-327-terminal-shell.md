# 2026-11-07 bugs: #327 not reproduced and guarded; #324 takes its shell setting; the nine standing demos are fresh

Focus: terminal and demos · Commits: 92ef7c38..HEAD (pass 467) · Machine: big · Outcome: shipped

## Summary
No ui or feel item is ready (#331 waits on bevy_gooey#51, the rest on owner decision bevy_gooey#76). The oldest open bug that is not a protected-test call is #327: it did not reproduce, so its acceptance became a guard test and the issue stays open with the evidence. #324's next slice (the shell and its arguments as settings) landed, and the six stale standing demos plus `editor.png` were re-recorded, so `stale.py` reports all nine fresh.

## Baseline
Gate: green on `main` at pass start (push at 190 s). `accept.py` not run: nothing the pass changed touches the acceptance fixtures. Load 17-34 (six CI containers), disk 60 GB free of 3.7 TB, 3 GB RAM available.

## Cluster
- **#327** (bug, oldest unprotected): not reproduced, guard test added, left open. Live under Xvfb with a 15 fps and a 4 fps capture and `editor.ron` rewritten mid-capture: columns 147 to 105 on main, 161 to 113 on a build from before `9cb98fa6`; a 213 report is the runaway `9cb98fa6` fixed (hypothesis, said so on the issue). A `record.py` run of the terminal-settings storyboard with the same write ended at 113 columns.
- **#324** (owner): slice 4, `terminal_shell` and `terminal_shell_args`. Stays open: `CAULDRON_PROJECT` from the assumed decision is still not in (the session knows a working directory, not a project), and the settings screen still has no text control.
- **#322**: unchanged since pass 465's diagnosis (camera ease uses `10.0 * dt`; needs an owner call on protected tests).
- Skipped: #331 to #336 (wait on bevy_gooey#51 and #76), #325 (not started: link detection is new surface; the budget went to the demos), the panel series (#236 to #260, all wait on #210's protected `chrome::NAME` spellings).

## Bugs fixed
None. #327 is a guard: `a_settings_edit_during_a_capture_reaches_the_terminal::a_text_size_saved_while_a_capture_runs_makes_the_terminal_narrower_in_cells` passes on main and did not fail on the build before `9cb98fa6` either, so it is not a failing-first proof.

## Enhancement: #324 slice 4, the shell is a setting
Files: `settings.rs` (two keys, refusals, layering), `terminal/pty.rs` (`ShellChoice`, `spawn_with`), `terminal/mod.rs`, `terminal_view.rs`, `docs/guide/data-files.md`.
Proof: `the_terminal_runs_the_shell_it_is_set_to` (3 tests): a tab with `SHELL=/bin/sh` runs the script the setting names and prints its two arguments, one holding a space; a blank shell and arguments with no shell are refused naming the key; a layer that names a shell brings its own arguments, not the one beneath's. It did not compile before, as slice 3's test.
MCP: none. No operation changed.

## Demos
Re-recorded under Xvfb `:97` on a scratch HOME and a copy of `assets/`, the editor binary copied out of the shared target: agent-drives (74 to 89 frames), agent-removes-then-undo (47 to 49), build-a-map (80 to 81), frame-the-map, move-the-camera, review-labels, and `editor.png`. Each GIF passed `check.py` and its contact sheet was read. build-a-map printed `RESULT: FAIL` on its first run: I had cut the output to its last six lines, so the failing line was not seen, and the events file was then overwritten by the re-record, so the cause is unknown. The same storyboard, unchanged, passed on the re-record (every line PASS, the first change moment at 11.6 %). The GIF in `docs/media/` is the passing one. `hero` and `place-and-undo` were already re-recorded by `91c5d380`; their README rows were wrong and were corrected in pass 465.

## Performance
Not measured: no hot path changed.

## Acceptance
`accept.py` not run (see Baseline).

## Review
No reviewer subagent was spawned. Self-review: (1) Does `set_shell` run before `set_agent`? Yes: `set_agent` marks the settings taken and lets the first shell open, so the first tab already runs the chosen shell; the test starts the editor with the key in the file. (2) Does an edit reach tabs already open? No, by design; the doc line says so. (3) Does a bad shell path fail loudly? `TerminalSession::spawn_with` returns the PTY error and the tab is drawn with the problem, as before. (4) A project's shell with the person's arguments? Refused by construction: arguments come from the layer that names the shell. (5) Is `spawn` still used? Yes by the existing session tests, which pass `None`.

## Test diffs
Two new files (`a_settings_edit_during_a_capture_reaches_the_terminal`, `the_terminal_runs_the_shell_it_is_set_to`), each one `mod` line in `all.rs` (the owner's exception for a new editor test file). No existing assertion changed.

## Release
None by this pass's own choice: no packaged crate changed (`bevy_cauldron` is `publish = false`); the editor's version `0.4.1` was not bumped, as no release was asked.

## Next focus
#324's `CAULDRON_PROJECT` and a settings-screen text field, once bevy_gooey has a text control the screen can use (check `bevy_gooey#76`); #325 link opening.
