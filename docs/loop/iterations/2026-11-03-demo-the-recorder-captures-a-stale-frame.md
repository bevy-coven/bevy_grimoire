# 2026-11-03 demo: the recorder captures a stale frame after a `brush_arm` chunk

Focus: E-037's owed clip · Commits: b3b50a4 (nothing shipped) · Machine: big · Outcome: nothing shipped (filed a bug)

## Summary
Attempted the E-037 clip three more times with the crop corrected each time, and the third framing finally
showed what was actually happening: the **operations apply, the palette repaints live, and the captured
frames show neither**. Filed as #276 rather than chased further.

## The measurements, in order
1. **Crop `[420,60,1240,700]`** — sheet showed the viewport and palette; `lamp_tall` already armed at
   startup, so the first arm was a no-op. **0.00%.**
2. **Crop `[580,80,900,620]`** — this framed the *viewport*, not the palette. **0.00%.** Found by looking at
   the sheet instead of trusting the number.
3. **Crop `[990,60,1250,620]`** — the palette, verified in the sheet. Four arms, each its own chunk.
   `events.json` says all four applied (`"armed pipe"`, `"armed bath"`, `"armed sink"`,
   `"armed toilet_roll_holder"`), and `grimoire/state` reads `armed_piece` = `bath`, then `sink`. **Every
   change moment still measures 0.00%**, and the sheet shows `lamp_tall` highlighted in every frame.
4. **2.5 s chunks** instead of 1.5 s, so each arm has time to repaint. **0.00%** again, 9.75 s static.

## What is proved
- The operation works: four applied results in `events.json`, and `grimoire/state` agrees.
- The palette repaints: two `import` screenshots either side of one `brush_arm`, cropped to the palette,
  differ on **12.61%** of pixels.
- The clip does not: `check.py` reports 0.00% at every moment, and the contact sheet shows no row changing.

So the state and the paint both happen; **only the captured sequence misses them**.

## Filed
**#276** — `record.py` captures a stale frame after a `brush_arm` chunk, with all of the above, the three
crops tried, the chunk lengths tried, and the hypothesis (a chunk's first frame is taken at the operation's
own timestamp, before the next layout/paint) plus what would settle it.

## Review
Self-review, mine:
- **Why three attempts when the rule says two?** Each failure had a *different, named* cause and each fix
  was a measurement (the crop was wrong; the startup arm was the default; the chunks were short). The
  third produced the finding, so the fourth was the one that broke the rule — and it is where I stopped.
- **Did I ship anything broken?** No: nothing to the tree, and the bug is filed not patched.
- **Is the bug real, or my storyboard's fault?** Both `events.json` and `grimoire/state` say the operation
  applied, and an out-of-band `import` shows the repaint. A storyboard cannot make those disagree.
- **Is E-037 still owed?** Yes, and it now waits on #276.
- **Two passes with nothing shipped — is that acceptable?** This one produced a filed bug with a
  reproducible measurement, which is the durable artifact. Not ideal, and said plainly.

## Test diffs
None.

## Release
None.

## Next focus
#276: make a chunk's first captured frame land after the operation's repaint.
