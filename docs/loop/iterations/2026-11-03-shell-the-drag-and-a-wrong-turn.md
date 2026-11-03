# 2026-11-03 shell: the drag works in a fixture, and pass 280's change was backwards

Focus: #224 UI-30, the drag · Commits: 687c94b..b8fb941 · Machine: big · Outcome: fixes only

## Summary
Two things came apart this pass, both measured. Pass 280's pointer change was **backwards** — the UI
camera renders to the image, so pointing the pointer at the window made the two disagree — and it is
reverted. The other half of the drag, a missing `Move`, is fixed: in a fixture an injected press and drag
now takes `SplitFraction` 0.22 → **0.349**. In the live editor it still does nothing, and that gap is
measured too.

## 1. Pass 280 was backwards (1c6f0c0)
`bevy_ui`'s picking backend keeps a pointer only where `render_target == pointer_location.target`
(`bevy_ui-0.20.0/src/picking_backend.rs:129`), and this editor's interface camera renders to the **same
surface image** as the map. Measured before reverting:

```
target_camera = Some(ComputedUiTargetCamera { camera: Some(403v0) })
divider Hovered? false
ptr target=Window(NormalizedWindowRef(658v0))
```

The divider resolved a camera and the pointer named the window, so they could never match. Pass 280 also
deleted the test that could have caught it, replacing it with one asserting the wrong invariant. The test
now compares the pointer's target against **the divider's own UI camera's** target.

**A second lesson in the same failure:** `Hovered` was the wrong signal to measure at all. `bevy_picking`
only *updates* `Hovered` on entities that already carry it (`hover.rs:481`), and the frame's furniture
carries none by design — so `Hovered` on a divider is impossible and its absence says nothing. The right
signal is `HoverMap`, and the divider **is** in it.

## 2. The missing move (b8fb941)
After the press, `PointerState` read **`pressing = 1, dragging = 0`**. `bevy_picking` makes a drag out of
`PointerAction::Move` with a non-zero delta (`bevy_picking-0.20.0/src/events.rs:1167`), and `Move` comes
only from `WindowEvent::CursorMoved`, which an injected cursor never produces. `inject_clicks` now writes
it on a real move.

Fixture result: **`pressing/dragging = (1, 1)`, `SplitFraction` 0.22 → 0.349.**
`tests/an_injected_drag_moves_the_dock_divider.rs`; **not vacuous** — removing the move fails it with
exactly *"with `pressing = 1, dragging = 0` no drag can start, which is what four passes measured"*.

## 3. What is still owed, measured
The live clip does not work: after a recorded drag on the running editor, `SplitFraction` stays 0.22, and
the live pointer's `PointerLocation` is **empty** (`{}`), where the fixture's holds a location. That is
the next question, and it is a different one from anything above. Neither `view::Pointer` nor
`DebugCursor` is exposed to BRP reflection, so this pass could not read them live — a limit of the
measurement, not evidence about them.

Stopped after one live attempt, per the rule, rather than guessing.

## Review
Reviewer not spawned; self-review, mine:
- **Did I catch my own last-pass error?** Yes — by measuring the UI camera's own target instead of
  reasoning about what a "UI picker" sounds like.
- **Is the revert complete?** `git diff` shows the pass-280 code gone; 1167 tests pass.
- **Is the drag fix load-bearing?** Removing it fails with the exact pre-fix measurement.
- **Is the changelog honest?** It says the live clip is owed and why.
- **Anything deleted that mattered?** No: one test was rewritten to assert the true invariant.

## Test diffs
`an_injected_pointer_over_a_dock_is_pickable.rs` rewritten (its old assertion pinned pass 280's wrong
behaviour; the new one is stronger — it compares against the UI camera itself). One new test file.

## Release
None: no packaged crate changed.

## Next focus
Why the live pointer has no `PointerLocation` — start by exposing it, or by reading the surface's own
state through a system rather than BRP.
