# 2026-11-03 snip: Ctrl+\ shows a receipt, not a false refusal — and five demos re-recorded

Focus: bug fix (#224's demo blocker) · Commits: fcfa3d0..(this) · Machine: big · Outcome: fixes only

## Summary
`Ctrl+\` armed the snip and then painted a red *"the operation could not be logged, so nothing was
changed: no project is open"* over the editor on **every** press. The tool was armed and nothing
needed writing, so the sentence was false in both halves — and it covered the demo's frame from the
key to the end. Fixed at the one place that lacked a project. Five of the nine standing demos are
re-recorded on the fixed build; four are owed.

## Bugs fixed
### Ctrl+\\'s false refusal
Must-fix: blocks y (it covered the hero demo on every frame after the key), common y (every press of
a documented key), no workaround n (the toast never clears by itself) — the 2026-11-03 hero sheet.
Root cause: `snip::snip_key` built its `Ctx` with `..Ctx::default()` — no `project`, no `root` — and
`ops::dispatch` appends the operation's log line. The append found nowhere to write, `take_back`
reported a refusal, and the status painted it. Every other key path that dispatches an operation
carries the project (`environment_key`, `set_yaw`, `set_tip`).
Regression check: `an_operation_that_writes_nothing_needs_no_log::the_snip_key_arms_and_its_status_is_a_receipt_not_a_refusal`
— pressed the chord as a person does and read `EditorState::status`; reports the exact banner without
the fix, no problem with it. Full editor suite 1161 passed; clippy clean.

**A first attempt was reverted**: skipping the log append for operations declaring `writes: &[]`.
`brush_arm_is_an_operation::each_applied_arm_writes_exactly_one_log_line` pins that arming a piece
**is** logged, and 8 existing tests agreed with it. The append is right; the missing project was the bug.

## Demos re-recorded (5 of 9)
`hero`, `place-and-undo`, `move-the-camera`, `frame-the-map`, `editor.png` — each on its own fresh
project and fresh editor, `check.py` PASS on the four GIFs, sheet looked at for each, and the
`Captured` column rewritten to `19598ac`.

**Storyboard changes, each with the measurement that drove it:**
- `hero`: crop `[440,64,980,690]` → `[500,170,940,560]`. The old crop's lower half was empty floor, so
  the snip box was 3.74% of the scene against `check.py`'s 5% floor; the tighter crop puts the box and
  the note field in frame and reads PASS. The banner's absence in the new sheet is the fix above.
- `place_and_undo`: the place click `[800,494]` → `[496,332]`, the viewport's own centre. The old
  coordinate was outside the viewport, so the sofa was placed off-camera and the frame barely changed
  (0.37%).

## Owed demos
`agent-drives`, `agent-removes-then-undo`, `build-a-map`, `review-labels`. Each was attempted and
stopped under the rule:
- `build-a-map`: 1.47% → 2.14% (crop) → 4.27% (framing) → 4.21% (tighter). Plateaus at ~4.2% against
  the 5% floor; the piece is small in the frame at any framing the room allows. Reverted the
  storyboard, left the previous GIF.
- `review-labels`: static run 1.60 s against 1.5 s; adding a tail step made it 2.60 s (the arrow does
  not pan in that door). Reverted.
- `agent-drives`, `agent-removes-then-undo`: same small-change class; not reached inside the budget.

## Review
Reviewer not spawned; self-review, mine:
- **Is the fix in the right place?** Yes — the append is shared and tested; only the one key path
  lacked a project. The reverted attempt is recorded above with the test that refuted it.
- **Does the test assert what a person sees?** It reads `EditorState::status`, which is the toast's own
  source, not an internal flag.
- **Could the crop change hide a regression?** `check.py` is protected and unmodified; the crop only
  changed which pixels the demo shows, and the sheets were looked at.
- **Is any recorded GIF stale in a way the caption misstates?** The four owed GIFs keep their old
  captions and old `Captured` dates, so nothing claims to show this build.
- **What is the honest state of #224?** Acceptance item 1 met (labels, dock floor); item 2's clip
  blocked on the injected drag, recorded on the issue in the previous pass.

## Test diffs
One new test file and its one `mod` line in `all.rs` (owner exception for a new editor test file).
No existing assertion changed; the storyboard edits are demo data, not tests.

## Release
None: no packaged crate changed.

## Next focus
The owed demos, `build-a-map`'s small-change floor first — a storyboard that places a larger piece is
the fix, not a looser check.
