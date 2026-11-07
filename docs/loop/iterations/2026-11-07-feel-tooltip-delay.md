# 2026-11-07 feel series: G-57 tooltips on a desktop's timing built in bevy_gooey, released as v0.41.0

Focus: epic #330 (owner's steer) · Commits: bevy_gooey 760405b, release 460b081 · Machine: big · Outcome: shipped (in bevy_gooey)

## Summary
G-57 (bevy-coven/bevy_gooey#64) was blocked on the protected `tests/tooltip.rs`. The loop applied the #76 decision, which was its own recommendation, as an assumed exception. A tooltip now opens only after the pointer has rested half a second. UI-R03 #333 still needs G-58 and G-66; UI-R04 #334 now needs G-53 and G-58.

## Baseline
bevy_grimoire was not touched, so its gate and `accept.py` did not run.

## Enhancement: G-57
- **Change:** `TooltipSettings { delay_seconds: 0.5 }` and `TooltipDwell` on each box, counted on `Time<Real>`, in `src/tooltip.rs`.
  - **Ends a visit:** a press on the widget or anything in it, a wheel turn, or a key. The box then stays shut until the pointer leaves and returns.
  - **Focus:** visible focus opens it at once.
  - **Optional readers:** the wheel and key readers are optional, because a style-only app has no input messages.
  - **Plugin:** the settings and the press observer live in `GooeyStylePlugin`, beside `show_tooltips`.
- **Assumed decisions (on the issue):**
  - `tests/tooltip.rs`'s `app()` inserts `TooltipSettings { delay_seconds: 0.0 }`. No assertion changed.
  - The presence fade is not built.
- **Proof:** `tests/tooltip_delay.rs`, 3 tests on a 60 Hz clock.
  - **Timing:** not shown at 0.4 s or just under 0.5 s, shown at 0.5 s and a frame.
  - **Ending a visit:** a press, a wheel turn and a key each end it, and it stays ended until the pointer returns.
  - **Focus:** Tab with the pointer resting opens the box in that frame.
  - **Zero delay:** the box opens on arrival.
  - The resource did not exist before this change.
- **bevy_gooey gate:** fmt, clippy -D warnings, nextest 1161/1161, doctests 256, wasm32 check, rustdoc -D warnings.

## Borrowed from Mixar
None. The reference is Rune's README, "Help tooltips".

## Performance
Not measured.

## Acceptance
`accept.py` not run: no bevy_grimoire code changed.

## Review
An independent `reviewer` ran: 1 P2 and 3 P3, all fixed.
- **P2:** the press observer and the settings were registered in the navigation plugin, while `show_tooltips` is in the style plugin. A style-only app would never hide on a press. Both are in the style plugin now.
- **P3:**
  - The press observer walked the parent chain at every bubbling hop. It acts on the first hop only.
  - The Tab test sent no key. It sends one now, with the pointer resting.
  - The "0.5 s" check actually ran at 0.52 s. It now asserts closed just under 0.5 s and open just over.

The fixes were not re-reviewed; the gate and 3 tests cover them.

## Test diffs
New `tests/tooltip_delay.rs` and its `mod` line in `tests/all.rs`. One line added to `tests/tooltip.rs` (assumed owner exception #76); no assertion changed.

## Release
bevy_gooey **v0.41.0**, https://crates.io/crates/bevy_gooey/0.41.0 (tag v0.41.0, commit 460b081). MINOR: `cargo semver-checks` flagged the system function `show_tooltips` taking 11 parameters instead of 7.

## Next focus
G-58 scroll glide (#65) and G-53 hover groups (#60), then the pin move for UI-R03 and UI-R04 (the editor's tooltip tests need delay 0).
