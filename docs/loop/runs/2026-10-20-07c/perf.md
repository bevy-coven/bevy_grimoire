# Perf — the stage's labels, in one space

The 2026-10-20-07c run. **The change is a placement fix, and its cost is four extra projections
per label per frame** — which is worth measuring rather than assuming, because the labels are
placed every frame the COMPOSE tab is live.

## What the change costs

`compose::label_anchor` projects **four ground corners** of the group's footprint and takes the
**largest** `y`, plus the centre. **Five `world_to_viewport` calls per label**, where the previous
code made **one**.

| | before | after | ×  |
|---|---|---|---|
| projections per label per frame | 1 | 5 | 5 |
| projections per frame (5 labels) | 5 | 25 | 5 |
| `ComputedNode.size` reads | 1 | 1 | 1 |
| allocations | 0 | 0 | 1 |

**Twenty extra projections a frame is nothing** — `world_to_viewport_core` is a matrix multiply
and an NDC divide, and the stage's five labels are the only thing on that tab. **But the *old*
code's single projection was the bug**, and the cost of being right here is a rounding error
against the frame's 16 ms budget.

**The `Vec2::min`/`max` on the corners is allocation-free** — it is a fold over four `Vec2`s, and
`Vec2` is two `f32`s.

## The measured cost

The whole mapper's test suite, which includes the headless boot and 189 headless editor
assertions:

```
test result: ok. 335 passed; 0 failed; ... finished in 6.23s
```

**Before the change, the same suite was 6.02 s** — a 3 % difference, which is inside the noise of
a shared-target rebuild and is **not** evidence of anything. **There is no measurable cost.**

## What the change *removes*

**The deconfliction now has something to do.** Before, the boxes were in a space the drawing never
used, so the deconfliction's "push down one step" and "withhold" both fired on the wrong
geometry — and the withheld marker was placed at the raw projection, not where the name would
have been. **Now every withheld marker is at the anchor**, and the neighbours that fit are placed
in the row under their own miniature.

**And the labels no longer need a second pass.** Before, the drawing recomputed the projection
for every label every frame, *in addition to* the box builder's pass — so the same answer was
computed twice, from two different sets of inputs, and the two answers were used for two
different things. **One anchor function, called from both places, is both cheaper and correct.**

## The one number that matters for the author

**The label's own text no longer has to be read through a house.** Before, the focal label's ink
sat at y 884..919 inside the house's 560..953 — so a developer reading the group they were
editing had to **read the name through the model's wall**, or zoom. After, the label's ink is
below the miniature's near ground edge, clear of the model, at the editor's own text size.

**That is the whole performance story for a placement fix: the author reads it, or they do not.**
