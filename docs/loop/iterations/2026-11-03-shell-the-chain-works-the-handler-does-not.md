# 2026-11-03 shell: the input chain works live; the fault is gooey's divider handler

Focus: #224 UI-30 · Commits: 5390b40..35a5921 · Machine: big · Outcome: shipped

## Summary
Pass 283 fixed a real fault and the drag still did nothing, with two candidate causes and no way to tell
them apart. The picker's own counts settle it: **every step of the input chain works live**. The drag
runs, and its effect never reaches the split — which is inside `bevy_gooey`, filed there as #38.

## The measurement (35a5921)
`grimoire/pointer` now reports `pressing` and `dragging` (`PointerButtonState`'s own counts). Live:

```
start:        pressing 0  dragging 0
after cursor: pressing 0  dragging 0      cursor [170,400], over_ui true, target Image(..)
after press:  pressing 1  dragging 0
after move:   pressing 1  dragging 1      <- a drag is running
```

`SplitFraction` stays **0.22** through the moves and the release.

**So neither candidate fault was real.** The press arrives, the `Move` `inject_clicks` writes reaches
picking, and a drag begins. Five passes were chasing a chain that works.

## What the chain looks like now, all of it verified
| Step | State |
|---|---|
| cursor | `[170,400]` from the injected position |
| `view::Pointer` | same |
| picking target | `Image(..)` — equal to the UI camera's, which is what `bevy_ui` requires |
| divider | in the `UiStack`, `ComputedUiTargetCamera` resolved, **is** the hover target, 29 x 582 px |
| press | `pressing = 1` |
| move | `dragging = 1` |
| `SplitFraction` | **0.22, unchanged** |

## Filed upstream
`bevy-coven/bevy_gooey#38` (G-34, `consumer`, `consumer:bevy_grimoire`, `grimoire-ui`) — a split divider
that does not move under a drag picking reports as in progress, with all of the above as the evidence and
an acceptance test that reproduces it headlessly. The ask includes the most useful thing gooey could give
back: making `divider_on_drag`'s early returns observable, since right now a refusal is silent.

## Review
Reviewer not spawned; self-review, mine:
- **Could the measurement be wrong?** It reads `PointerButtonState` directly rather than inferring.
- **Is the ask the right repo?** Yes: the drag is gooey's observer, the values it needs resolve, and this
  crate now does not touch that path.
- **Is the changelog honest?** It says the chain works and the fault is in gooey's handler.
- **Did I weaken a check?** No: one test extended.
- **Anything left uncommitted or unreleased?** No: `bevy_cauldron` is `publish = false`.

## Test diffs
`the_pointer_payload_says_where_the_cursor_got_to.rs` gained one test (missing picker state is `null`,
a press with no drag is its own shape). No existing assertion changed.

## Release
None: `bevy_cauldron` is `publish = false`.

## Next focus
Await gooey#38; meanwhile UI-30's other work, or the next ready `ui` issue.
