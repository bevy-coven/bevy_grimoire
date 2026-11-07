# 2026-11-07 feel series: G-44 colour motion built in bevy_gooey, released as v0.40.1

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 779849f, b9aa28c (merged 2bca3af), release e264418 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
G-44 (bevy-coven/bevy_gooey#51), colour motion, is built and released. It was one of the two needs of UI-R01 #331, and with G-46 from pass 464 both are now closed: #331 is ready for its pin move. The loop's steer to publish after each issue is followed: bevy_gooey v0.40.1 is on crates.io.

## Baseline
The bevy_grimoire gate and `accept.py` were not run: no bevy_grimoire code changed.

## Enhancement: G-44 (bevy-coven/bevy_gooey#51)
- **Change:** `src/color_motion.rs`.
  - **Mechanism:** every `Role` requires a `ColorTrack`. `update_color_motion`, after `GooeySystems::Style`, leads each node's background, four border sides, text and icon tint to the colour `apply_skin` wrote, in premultiplied Oklab on the skin's springs. `apply_skin` stays the one writer of the final colour.
  - **Springs:** snappy for a state change, calm for a skin or scope change, and text and icons at half their fill's length. Reduced motion fades over `fade_seconds`.
  - **Lands at once:**
    - a node's first two frames;
    - a node that is not drawn;
    - a pressed or dragged widget, whose release eases;
    - a rebound table row;
    - the data colours: `Role::Tone`, the swatch halves, the picker area and the rail;
    - everything, when motion is switched off, even mid-change.
  - **Alpha-only changes:** a lone change of alpha is a fade. It lands, or follows the colour on its way to the new alpha.
  - **Cost:** a settled node is skipped before any lookup, and writes nothing.
- **Assumed decisions (on the issue, label `assumed-decision`):**
  - "Contrast stays at or above the lower of the two presets" cannot hold between a dark preset and a light one: the label must cross its fill (intermediate value theorem). It is proved for a same-polarity switch. For dark → parchment the label is under 4.5:1 for at most a third of the calm spring (7 of 24 frames measured).
  - The colour-sensitive real-clock test files (22 files, one line each, `app.insert_resource(MotionSettings::off())`, commit 779849f) apply the #76 decision. No assertion changed. Without them, 41 existing tests read a colour on its way.
- **Proof:** `tests/color_motion.rs`, 10 tests at 60 Hz.
  - **Hover:** in and out, also turned round at 3 frames, no step bigger than an uninterrupted change's largest. 30 idle frames write nothing.
  - **Skin change:** every colour of every styled node reaches the new skin's within the calm spring plus a frame.
  - **Label:** it leads its fill. A same-polarity switch keeps contrast.
  - **Press and landing:** a press lands and its release eases. The first frame and a tone land.
  - **Off and reduced:** off lands, and so does off mid-change. Reduced is shorter than snappy.
  - **Fade over a moving colour:** the colour goes on to its new target.
  - The module did not exist before this change.
- **bevy_gooey gate:** fmt, clippy -D warnings, nextest 1143/1143, doctests 250, wasm32 check, rustdoc -D warnings, all green after the review fixes.

## Borrowed from Mixar
None. The reference is Rune's README, "Animation".

## Performance
Not measured. The settled path is a change-tick test per styled node and no lookups.

## Acceptance
`accept.py` was not run: no bevy_grimoire code changed.

## Review
An independent `reviewer` ran: 3 P2 and 4 P3, all fixed before the commit.
- **P2:**
  - Switching motion off mid-change did not land it.
  - Pressed, parent and skin lookups ran before the skip, for every node.
  - Table rows rebound while scrolling shimmered.
- **P3:**
  - A thumb drag eased.
  - A fade over a moving colour threw it to its target.
  - A widget spawned late in a frame faded in.
  - Two tests were weaker than the acceptance: skin change and reduced.

The fixes were not re-reviewed. The gate and the tests, including new ones for off mid-change and a fade over a moving colour, cover them.

## Test diffs
- New `tests/color_motion.rs` and its `mod` line in `tests/all.rs`.
- One line added to each of 22 existing test files (assumed owner exception, #76). No assertion changed.

## Release
bevy_gooey **v0.40.1**, https://crates.io/crates/bevy_gooey/0.40.1. Tag v0.40.1, commit e264418, and `cargo info bevy_gooey@0.40.1` resolves. `cargo semver-checks check-release` said no semver update required, so PATCH. This repo's crates did not change.

## Next focus
UI-R01 #331: move this repo's `bevy_gooey` pin to e264418 and adopt the motion row.
