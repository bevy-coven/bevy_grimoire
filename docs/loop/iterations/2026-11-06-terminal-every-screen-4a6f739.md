# 2026-11-06 terminal: open on every screen from launch, Cmd/Ctrl+J collapses it

Focus: the embedded terminal, the owner's ask of 2026-11-06 ("the terminal is the centerpiece of work … on every screen from the moment you open the application … collapsible by hitting Command J or Control J") · Commits: dd66a11 (code), 7fcdf2d (docs, clip), 4a6f739 (release) · Machine: big · Outcome: shipped

## Summary
The terminal is docked across the bottom of the projects home, the project page, the loading screen and the editor, raised with its shell running from the first frame and without the keyboard (#314). Primary+J — Cmd+J on a Mac, Ctrl+J elsewhere — collapses and restores it on every screen, and the collapse carries to the next screen (#315). While the shell has the keyboard, gooey's keyboard navigation and Bevy's Tab navigation no longer also act on the keys (#316).

## Baseline
Gate: green on main at e5c5c4b. `accept.py`: not run (no operation or door the harness drives changed). Free disk 192 GB, available RAM 18 GB.

## Bugs fixed / issues closed
- #314 (new, owner): `TerminalDock` marker on screen roots + an `On<Add>` observer; pane built raised; opens at a third of its screen (clamped 96..320 px) and follows it until dragged; `Drawn` reset per body; the pane steps out of the layout while the Hold-K overlay is up.
- #315 (new, owner): `COLLAPSE_CHORD`, collapse state on `Terminal`, swallowed from the shell.
- #316 (new, bug found while designing #314): `hold_the_keyboard` in PreUpdate clears the terminal's keys from this frame's presses and puts the input focus on the body, whose observer stops focused keys.
- #317 (filed, not fixed): a `bevy_fairy` click on the pane does not give it the keyboard in the live editor (`TerminalFocus` stays false); the clip uses Ctrl+` instead.

Regression checks: `tests/the_terminal_is_on_every_screen.rs`, 5 tests. On the base e5c5c4b in its own worktree: 5/5 failed. With only `hold_the_keyboard` disabled: "ArrowDown typed into the shell moved the home's focus to Some(769v1)". With only the chord swallow removed: the shell showed `echo A` / `A` / `sh: B: command not found`. With all of it: 5/5, the crate 1808/1808.

## Measured on the way
With the terminal at a fixed 320 px in a 1280x720 window (600 px of UI), the editor's own tests failed on real layout: the home's New project button under the terminal, the Place palette 189 px, the Hold-K legend over the shell's prompt, leaders crossing on Anim and Meshes. A quarter of the screen gave 4 rows (under the 5-row floor); a third gives 7. Keeping the badges out of the terminal's rect was tried and made it worse (band badges below the terminal lost their band), so the terminal steps aside while K is held instead.

## Test diffs (owner exception, in conversation 2026-11-06)
- `a_terminal_runs_a_shell_inside_the_editor`: "a shell must not start until somebody asks" → before any chord the pane is raised, a shell runs, and the terminal does not hold the keyboard.
- `the_editor_spawns_a_laid_out_split_divider`: `height > 500.0` → the frame body is taller than 250 px and the divider spans it top to bottom (±1 px).
- New file `the_terminal_is_on_every_screen.rs` and its `mod` line (owner's new-test-file exception).

## Review
Reviewer not spawned. Self-review, mine: (1) A screen with no `TerminalDock` gets no pane — the editor docks its own, every other screen root names the marker. (2) Collapse survives screens through `Terminal.collapsed`; `step_aside_for_the_key_overlay` re-asserts `!collapsed` each frame, so a chord and the overlay cannot disagree. (3) `hold_the_keyboard` only clears `just_pressed`, so held modifiers still read for the shell's Ctrl/Alt. (4) Input focus is let go only if it is still the body. (5) Known gap: menu-item shortcuts (gooey `Shortcut` on menu rows, e.g. Ctrl+O) still fire while the shell has the keys — VS Code's commands-to-skip-shell question, not taken here.

## Release
Cauldron 0.3.3: `release: 0.3.3` (4a6f739), tag `bevy_cauldron-v0.3.3` pushed. Nothing on crates.io: the editor is `publish = false` and no publishable crate changed.

## Next focus
#317 (click to focus the terminal), then which window chords the shell should keep (Ctrl+O, Ctrl+P) while it holds the keyboard.
