# 2026-11-07 feel series: G-46 size motion built in bevy_gooey, under the assumed evaluator decision #76

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 75f8cc0 (not a commit of this repo), 1deec8a (release d6fa6dd) · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
The owner had not replied to bevy-coven/bevy_gooey#76, and the loop's steer says "Always go with your recommendation and don't prompt me". So this pass applied #76's recommendation as an assumed decision, one file at a time: `tests/split.rs` and `tests/section.rs` run with `MotionSettings::off()`, and no assertion changed. It then built G-46, size motion (bevy-coven/bevy_gooey#53), one of UI-R01 #331's two remaining needs. The owner's interjected steer, "Ensure to publish a new version after each issue is pushed", is followed: bevy_gooey is released after the push (Release below).

## Baseline
The bevy_grimoire gate and `accept.py` were not run: no bevy_grimoire code changed.

## Assumed owner exception (#76): commit 75f8cc0
Each file gains one line, `.insert_resource(MotionSettings::off())`, in its real-time `app()`. Both files pass 43/43 before the change and after it. No assertion changed. Recorded on #76, which the owner may override.

## Enhancement: G-46 (bevy-coven/bevy_gooey#53)
- **Change:** `src/size_motion.rs` adds `SectionGlide` and `SplitGlide`.
  - **Sections:** a section toggled after its first frame grows or shrinks its clipped body toward its widgets' height (`content_size`) or 0, on `motion.default`, in whole device pixels. When it ends, the body's own height comes back, or the body is hidden.
  - **Splits:** `toggle_split` slides the panes from where they are laid out.
  - The state is the new value at once. A drag, a key step, a game's write and reduced motion land.
- **Assumed decisions (on the issue's label):**
  - A popover's content-height spring is deferred to G-66. It is in the issue's proposed API but not its acceptance, and nothing changes an open popover's height today.
  - The height is snapped to device pixels. Without it, a ±1 px wobble showed in the widget under the section as the spring settled; measured before the fix.
- **Proof:** `tests/size_motion.rs`, 10 tests at 60 Hz.
  - **Opening:** the body rises and the button under it travels, one way, no frame over a third, to their unanimated places, within the settle time.
  - **Closing:** the body shrinks, then is `Display::None` and `Visibility::Hidden`.
  - **Turned round:** a toggle on the way turns it round without a jump.
  - **Split:** Enter slides the pane to its 100 px least, and the fraction is the new one at once.
  - **Key step:** a key step mid-slide lands at once.
  - **Reduced motion:** both a section and a split land at once.
  - **Double press:** a `DoublePress` on the divider slides the collapse.
  - **Same-value writes:** a game writing the same `SectionOpen` every frame lays nothing out.
  - **Tab stops:** a shrinking body has no Tab stop, and they come back when it turns round.
  - The API is new, so the file does not compile before this change. The 1 px wobble failed two tests before the pixel snap.
- **bevy_gooey gate:** fmt, clippy -D warnings, nextest 1133/1133, doctests 250, wasm32 check, rustdoc -D warnings, all green, run after the review fixes.

## Borrowed from Mixar
None (the reference is Rune: `README.md` "Animation", `rune-kit/src/shell/pane.rs:172,400`).

## Performance
Not measured. One query over gliding bodies and splits only while they move.

## Acceptance
`accept.py` was not run: no bevy_grimoire code changed.

## Review
An independent `reviewer` subagent ran. It found no P0 or P1. Every finding was fixed before the commit:
- **P2:** a same-value `SectionOpen` write restarted a glide every other frame. Only a state that differs from what is shown starts a glide now.
- **P2:** a drag started mid-slide jumped. It now takes the divider from where it is drawn.
- **P3:**
  - The target now includes the border and end padding, or the body's own `Px` height.
  - The clip is `clip_y`.
  - A shrinking body takes no pointer or Tab stop.
  - Tests were added for the double press, same-value writes and Tab stops, and the stale docs were fixed (`src/section.rs`, `docs/guide/sections.md`, the CHANGELOG, the research note).

## Test diffs
- bevy_gooey: the new `tests/size_motion.rs` and its `mod size_motion;` line.
- One line added to each of `tests/split.rs` and `tests/section.rs` (#76, above). No assertion changed.

## Release
bevy_gooey **v0.40.0**, https://crates.io/crates/bevy_gooey/0.40.0. Tag v0.40.0, commit d6fa6dd `release: v0.40.0`, and `cargo info bevy_gooey@0.40.0` resolves.
- **Version:** `cargo semver-checks check-release` failed 2 major checks, so MINOR. It ships G-43, G-45, G-46, G-50, G-52 and G-54.
- **Assumed decision:** the owner's steer "publish a new version after each issue is pushed" is read as releasing the crate the issue was built in. This repo's own crates did not change, so none was released.

## Next focus
G-44 (colour motion) under the same #76 decision, the last need of UI-R01 #331; then UI-R01 itself (move the bevy_gooey pin).
