# Review — the stage's labels, in one space

The 2026-10-20-07c run. **The feature: the COMPOSE tab's stage labels stop overlapping, stop
being drawn through their own miniatures, and stop being placed in a space that is not the one
they are drawn in.**

## What the defect was, in one sentence

**The labels' collision test ran in one space and the labels were drawn in another**, so the test
passed while the screen overlapped — and the two spaces were not even the two the code claimed
(`world_to_viewport` is *physical* pixels; `Val::Px` is *logical*; the node's size is *physical*;
and a viewport's `y` grows **downwards**, which the "nearest corner" arithmetic got backwards).

## The measurement that settled it

Every number below was read out of a live editor at `UiScale(2.4)` on a 2560×1406 window, with
Sanctuary's `town` kit and the COMPOSE tab open, focal group `town/house_cottage_plaster`
(26 characters).

| | space | measured |
|---|---|---|
| `world_to_viewport` | **physical** px, relative to the camera's viewport rect (`970..2560 × 146..1344`) | `(1765.0, 878.59)` |
| `ComputedNode.size` | **physical** px | `412 × 32` |
| `UiGlobalTransform.translation` | **physical** px, the node's **centre** | `(1885.0, 899.0)` |
| `Val::Px(x)` | **logical** px — Bevy multiplies it by `UiScale` on the way to physical | — |

**The check that the numbers have to pass:** `Val::Px(699.58) × 2.4 = 1679`, and
`1885 − 1679 = 206 = 412 / 2`. The node's global centre, the written `Val::Px` and the node's own
width agree — **so the scale is applied exactly once, on the way in**, and the box is in logical
pixels.

**And the advance is right, which was worth checking:** the focal node's 412 physical px over 26
characters is **6.60** logical px per glyph, against `BODY_CHAR_W`'s **6.6**; the neighbour's 312
over 24 is **5.42**, against `6.6 × 9/11 = 5.4`. **The constant was never wrong; the space was.**

## Pass 1 — Function

**What was wrong before.** Measured on a real capture of the same project: the labels' ink ran
**x 1653..2466 — 813 px** with no clean boundary where three labels should be, and **918 px**
where four should be. At 2.5× the focal group's name was drawn *through* its neighbour's. Worse,
the focal label was drawn **across its own house's wall** — its ink at y 884..919, inside the
house's 560..953.

**The fix, in three parts, each measured:**

1. **One space.** `compose::label_box` builds the box in logical pixels from the projection
   (physical) and the node's size (physical), and the drawing writes it into the node
   **unchanged** — because `Val::Px` *is* logical pixels. The scale is applied once.
2. **One anchor.** `compose::label_anchor` answers "where does this group's label go", and **both**
   the box builder and the drawing call it — so the collision test and the drawn label cannot
   disagree about *where* the label is, only about whether it collides.
3. **The right point.** The label goes under the footprint's **near ground edge**, not the group's
   origin — the camera looks down at the strip, so the near edge is **lower** on the screen
   (953) than the centre (874). **And "lower" is the *larger* `y`**: a viewport's `y` grows
   downwards (`bevy_camera`'s `world_to_viewport_core` flips the NDC's `y` for exactly this
   reason), so the first version of this took the *smallest* `y` and put the label at the
   footprint's **far** edge — behind the miniature, and therefore inside it.

**Verdict: NOT YET** — the labels no longer overlap and no longer cross their houses, but the
focal label sat at the *left* of its house, because the anchor took the minimum in **both** axes
and so inherited the near-**left** corner's `x`.

**Verdict: GREAT** — after the anchor's `x` was pinned to the group's centre and only its `y` to
the near edge. The capture shows the amber focal label centred under the big house, the second
label under the middle house, and the third **withheld** — the `…` marker, the deconfliction
working exactly as Bell et al. (2001) prescribe.

## Pass 2 — Zoom

Cropped and upscaled 2× on the label band (`review/evidence/crops/`).

**What is right:** the labels are clear of the miniatures, so no glyph is lost against a wall.
The focal label is amber (`ACCENT`) and the neighbours are `DIM` grey, so the eye lands on the
group being edited first — the hierarchy the `chrome` tokens are for. The withheld neighbour is a
bare `…` in the same grey, so it reads as *"there is a group here and its name is one action
away"*, not as a rendering fault.

**What was wrong:** the focal label's ink was **drawn over the house's own wall** before the
anchor fix, and the *second* label's text ran into the *third* house's roofline. Both were the
same defect (the anchor was the group's origin rather than its near edge) and both are gone.

## Pass 3 — Fresh eyes

Re-captured from a cold start: launch the editor on Sanctuary's project, open the Kit door, click
the strip, press `3` for COMPOSE, and look.

**Can a developer discover it?** Yes — the labels are where a label belongs, under the thing they
name, and they are readable without a zoom. **Can they undo it?** Nothing to undo: this is a fix,
not a verb. **Do they trust it?** The withheld neighbour is the trust test, and it passes: the
author can see that there are three groups, that the third is unnamed, and that `O`/`P` will name
it.

**One thing the fresh-eyes pass found, and it is not a defect:** the *third* house is very small on
screen (it is a `MINIATURE`-scaled neighbour) and its `…` marker sits well to the right of it.
The marker is the elided **name**, placed where the name would be — which is the rule — and the
name is 24 characters wide, so the marker is wide too, and a wide marker under a small miniature
reads as belonging to nothing. **Logged as the next item**, not fixed here: the fix is a leader
line, which is a feature, not a bug fix.

## Evidence

- `evidence/before/` — the overlap, the label through the wall.
- `evidence/after/01-the-anchor-at-the-left.png` — the first fixed capture, before the anchor's `x`
  was pinned to the group's centre: the labels no longer overlapped each other, but the focal
  label sat at the **left** of its house.
- `evidence/after/02-labels-fixed.png` — the focal label centred under the big house, the second
  under the middle house, and the third **withheld** — a bare `…`, the deconfliction working as
  Bell et al. (2001) prescribe.
- `evidence/after/03-the-carousel-stepped.png` — **the strongest evidence, and the worst case.**
  The carousel stepped to `town/house_hall_wood`: the focal label is amber and centred under the
  big house, the two left neighbours stack on one row (`town/house_hall_plaster` above
  `town/house_hall_brick`), the right neighbour `town/pub_extras` is under its own tiny prop, and
  **no two labels overlap anywhere**. **That is the deconfliction doing its job on a strip where
  every name is 21 to 24 characters long** — the exact case that produced 813 px of continuous ink
  before.
- `evidence/crops/` — the label band, 2×.

## The tests

- `the_stage_labels_live_in_one_space` — **the conversion, measured against a known value.** Four
  tests: the box's width is the node's own, the box's left is the anchor over the scale less half
  the width, the drawn position is the group's centre (the scale applied **exactly once**), the
  box sits below its group, a zero-sized node has no box, and a zero scale does not divide by
  zero.
- `the_stage_labels_are_on_screen` — the labels do not overlap, **measured in the real layout**
  (the headless harness *does* spawn the camera, so this measures real positions), and there is
  **one** box builder and **one** anchor.
- `the_stage_labels_do_not_collide` — the deconfliction's rules: the focal first, the neighbours
  by distance, one step down, or withheld.
