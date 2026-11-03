# 2026-11-03 shell: the press was a frame early

Focus: #224 UI-30 · Commits: 53ce1b4..ccb3bb3 · Machine: big · Outcome: fixes only

## Summary
Pass 282 measured `[inject] ... presses=0` live and could not say why. The answer is a schedule, not a
guard: `bevy_fairy` writes `MouseButtonInput` in **`PreUpdate`**, and `inject_clicks` read the stream in
**`First`** — the schedule before it — so it found nothing and cleared it. Moved; the press is now seen
live. The drag still does not move the dock, and that gap is the next measured question.

## The fix (ccb3bb3)
`inject_clicks` was `First`, `.after(PickingSystems::Input)`. Its own comment explained the ordering in
terms of `PointerInput` being consumed by `ProcessInput` — true, and not the binding constraint: the
message it *reads* is written later than the schedule it ran in.

Now `PreUpdate`, `.after(InputSystems)` (where the fairy's own `apply_pending_input` sits) and
`.before(PickingSystems::ProcessInput)`.

**Why the fixture never caught it:** a headless test writes `MouseButtonInput` itself, before `First`
runs. The live editor's press is written by the fairy in `PreUpdate`. The fixture's press was visible and
the live one never was — the whole class of fault this pass has been chasing.

**Measured on a running editor:** before `presses=0` across 86 frames; after,
`[probe] SAW PRESS Pressed`.

## What remains, measured
The drag still leaves `SplitFraction` at 0.22 live. Two candidate faults, both now readable in a single
request thanks to pass 282's `grimoire/pointer`:
- does the `Move` this system writes reach `PointerState.dragging`?
- is the divider what the press landed on (the `HoverMap` says it is, headlessly)?

Stopped after two live attempts, per the rule, rather than guessing a third.

## Review
Reviewer not spawned; self-review, mine:
- **Is the fix a guess?** No: the fairy's own `apply_pending_input` is registered in `PreUpdate` with a
  doc comment saying so, and the probe shows the behaviour change.
- **Could the new ordering break picking?** `ProcessInput` still runs after it, so `PointerInput` is
  consumed the same frame it is written; 1169 tests pass.
- **Was the probe left in?** No: `grep probe surface.rs` is empty and the diff is 11 lines.
- **Is the changelog honest?** It says the press is seen, not that the drag works.
- **Any protected file touched?** No.

## Test diffs
None: the change is a schedule and the existing suite exercises both schedules. Pass 282's
`grimoire/pointer` tests still pin the payload.

## Release
None: `bevy_cauldron` is `publish = false`.

## Next focus
Whether `inject_clicks`'s `Move` reaches `PointerState.dragging` live — one `world.query` away.
