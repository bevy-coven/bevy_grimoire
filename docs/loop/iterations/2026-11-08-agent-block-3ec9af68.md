# Pass 7 (mac_air clone), 2026-11-08: the AGENT block, and the eight standing GIFs retaken

Focus: the editor's UI on bevy_gooey · Commits: 78a32db9..3ec9af68 and the docs and release that follow · Machine: mac_air edits, big builds and tests · Outcome: shipped

Log number 480 (pass 7 of this clone; bevy_emerge_loop's passes are numbered in the same sequence).

## The cluster

| item | state |
|---|---|
| #246 UI-60 the Familiar | **Closed with this pass's last commit.** View > Show agent: an AGENT block at the top of the Map door's INSPECTOR. HEARD (the call the editor last got from an agent and when), NEWEST (whose the newest change on the undo stack is, and what it was), HAND-BACK (what `grimoire_present` handed back, waiting or how it was answered), an Undo button that presses Undo's own chord and is greyed unless the newest change is the agent's, and a pause switch that is the View row's twin. Both acceptance items are met (attributed with Undo; the pause refuses the next write with a Problem) and the clip shows an agent edit attributed, then undone. Clip `2026-11-08-ui-60-agent-block`. |
| the agent's plan, and the hand-back inline with Accept, Undo and Dismiss | **Filed as its own issue** (the editor has no plan an agent declares; the issue recommends a `plan` field on `grimoire/present`). |
| owed: `place-and-undo` | **Retaken, passes `check.py`, after its framing changed.** On this build it failed four takes in a row at the same 3.00 % (need 5 %). The check measures the placed sofa against the frame before the click inside 0.2 s, and the sofa is a small part of the scene; the same storyboard passed on take 1 last pass and failed all eight takes in pass 5, so the pass or fail follows the machine's speed, not the editor. The storyboard now frames the corner a little tighter (`[1.75, 0.75, 2.65, 1.65]` for `[1.4, 0.4, 3.0, 2.0]`): 5.66 % at the place, 11.5 % at the undo, every other check unchanged. No threshold was touched. |
| the other seven standing GIFs and `editor.png` | Retaken on the build with the AGENT block; `review-labels` needed take 2, the rest passed on take 1; every sheet looked at. |
| #337 the update freeze | Not verifiable here: it stays open until an update from 0.4.4 or later has run on a person's Mac, and an app that updates itself is the person's to run. `an_update_swaps_even_when_the_old_app_will_not_leave` and `a_quit_that_does_not_finish_is_ended` hold the two halves. |
| #237, #238, #239, #240 to #245, #247, #251, #254, #257, #258, #260 | Not taken: each waits on #210 (the owner's word on two protected tests), on gooey, or is a rebuild larger than a slice. The comments from earlier passes stand. |

## What this pass learned

1. **A headless layout is not the drawn one.** The first block used gooey's `label` for its lines. In the headless harness the layout test passed (the lines do not overlap); in a capture of a real editor the wrapped line was measured one line short and the next was drawn over it. Two guesses (a definite width, `flex_shrink: 0`) changed nothing; the fix was to build the lines as the INSPECTOR's own rows (`row_label`, a column with `min_width: 0`, `row_value`), which have been drawn at this panel's width since the readout began. The layout test stays, as the regression for the part it can see.
2. **A `SystemParam` bundle must not read what the system holds mutably.** `Gates` reads the confirm modal; `editor_present` holds it with `ResMut`, so putting `Gates` in it would panic at run time. It takes the two parameters it needs itself.
3. **`check.py`'s change window is a race against the machine's speed, not a measure of the editor.** The fix that is legitimate is a storyboard that makes the thing it shows larger, never a looser threshold.

## Evidence

- `the_agent_block_says_who_changed_the_map`: 5 tests (the row and the block; heard, newest and undo, including that the button never undoes the person's own edit; the switch as the View row's twin; the hand-back waiting and then answered; no line drawn over another). `the_agent_can_be_paused`: 2.
- Gate: see the log line.
- Every clip and contact sheet looked at: the AGENT block clip (four recordings, three of them found the layout fault), the eight GIFs, the tighter `place-and-undo`, `editor.png`.

## Release

0.5.1 (`version.py next`), tagged after the gate.

## Not done, said plainly

- The agent's plan and the inline hand-back (filed). #337's last step is a person's.
- #210 UI-02 waits on the owner's word about two protected tests; #336 UR06 waits on gooey #69, #70 and #71.

## Cleanup

Worktree `~/p7-wt` and `/tmp/p7_*` were this pass's and are removed; the `p7-*` branches in the loop clone, and the local `p7dbg` and `p6dbg` branches here, are listed for the owner.
