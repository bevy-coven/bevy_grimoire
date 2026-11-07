# 2026-11-07 feel series: G-43 reduced motion keeps fades, built in bevy_gooey

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 8c68c5b, e19492c · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
No bevy_grimoire item in the epic is ready. UI-R02 #332 depends on UI-R01 #331, which needs the motion items G-43..G-46. This pass claimed and built G-43 (bevy-coven/bevy_gooey#50), the first of them, in `~/family_src/bevy_emerge_loop/bevy_gooey`. Under `MotionSettings::reduced`, nothing travels and fades still run, and `motion_row` is the options row for it.

## Baseline
The bevy_grimoire gate and `accept.py` were not run: no bevy_grimoire code changed.

## Enhancement: G-43 (bevy-coven/bevy_gooey#50)
- **Commit 1, 8c68c5b:** `MotionSettings::reduced: bool` (serde default false) and `MotionSettings::reduced()`.
  - **Assumed owner exception:** `reduced: false` is added to the three protected test literals that spell `MotionSettings` out (`tests/meter_motion.rs:303`, `tests/motion.rs:186`, `tests/motion_trace.rs:532`). This is the new field's empty default; no assertion changed. Gooey's build-loop rule 6 allows exactly this addition.
- **Commit 2, the behaviour:**
  - `lands_travel()` lands every `MotionTarget`, a popover's slide and a meter's fill.
  - A dialog card's and a popover's entrance becomes `Arriving`: a timed ease-in-out fade over `fade_seconds × duration_scale`, stepped by `update_arrivals` and then removed.
  - `Leave` keeps its fade. Off wins over reduced.
  - `motion_row` (Full, Reduced, Off) writes only `MotionSettings`, and Full and Reduced keep the player's own `duration_scale`.
- **Assumed decision (on the issue's label):** the fade-in is fixed-length, like Rune's `REDUCED_FADE`, so that it lasts the 0.15 s the acceptance asks. The exit keeps the presence spring. Colour fades wait for G-44. Recorded in `docs/research/reduced-motion.md`.
- **Proof:** `tests/reduced_motion.rs`, 9 tests at 60 Hz.
  - The card is in place on its first frame and whole after 0.15 s ±1 frame.
  - A popover opens with no slide.
  - Turning reduced on mid-slide lands the slide in the next frame, and an exit keeps fading.
  - A `Leave` during the fade-in leaves from where it had got to.
  - A leave and then a stay during the fade-in bring the card back whole, and a host's opacity written mid-fade is kept.
  - A meter's gain lands.
  - Off wins over reduced.
  - The row steps with two writes of `MotionSettings` and none of any other setting.
  - On 8c68c5b (the field, no behaviour) five of the six behaviour tests fail; the off-wins test passes there, as it should.
- **bevy_gooey gate:** all green after the review fixes: fmt, clippy `--all-targets --all-features -D warnings`, nextest 1100/1100, doctests 247, the wasm check and rustdoc `-D warnings`.

## Borrowed from Mixar
None (the reference is Rune: `crates/rune/README.md` "Animation", `crates/rune-motion/src/props.rs:44,73`).

## Performance
Not measured. `update_arrivals` returns at once when nothing is arriving.

## Acceptance
`accept.py` was not run: no bevy_grimoire code changed.

## Review
The reviewer subagent ran. It found no P1 or P2 problems: the system order is right, no arrival can be left half-transparent, and a settled arrival writes nothing. Of its ten P3 items, these are fixed:
- A fade length that is not a finite number now lands at once.
- A host's `Opacity` written mid-fade is kept, using a new `drawn` field.
- The hidden check comes before the skin lookup.
- The docs no longer say that nothing turns: a spinner and a hold bar keep moving, and the docs now say so.
- Two tests were added: Leave then Stay during the fade-in, and a host's opacity written mid-fade. The popover test now checks the fade's length.
- Stale docs that called `off()` "reduce motion", or counted four option rows, are fixed.

The exit half of the mid-flight test only guards against future changes, as the reviewer noted.

## Test diffs
bevy_gooey: the new `tests/reduced_motion.rs` and its `mod reduced_motion;` line, plus the three `reduced: false` literal defaults described above. No assertion changed.

## Release
None here: nothing in bevy_grimoire's packages changed. G-43 is in bevy_gooey's Unreleased section, for that repo's loop to release.

## Next focus
G-44 (colour motion), G-45 (layout motion) and G-46 (size motion): the rest of UI-R01 #331's needs.
