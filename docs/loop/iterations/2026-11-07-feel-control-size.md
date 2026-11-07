# 2026-11-07 feel series: G-50 control sizes built in bevy_gooey, G-46 blocked on the evaluator

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 5c96d45 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
No bevy_grimoire item in the epic is ready: UI-R01 #331 needs G-44 and G-46. G-46, size motion (bevy-coven/bevy_gooey#53), was claimed first and found **blocked**: protected tests in `tests/split.rs` and `tests/section.rs` pin instant collapse and close on a real-time clock. The claim was released with the exact assertions and a recommendation (owner-approved `MotionSettings::off()` in those test apps). This pass then built G-50, control sizes (bevy-coven/bevy_gooey#57), one of UI-R03 #333's needs, which needs no evaluator change.

## Baseline
The bevy_grimoire gate and `accept.py` were not run: no bevy_grimoire code changed.

## Blocked: G-46 (bevy-coven/bevy_gooey#53)
Animating the size would re-pin these protected assertions, which is not an addition:
- `tests/split.rs:817-821`, `:857-861` and `:892-895`: the pane is at its collapsed width within two frames of Enter, South or a double click.
- `tests/section.rs:295-300`: the body height is 0 right after a click. `:335` and `:360`: it is over 40 px within a few frames of opening.

Recommendation, recorded on #53 as an assumed decision: the owner adds `MotionSettings::off()` to those two test apps. G-44 (colour motion) is likely to meet the same wall, because colour assertions are read right after hover and press. It was not attempted.

## Enhancement: G-50 (bevy-coven/bevy_gooey#57)
- **Change:**
  - `src/control_size.rs` adds `ControlSize { Small, Regular, Large }` at 24, 28 and 36 logical px, with `height(target) = max(px, target)`.
  - `apply_skin` reads it through `State::control`.
  - A sized button or toggle gets vertical air that centres its content in the size. A sized choice's arrow is the size less the frame. Text fields centre in the size.
  - An unsized control's path is unchanged.
- **Assumed decision:** the issue's names, which are one step below Rune's for the same heights. Recorded in `docs/research/control-size.md`.
- **Proof:** `tests/control_size.rs`, 6 tests.
  - Under the Desk, a button, an icon button, a toggle, a select and a text field are 24, 28 and 36 logical px (±0.5), and twice that in physical px at `UiScale` 2.
  - Under dark every size is at least 44.
  - With no size, every control is at least the skin's target.
  - A choice and a number field grow with their size.
  - A vec field's components are the size.
  - A new size, or one taken off, restyles the select's arrow and the text field in the next frame.
- **bevy_gooey gate:** all green after the review fixes: fmt, clippy `--all-targets --all-features -D warnings`, nextest 1117/1117, doctests 249, the wasm check and rustdoc `-D warnings`.

## Borrowed from Mixar
None (the reference is Rune: `crates/rune-kit/src/lib.rs:182-193`).

## Performance
Not measured: one more optional component read per restyled node.

## Acceptance
`accept.py` was not run: no bevy_grimoire code changed.

## Review
The reviewer subagent ran and found no P1 problems: the unsized path is unchanged. Both P2 findings are fixed and tested:
1. A vec field's framed fields stayed at the target. A part inside a part now takes the next owner's size.
2. Removing a `ControlSize` did not restyle the control. It is now in `Removals`.

The P3 findings are fixed too:
1. A sized `text_area` keeps its top-aligned inset.
2. The restyle test now covers parts and removal.
3. The doc wording on the unsized height and on which node of a field is the size is corrected.

## Test diffs
bevy_gooey: the new `tests/control_size.rs` and its `mod control_size;` line. No existing test changed.

## Release
None here: nothing in bevy_grimoire's packages changed. G-50 is in bevy_gooey's Unreleased section.

## Next focus
UI-R01 #331 waits on G-44 and on G-46, which is blocked on the evaluator. G-51 (button variants) needs G-44. The next ready items are G-54 (press scale) and G-57 (tooltip timing; check its protected tooltip tests first).
