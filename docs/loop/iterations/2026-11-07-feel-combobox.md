# 2026-11-07 feel series: G-66 combobox built in bevy_gooey (v0.43.0), and the eight standing demos re-recorded

Focus: epic #330 (owner's steer, which now also says re-record the demo GIFs each loop) · Commits: bevy_gooey 4a29aab (merged 32dfb72), release 1f7e4ad; bevy_grimoire demos below · Machine: big · Outcome: shipped

## Summary
G-66 (bevy-coven/bevy_gooey#73) is a labelled field with a popover list under it that narrows as the player types. UI-R03 (#333) has no open Needs left: the next pass moves the editor's pin to v0.43.0. The eight standing GIFs and `editor.png` were re-recorded on the current editor (origin/main 3e9d9bb8), under a private Xvfb on big.

## Baseline
bevy_grimoire code was not touched, so its gate and `accept.py` did not run. Disk on big was 640 GB free after the runner caches pruned, load high from CI.

## Enhancement: G-66 combobox
- **Change:** `src/combobox.rs`: `combobox(label, options)`, `ComboOption { id, label, detail, enabled }`, `ComboValue`, `Combobox` (state), `combobox_by_input`, `update_comboboxes`, `keep_focus_on_escape`; hooks in `text.rs` (Enter belongs to an open list) and `nav.rs` (Up and Down belong to the field).
- **Assumed decisions (on the issue):** a plain popover (the open-from-anchor motion of #71 is open); one row per option; no pad path.
- **Proof:** `tests/combobox.rs`, 9 tests: forty options, "par" narrows, Down, Enter sends one `ValueChange` with the id; rank order; Ctrl+N and Ctrl+P wrap over enabled rows; disabled refusals; a click; no match; Escape then Cancel; focus leaving; a starting value with a neighbour below.
- **Gate:** fmt, clippy -D warnings, nextest 1200/1200, doctests 261, wasm32 check, rustdoc -D warnings.

## Review
An independent `reviewer` ran: 3 P1 and 1 P3, all fixed, each with a test or by the gate.
- **P1:** the PreUpdate system took a missing input resource (a style-only app panicked); fixed with an `Option`.
- **P1:** `ComboValue` was in the bundle twice when a game gave a starting value; it is now a required component.
- **P1:** Up and Down moved focus off the field in any form with a neighbour; `navigate_by_input` leaves them to the field.
- **P3:** a game's write to the field row read as typing; the open-on-typing needs focus.

A race that only a test showed: a write to the field row's value, before the field's own text caught up, read as the player typing "" after one frame. Both are written together now.

## Demos (owner, 2026-11-07: re-record them each loop)
All eight GIFs and `editor.png` re-recorded from the editor built at 3e9d9bb8, with `tools/demo/retake.sh` (new): a private Xvfb `:96` at 1280 x 720, a fresh editor and a fresh copy of `assets/` per take, the first take that passes `check.py`, and each contact sheet looked at.
- **Takes needed:** hero 2, agent-removes-then-undo 1, review-labels 3, the others 1 (the clean run).
- **A first pass was wrong and thrown away:** the script took the shell's `$!` of a `nohup` wrapper as the editor's pid, so nineteen editors piled up on port 15710 and every take recorded the oldest one's state (a `hero` take showed "no placement called bed"). The script now `exec`s the editor and kills by command line.
- **Captions:** frame counts updated (hero 116, agent-drives 78, agent-removes-then-undo 48, place-and-undo 50); `stale.py` says all nine fresh. The fridge preview that stands at the pointer in `hero` and `agent-removes-then-undo` was in the old files too.

## Test diffs
New `tests/combobox.rs` and its `mod` line in `tests/all.rs` (bevy_gooey). None in bevy_grimoire.

## Release
bevy_gooey **v0.43.0**, https://crates.io/crates/bevy_gooey/0.43.0 (tag v0.43.0, commit 1f7e4ad). MINOR: `cargo semver-checks` flagged `begin_entrances` (the sibling pass's G-65). It also ships G-53 and G-65 from the sibling pass. bevy_grimoire has no crate change.

## Next focus
UR03 (#333): move the editor's gooey pin to v0.43.0 (editor tests that read a tooltip need `TooltipSettings { delay_seconds: 0.0 }`, and ones that read a scroll after a key need `MotionSettings::off()`), then adopt the variants, default button, control size and combobox. Re-record the demos again after it.
