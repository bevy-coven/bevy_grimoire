# 2026-11-07 feel series: G-54 press scale built in bevy_gooey, G-57 blocked on the evaluator

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 7d06510 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
No bevy_grimoire item in the epic is ready. UI-R01 #331 waits on G-44 and on G-46, which is blocked on the evaluator, and so do UI-R02 to UI-R06, which depend on it. Of the ready bevy_gooey items, G-57 (tooltip timing, #64) turned out blocked as well: `tests/tooltip.rs:215-217` pins a tooltip opening on one pointer move, on a real-time clock. The block is recorded on #64 with a recommendation. This pass built G-54, press scale (bevy-coven/bevy_gooey#61), one of UI-R04 #334's needs.

## Baseline
The bevy_grimoire gate and `accept.py` were not run: no bevy_grimoire code changed.

## Enhancement: G-54 (bevy-coven/bevy_gooey#61)
- **Change:** `src/press_scale.rs` adds `PressScale` (default 0.97) and `PressScaled`.
  - `update_press_scale` springs the scale on `motion.snappy` while the widget is `Pressed` and hovered. It springs back with Rune's `POP` bounce of 0.15. A scroll of an ancestor cancels the press by taking `Pressed` off.
  - `draw_press_scale` scales the subtree's `UiGlobalTransform` after layout. `restore_press_scale` puts the resting values back in `First`, before picking.
  - `mark_press_scale` puts it on every card and slot.
- **Assumed decision (on the issue's label):** the drawing is scaled through the computed transform with a restore, not through an inner content node. The crate's cards and slots have no such node, and scaling only the children would leave the frame and fill full size. Recorded in `docs/research/press-scale.md`.
- **Proof:** `tests/press_scale.rs`, 6 tests at 60 Hz with pointer input.
  - A held card is at 0.97 ±0.005 after `motion.snappy` and is drawn so; its layout size is unchanged.
  - A press 0.5 px inside the edge stays hovered and pressed on each of 30 frames. **This test fails at frame 1 with the restore removed.**
  - A scroll during the hold means the release chooses nothing.
  - Under reduced motion a held card stays at 1.
  - A button has no press scale, and an idle card writes nothing.
  - Full and empty slots both give, and a secondary press does not.
- **bevy_gooey gate:** all green after the review fixes: fmt, clippy `--all-targets --all-features -D warnings`, nextest 1123/1123, doctests 250, the wasm check and rustdoc `-D warnings`.

## Borrowed from Mixar
None (the reference is Rune: `el/visuals.rs:241-250`, `input/hover.rs:360-361`, `rune-motion/src/tokens.rs:12,30`).

## Performance
Not measured. One `Pressed` check per card or slot per frame, and a subtree walk only while one is shrunk.

## Acceptance
`accept.py` was not run: no bevy_grimoire code changed.

## Review
The reviewer subagent ran and confirmed the restore-in-`First` design: no reader gets the wrong transform. Its three P2 findings are fixed:
1. Empty slots (`Role::SlotEmpty`) were never marked.
2. A secondary press shrank the card. The press now asks `PointerPressState::is_primary_pressed`.
3. A popover or context menu hung from a card was scaled with it. `FixedNode` subtrees are now skipped.

Five of its six P3 findings are fixed:
1. Nested restores now drain in reverse.
2. Removing `PressScale` mid-press now stops the drawing.
3. A wheel tick at a list's end no longer cancels the press: scroll values are compared to the ones taken at the press, compared in place.
4. `PressScale` requires `Hovered`.
5. The snappy test is off by one no more.

The sixth, a cancelled click still completing a `DoublePress`, is left and recorded as not covered.

## Test diffs
bevy_gooey: the new `tests/press_scale.rs` and its `mod press_scale;` line. No existing test changed.

## Release
None here: nothing in bevy_grimoire's packages changed. G-54 is in bevy_gooey's Unreleased section.

## Next focus
The epic is now gated on the owner: G-46 (#53) and G-57 (#64) need an evaluator change in bevy_gooey, and G-44 likely does too. The ready gooey items left are G-58 (scroll glide), G-60/61 (need G-44) and G-64 (popover entrance). Check each one's protected tests before claiming.
