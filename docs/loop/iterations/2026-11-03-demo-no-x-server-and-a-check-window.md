# 2026-11-03 demo: no X server, and a check window too narrow

Focus: #276 / E-037's clip · Commits: 1b6c421 (nothing shipped) · Machine: big · Outcome: nothing shipped

## Summary
Attempted #276's fix. The first finding is that **the display was dead** — every `0.00%` measurement in
pass 287 and the first two here ran with no X server, so the frames were genuinely identical. The second
is that even on a working display the change exists in the frames and `check.py` does not see it at the
moment it samples.

## The dead display
```
Failed to build event loop: Os(OsError { ... XNotSupported(XOpenDisplayFailed) })
pgrep Xvfb   -> (nothing)
/tmp/.X11-unix/  -> X0          (no :99)
```
The unit's `:99` had gone away; the editor exited before rendering, so every capture was of a surface that
was never painted. **That is my error to have missed**: an editor that fails to open is not a clip.

With an Xvfb started by hand, the same storyboard's frames differ from frame 0 at frame 26 onward, across
`bbox (29, 270, 781, 656)`. **"60 identical frames" was never a recorder bug.**

## What is still real
On the live display, `check.py` still reports `0.00%` at the `brush_arm` change moment, with the crop set
to that bbox and again with a palette crop. So the change is in the frames and the check does not sample
it.

Two hypotheses, cheapest first:
1. **The repaint lands later than `CHANGE_WINDOW_S`.** `operation_applied_t_s` is `0.158` into its chunk
   and the check compares the last frame before it against frames up to `+0.2 s`; at `fps: 12` that window
   holds two frames. A paint two frames after the apply falls outside it.
2. **The change is under 5% of any crop that contains it.** The bbox is 752 x 386 = 290k pixels; a ghost's
   outline plus one row's tint may be a few thousand.

## Reverted
A `OPERATION_WAITS_FOR_FRAMES = 2` change in `sequencer.rs`, which made the operation wait two frames
instead of one. **Unproven and reverted** — the dead display made its effect unmeasurable, and shipping a
schedule change on a measurement taken against no display would have been exactly the mistake this pass
found.

## Review
Self-review, mine:
- **What should have caught the dead display sooner?** The editor's own log says
  `XOpenDisplayFailed` in plain text. Checking it after the *first* `0.00%` would have saved two passes.
  Added to the next focus.
- **Is #276 mis-titled?** Partly: it says "the recorder captures a stale frame", and the first half was no
  display. Corrected on the issue rather than edited away.
- **Did I ship the sequencer change?** No: reverted, and the tree is clean.
- **Is E-037's clip closer?** Yes — it is now a `check.py` window question, not a mystery.

## Test diffs
None.

## Release
None.

## Next focus
`fps: 4` on the same storyboard, to test whether the window is the problem — one recording, one answer.
