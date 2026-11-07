# 2026-11-07 feel series: G-48 elevation as data, built in bevy_gooey

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 5b022a1 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
None of the epic's bevy_grimoire items (UI-R01 to UI-R06, #331 to #336) is ready: all wait on bevy_gooey. This pass built G-48, elevation as data (bevy-coven/bevy_gooey#55), in `~/family_src/bevy_emerge_loop/bevy_gooey`, after claiming it. G-48 is one of UI-R02 #332's needs, and G-49 waits on it.

## Baseline
bevy_grimoire gate and `accept.py`: not run, because no bevy_grimoire code changed. Load 17, free disk 59 GB (99% used). GPU unleased at the start; leased twice for about 40 s each (`gpu-tenant run 3d --who bevy_gooey@big`) to render the stills.

## Enhancement: G-48 (bevy-coven/bevy_gooey#55)
- **Goal:** a skin's shadows are data, stacked per height and drawn in CSS terms.
- **Files (bevy_gooey):**
  - `src/elevation.rs` (new)
  - `src/skin.rs`: the `elevation` field on every preset, `problems()`, `level_of`, and `BoxShadow` moved out of the floating tuple
  - `src/model.rs`: `shadow_sigma` and `CSS_BLUR_PER_SIGMA`
  - `src/lib.rs`, `src/seed.rs`
  - `tests/elevation.rs` (new) and its `mod` line
  - docs: `docs/research/elevation.md`, `docs/guide/skins.md`, `docs/media/elevation.png` with its row in `docs/media/README.md`, `README.md`, `AGENTS.md`, `CHANGELOG.md`
- **Reference:** Rune's neutral stacks (`neutral.rs`: drops 1/2, 4/8 and 8/16, each with a 0.5 px ring) and its theme colours (`tok.rs`).
- **Blur conversion:** read off Bevy's `box_shadow.wesl`, where the shader's blur is the Gaussian's standard deviation. CSS's blur radius is twice that, so Bevy's value is CSS's divided by 2.
- **Proof:** `tests/elevation.rs`, 7 tests. The API is new, so the file does not compile on 39730eb.
  - Each role reads its stack from `BoxShadow`: a screen panel, a host's own `Role::Panel` and a card are `raised`; a dialog's card is `overlay`; a popover is `floating`; a toast is `overlay` with its colours faded with the toast.
  - `0 8px 24px` converts to Bevy's `Px(12)`.
  - `high_contrast` casts none, and a skin change restyles the shadows.
  - Each preset's stacks blur more at each height.
  - A NaN or negative blur is a `BadSize` problem.
  - A theme step carries the stacks of the look it takes.
  - A role that stops casting gives its shadow back, and a host's own shadow is left alone.
- **Stills:** `docs/media/elevation.png` shows all four presets with the Quit dialog open, rendered under Xvfb on big's GPU. It was re-captured after the spread fix.
- **bevy_gooey gate:** fmt, clippy `--all-targets --all-features -D warnings`, nextest 1071/1071, doctests 241, the wasm check and rustdoc `-D warnings`: all green. The first clippy run refused a complex query type, which is now a type alias.

## Borrowed from Mixar
None (the reference is Rune).

## Performance
Not measured. A shadow is written only when the skin's stack for it differs (`set_if_neq`). The new system runs only on a changed role.

## Acceptance
`accept.py`: not run, because no bevy_grimoire code changed.

## Review
The reviewer subagent ran. It confirmed the CSS-blur-to-deviation factor and that `UiScale` is in the scale factor. Its findings, and what became of them:
1. **The Theme row kept the old skin's shadows (P1).** `with_colours_of` did not copy `elevation`. Fixed, with a test.
2. **Spread was not in CSS terms.** Bevy grows the whole width by `spread_radius`, so the spread is now doubled. The vertical axis still grows in proportion to the width, which is documented.
3. **The toast fade test could not fail.** It now asserts that the shadow alpha equals the stack alpha times the toast's alpha, while the toast is still fading in.
4. **Select lists casts overlay, not floating** (the card is the palette's). Recorded as an assumed decision on #55 and in the research note.
5. **A shadow stayed after a role left the elevated set.** An `Elevated` marker now takes the shadow back, and a host's own shadow is left alone. Tested.
6. **Bevy draws the shadow under the whole box**, so a translucent fill shows some of it. Documented.

Also fixed: the field doc had the wrong default.

## Test diffs
bevy_gooey: one new file, `tests/elevation.rs`, and its `mod elevation;` line in `tests/all.rs`. No existing test changed.

## Release
None here: nothing in bevy_grimoire's packages changed. bevy_gooey's release belongs to its own loop.

## Next focus
G-49, the Desk preset; it needs G-47 and G-48, both now closed. After it, G-52 (whole-pixel text), and then UI-R02 #332 can take the gooey pin.
