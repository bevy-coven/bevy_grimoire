# 2026-11-03 shell: an injected drag still cannot move the divider — three attempts, then stop

Focus: shell UI-30 (#224) · Commits: afcbb05..(this) · Machine: big · Outcome: nothing shipped

## Summary
Tried the one item left on #224: make an injected cursor able to drag the divider, so the
acceptance item's clip ("the shell with its docks resized") can be recorded. Three attempts failed;
per the stop rule I reverted the unverified change and shipped no code. Two things were measured and
are worth the next pass's time.

## Baseline
Gate: green at afcbb05. `accept.py`: not run (no display work produced a claim). Load 12.6, free disk
149 GB, available RAM 12 GB.

## What was measured, and what it rules out

**1. The drag needs `PointerAction::Move`, and `inject_clicks` never writes one.** Established from the
source, not guessed: `bevy_picking-0.20.0/src/events.rs:1167` builds `PointerDragStart`/`PointerDrag`
out of `Move` **with a non-zero delta**, and `Move` is emitted only by `mouse_pick_events` from
`WindowEvent::CursorMoved` — which an injected cursor never produces. `surface::inject_clicks` writes
`Press` and `Release` only.

**2. A `Move`-emitting `inject_clicks` was written and did not fix it.** Reverted, because it did not
turn the failing test green: a test pressing the divider's own centre (x = 218.5, measured over BRP;
the viewport's left edge is 233, so a press aimed there misses the 29-px channel by 14 px) still read
`SplitFraction` 0.22 -> 0.22. Something earlier in the chain does not register the press as a
press-and-hold, and three attempts at finding it failed. That is where the next pass starts, and the
measurement to make first is `PointerState::pressing` after the press, not the fraction after the drag.

**3. The editor is healthy, despite what a recording looks like.** Three hero recordings showed a red
toast — *"the operation could not be logged, so nothing was changed: no project is open"* — over every
frame. It is **not** the recording failing: `grimoire/oplog_read` holds `placement_remove` entries
stamped inside each run (07:21:07, 07:22:28, 07:24:39), so the agent's operations applied and logged.
The banner is a stale toast. Two hypotheses of mine were **wrong** and are recorded so nobody repeats
them: `grimoire/operation_apply` defaults `dry_run: true` **by design** (the raw BRP door previews;
the MCP tool applies), and `tool_arm` refuses over BRP on `Ctx::view` before it ever reaches a log.

**4. Consequence for the demos.** `tools/demo/check.py` fails hero on its `place` moment, 2.91-3.28% of
pixels against a 5% floor, because the snip box covers a small part of the frame. The check is
protected, so the fix is a storyboard or renderer change (a crop that includes the box and the note
field, or a larger drag), not a looser threshold.

## Test diffs
None left in the tree. The failing-first test `an_injected_cursor_can_drag.rs` was written, run, and
removed with the reverted fix; it is quoted above so the next pass can restore it from this report.

## Review
Reviewer not spawned; self-review, mine:
- **Did I weaken anything?** No: the only change was reverted and its test deleted, so the tree is at
  afcbb05 for every crate.
- **Is the "Move is missing" claim checked?** Yes, by reading the emission site in the pinned bevy, and
  by the failing test before and after the attempted fix.
- **Could the banner have been the demo's own failure?** Checked against the oplog, which has entries
  stamped inside each run: no.
- **Is stopping right?** Yes, by the stop rule: two failures at the same step end it, and a third
  attempt at a guess costs the next pass more than it saves.
- **What should the next pass do first?** Read `PointerState` after an injected press, which is the
  one measurement this pass did not take.

## Release
None: no packaged crate changed (nothing was committed to a crate).

## Next focus
`PointerState::pressing` after an injected press, then the Move emission — in that order.
