# 2026-11-06 terminal: keys, the wheel and the mouse reach full-screen programs the way they ask

Focus: the embedded terminal (the owner's ask) · Commits: 3151855, d5896e6, cedb963, 2f4f89c · Machine: big · Outcome: shipped

## Summary
Terminal step 4 of the owner's VS Code series: input that programs understand. Cursor keys now follow application cursor mode and carry modifiers (#311), the wheel drives `less`/`man`/`git log` on the alternate screen (#312), and a program that asks for the mouse gets presses, releases, held-button drags and the wheel in SGR, UTF-8 or legacy encoding, with Shift keeping the mouse for selection (#313).

## Baseline
Gate: green on main. `accept.py`: not run (no editor operation or door the harness drives changed). Load 12.7, free disk 213 GB → 158 GB over the pass (shared target, other loops' builds too), available RAM 17 GB.

## Bugs fixed
### #311: cursor keys ignored application cursor mode
Must-fix: blocks y (arrows wrong in every curses TUI), common y, no workaround y. Measured before building: a Python curses program in a PTY with `keypad(True)` read `ESC [ A` as `[27, 91]` and `ESC O A` as `[259]`.
Regression check: `the_terminal_speaks_to_programs_in_their_modes::an_arrow_is_ss3_in_application_cursor_mode` (failed on base 427d286: `cat -v` printed `^[[A`).

### #312: the wheel did nothing on the alternate screen
Must-fix: blocks n, common y, no workaround n (keys work) — taken with the cluster because it shares the mode reader.
Regression check: `the_wheel_sends_arrows_on_the_alternate_screen_and_nothing_on_the_normal_one` (failed on base: printed `""`).

### #313: mouse reporting was never sent
Must-fix: blocks y (htop, lazygit, tmux, vim mouse), common y, no workaround n.
Regression check: `a_click_reaches_a_program_that_asked_for_the_mouse` (failed on base: printed `"sh-5.3$ cat -v"`); `shift_keeps_the_mouse_for_the_person` guards the bypass. Assumed decision: the right button stays the pane's menu. Not in this slice: mode 1003 hover motion with no button held.

## Enhancement
None beyond the cluster.

## Borrowed from Mixar
None. From xterm's ctlseqs, as xterm.js (VS Code) and Alacritty implement them: PC-style cursor keys, alternate scroll (1007), mouse tracking (1000/1002/1003, 1005/1006), Shift bypass.

## Performance
Not measured: input path only, one write per event.

## Acceptance
`accept.py` not run: the terminal is outside what it drives.

## Review
Reviewer not spawned. Self-review, mine: (1) Can a reported press leave a button stuck down in the program? The release observer is global and falls back to the last cell when let go outside the body. (2) Does a reported press still start a selection? No: `a_drag_begins` returns while `mouse_down` holds. (3) Legacy encoding past column 223? Refused, not wrapped (test). (4) Alt+arrow double-ESC? No: Alt is a CSI parameter now, and the Alt-prefix rule skips sequences that start with ESC. (5) Right click: still the menu; recorded on #313.

## Test diffs
New files: `src/terminal/input_tests.rs`, `tests/the_terminal_speaks_to_programs_in_their_modes.rs`, its `mod` line in `all.rs` (owner's exception); the base-commit failures were taken in a separate worktree, removed after.
Changed: `src/terminal/flow_tests.rs` (written by pass 405), after the first push's gate refused `a_flood_never_holds_one_drain`. Cause measured: the tests used the process's `$SHELL`, fish 4 on the gate, which sends a Primary Device Attributes query at startup and runs nothing for 10 s when it is unanswered (these tests answer nothing; the editor does, via alacritty's `identify_terminal`); and back-to-back drains found the buffer empty under load (3 of 5 runs with 20 busy CPUs). Now `SHELL=/bin/sh` under a lock, as `tests.rs` does, the flood is waited for, and a frame passes before each drain. No assertion removed or loosened; 168/168 under SHELL=fish and 20 hogs (cedb963).

## Release
Cauldron 0.3.2: `release: 0.3.2` (2f4f89c), tag `bevy_cauldron-v0.3.2`. Nothing on crates.io: no publishable crate changed (the editor is `publish = false`).

## Next focus
Terminal step 5: link detection (`path:line:col` and URLs, Ctrl+click), then a translucent selection so a selected inverse cell stays legible.
