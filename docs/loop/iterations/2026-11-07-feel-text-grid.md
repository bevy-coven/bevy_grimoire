# 2026-11-07 feel series: G-52 text on the device-pixel grid, built in bevy_gooey

Focus: epic #330 (owner's steer) · Commits: bevy_gooey a596481 (merge c69b73b) · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
None of the epic's bevy_grimoire items (UI-R01..R06, #331-#336) was ready at the start: all wait on bevy_gooey. This pass claimed and built G-52 (bevy-coven/bevy_gooey#59), the last open need of UI-R02 #332, in `~/family_src/bevy_emerge_loop/bevy_gooey`. Every widget's text size is now rounded up to a whole number of device pixels at the node's scale.

## Baseline
The bevy_grimoire gate and `accept.py` were not run: no bevy_grimoire code changed.

## Enhancement: G-52 (bevy-coven/bevy_gooey#59)
- **Goal:** glyphs set at a whole pixel size at 1.25x and 1.5x, not softened between two.
- **Change:**
  - `snap_text_size(size, scale)` is new in `src/model.rs`, beside `snap_line_width`, and shares its ulp stepping. It rounds `size × scale` up, with Rune's tolerance (`ceil(x − 1e-3)`).
  - `apply_skin` writes every `TextFont` size through it, at `ComputedUiRenderTargetInfo::scale_factor` (the window's scale factor × `UiScale`). It is the crate's only `TextFont` write.
  - The restyle filter already holds `Changed<ComputedUiRenderTargetInfo>`, so a new scale sets every text again.
  - A list's row, a text field's inset and a keycap's height now use the body text as it is set.
- **Assumed decision (on the issue's label):** the font size is rounded, as the issue proposes. Rune rounds its measured text box instead; `docs/research/text-grid.md` records the departure.
- **Proof:** `tests/text_grid.rs`, 6 tests.
  - At 1, 1.25, 1.5 and 2x with text 1.1, every widget text is on the grid and within one device pixel of its own role's unrounded size.
  - 15.4 px at 1.25x is set at 16.
  - A new window scale and a new `UiScaleSettings::scale` set every text again.
  - A host's own `Text` keeps 13.3 px.
  - A Desk list 8 rows tall does not scroll.
  - A sweep covers `snap_text_size`.
  - On 42a9cf5 the three sizing tests fail (15.4 stays 15.4; a 35.2 px heading at 1x; 27.5 device px). The list test failed with the old row height (1.6 px overflow).
- **bevy_gooey gate:** all green, run after merging gooey's v0.39.0 work with one conflict in `restyle`, where both sides were kept: fmt, clippy `--all-targets --all-features -D warnings`, nextest 1091/1091, doctests 246, the wasm check and rustdoc `-D warnings`.

## Borrowed from Mixar
None (the reference is Rune: `crates/rune/README.md` "Layout", `runtime/commit/text_layout.rs` `CeilTo`).

## Performance
Not measured: one `ceil` per restyled text.

## Acceptance
`accept.py` was not run: no bevy_grimoire code changed.

## Review
The reviewer subagent ran. It found one P2 and three P3 items, and all are fixed:
- **P2:** list rows, the text-field inset and keycap heights still used the unrounded size, so a list of `rows` rows overflowed. Fixed with `set_px`, and a regression test added.
- **P3:** the research note claimed the text is re-set in the same frame. It is the frame after, as for line weights, and the note and the `apply_skin` doc now say so.
- **P3:** the main test matched any asked size. It now checks each role against its own size and covers a type-scale step (a cooldown chip).
- **P3:** the doc wording on the 1e-3 tolerance is fixed.

## Test diffs
bevy_gooey: one new file, `tests/text_grid.rs`, and its `mod text_grid;` line in `tests/all.rs`. No existing test changed.

## Release
None here: nothing in bevy_grimoire's packages changed. bevy_gooey's own loop released v0.39.0 during this pass, with G-47, G-48, G-49 and its own G-42. G-52 is in its Unreleased section for its next release.

## Next focus
UI-R02 #332 now has every gooey need closed, but it depends on UI-R01 #331. #331 still needs the motion items G-43..G-46 (gooey#50-#53); G-42 closed in gooey's own loop this pass. The next pass takes the next ready feel item in bevy_gooey that its loop is not holding.
