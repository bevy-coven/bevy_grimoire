# Research — the run of a thing: a piece repeated along a line

Run: 2026-10-20-02. Feature: **an author draws a run of posts — a fence, a wall, a kerb — with one
gesture, at a pitch of their choosing, with the corners handled.**

## The problem, in one sentence

A fence is not 282 pieces. It is **one piece, a pitch, a direction** — and the editor has no way
to say that.

## The two measurements, and why this is the biggest usability win

The brief asks for usability, in five words: *a developer discovers it without help, does the task
in fewer steps, understands what the editor is telling them, recovers from mistakes, and trusts the
result.* Measured on Sanctuary's own project, as a first-time user, the two sharpest numbers in
the editor are both here:

| | Before | After |
|---|---|---|
| **Find one piece** in a 211-descriptor palette | **~90 keystrokes** of `Shift+Down`, and the walk *wraps* into the wrong kit | **2 keystrokes** — `Cmd+F`, then the name |
| **Draw a fence of N posts** | **N placements**, one click each, at a pitch the editor's lattice cannot express | **1 drag** |

And the sharpest *trust* number, which is why this is the top item and not merely a good one:

| | Before | After |
|---|---|---|
| **Reach the fence piece at all** | The Map tab's palette filter was **mouse-only** — `FocusFilter` was `Context::Tiles`. A click that missed the box **fell through to the map and replaced a house block with a barrel.** | `Cmd+F` focuses the box on every tab. The keystroke that opens it is drained, so it cannot become the first character. |

That last one is the whole argument. A filter the author cannot reach by keyboard is a filter
they will miss, and a miss is not a no-op — **it edits the level**. An author who does not trust
the editor's palette will not use the editor's palette, and every other feature inherits that.

## Why the fence specifically

The three placement tools the editor had could not draw it, and the reason is arithmetic rather
than clumsiness — see *Why today's tools cannot draw it* below.

## The evidence, measured

### What the fence actually is

Measured from Sanctuary's `town.map.ron` (the 282 `prop_wooden_fence_single` placements):

| | |
|---|---|
| The piece | `prop_wooden_fence_single`, footprint **2.064 × 0.12 m**, height 0.838 m |
| The pitch between posts | **2.032 m** — a whole tile, twice, on a 1.016 m half-tile lattice |
| The corners | the **same piece turned 90°** — there is no corner descriptor in the kit |
| The shapes | **two**: four field enclosures (18×4 + 12×4 posts) and one graveyard (42 posts, with a **gap** in the south edge) |
| The gap | a 3-post opening in the graveyard's south side (a step of 6 cells where the rest step 2) |

So the fence is a *program* — a piece, a pitch, a direction, a corner rule and a gap — and the map
holds 282 rows of it.

### Why today's tools cannot draw it

**The pitch is not on the lattice.** The project's lattice is `snap_divisor: 3`, so the editor's
three rungs are 1.000 m, 0.333 m and 0.111 m. A fill steps by `ceil(span / pitch) * pitch`, so a
2.064 m fence steps:

| Rung | Lattice pitch | The fence's step | Error against 2.032 m |
|---|---|---|---|
| Tile | 1.000 | **3.000 m** | 0.968 m |
| Fine | 0.333 | **2.333 m** | 0.301 m |
| Finer | 0.111 | **2.111 m** | 0.079 m |

**None is 2.032 m.** The closest is 8 cm out — a visible gap in every joint, which is exactly the
thing a fence must not have. A fill is a *grid*; a fence is a *run*, and the run's pitch is a
property of the fence, not of the project's tile.

**The corner is not a piece.** The kit has exactly one fence descriptor. The corner is the same
piece at yaw 90. A fill has one yaw for the whole region; a fence has a yaw per *edge*.

**The flood has the wrong shape.** `F` spreads from a point in four directions until something
stops it. A fence is a *line* with a *gap* in it (the graveyard's south side) — a flood cannot make
a gap, and a box fill cannot make a corner.

**And the gesture is expensive before it even fails.** Reaching one palette row on a 211-descriptor
project took **~90 keystrokes**: the Map tab's filter box is **mouse-only** (`FocusFilter` is
`Context::Tiles`; the Map tab's box is `Pane::Palette`, which no key focuses), so the only keyboard
route is walking the palette with `Shift+Down`, and the walk *wraps* — the fence is in
`KIT · FENCE` and the `town` kit's groups come after it, so an over-long walk lands in the wrong
kit. Two clicks during the measurement landed in the *world* rather than the filter, and one of
them replaced a house block with a barrel.

That last one is the sharpest edge found: **the Map tab's filter cannot be reached by keyboard,
and a click that misses it falls through to the map and places a piece.**

## Sources

| # | Source | What it was used for |
|---|---|---|
| S1 | Merrell, P., Manocha, D., "Model Synthesis: A General Procedural Modeling Algorithm", *IEEE TVCG* 16(2):201–216, 2010, DOI 10.1109/TVCG.2010.112 | The constraint-over-output framing: the author's intent is stated as a *constraint on the output*, not as output |
| S2 | Merrell, P., Manocha, D., "Constraint-Based Model Synthesis", *Pacific Graphics* 2007 | The same argument for 3D: "the user input can often be specified in the form of a constraint on the output" |
| S3 | Alvarez, J., et al., "Interactive Sketching of Urban Procedural Models", *SIGGRAPH Asia* 2009, DOI 10.1145/2897824.2925951 | §6.2 "Repetition Management": a repetition is a **first-class gesture**, inferred from a few drawn segments, not a menu choice |
| S4 | Whiting, E., Ochsendorf, J., Durand, F., "Procedural Modeling of Structurally-Sound Masonry Buildings", *SIGGRAPH Asia* 2009, DOI 10.1145/1618452.1618458 | The *repeat along an axis* rule as a standard procedural rule, and the template/parameter split |
| S5 | Shirley, M. et al., *Procedural Content Generation in Games*, ACM Books, 2018 — ch. 11 "Mixed-Initiative Content Creation" | "The human has final say over what is produced by the tool"; "What novel and useful editing operations can be incorporated?" |
| S6 | Cooper, S., "Sturgeon: Tile-Based Procedural Level Generation via Learned and Designed Constraints", *AIIDE* 2022, DOI 10.1609/aiide.v18i1.21944 | A **small, mid-level constraint API** over a solver — a run is a constraint, not a feature |
| S7 | Cockburn, A., Ahlström, D., Gutwin, C., "Understanding performance in touch selections", *Int. J. Human-Computer Studies* 69(7):538–547, 2011 | Fitts' Law: target acquisition time grows with the log of the ratio of distance to target size — the reason a long palette walk is expensive and a filter is not |
| S8 | Merrell, P., et al., "A Semantic Scene Description Language for Procedural Layout Solving", *AIIDE* 2010 | A support is a first-class surface feature — a piece's placement derives from what it stands on, not from a guess |
| S9 | Gregory, J., *Game Engine Architecture*, 3rd ed., A K Peters/CRC, 2018, ch. 15 | The world editor's own vocabulary: splines, regions, "a special-case editing tool is almost certainly required" for shapes a general tool cannot express |

## Technique

### 1. A run is a constraint, not a set of pieces

S1 and S2 make the modelling argument directly. Model synthesis takes a simple shape and
*constrains* the output:

> "The user input can often be specified in the form of a constraint on the output. Any output that
> satisfies all of the user's constraint is acceptable."

A fence run is exactly that: the author states a *piece*, a *pitch*, a *direction* and a *span*,
and the tool produces the rows. The author is not stating 282 placements; they are stating four
numbers. S2's "minimal amount of user input" framing matters — a run of posts should cost the
author **one drag**, not 282 clicks and not a fill that cannot make the shape.

### 2. Repetition is a gesture, and it is inferred

S3's §6.2 is the closest prior art found, and its mechanism is the one to borrow:

> "In some cases, the user may want to apply an object type repetitively (e.g., a row of windows,
> stack of floors). Instead of sketching the object many times or making extensive use of copy and
> paste, we provide a quick mechanism to subdivide the anchor region, sketch an object type, and
> generate repetitions. **Our tool infers the intended horizontal or vertical 1D repetitive
> application from just a few user-drawn line segments.**"

Two things follow. First: **the repetition is the gesture, not a mode.** The author draws a line
and the repetition appears along it. Second: the repetition's *parameters* — how many, at what
pitch — are **derived from the drawn extent and the piece's own size**, not asked for in a dialog.

S4 names the same rule from the geometry side: the standard procedural rules are "repeat, split,
transform", and the *repeat* rule's parameters are the span and the count.

### 3. A run is a small, mid-level API over an arrangement

S6's Sturgeon is the design precedent for *how* to express it: a small mid-level constraint API
that can be instantiated with various low-level solvers. Here there is no solver — the
arrangement is arithmetic — but the shape of the API is the point: **the run's parameters are a
few named values, not a bag of flags.** A run has: a piece, a pitch, a direction, a length, a
corner rule and (optionally) a gap.

S5's mixed-initiative rules say how the human and the tool divide the work:

> "The majority of mixed-initiative tools follow the principle that the human has final say over
> what is produced by the tool."

and, on what such a tool should add:

> "What novel and useful editing operations can be incorporated? A mixed-initiative environment
> offers the opportunity for more sophisticated level-editing operations than merely altering
> content as one could do in a non-AI-supported tool."

A run *is* a novel editing operation, and the human's final say is kept by the gesture: the author
draws the line, the tool fills it, and the author can still change the piece or the pitch.

### 4. The corner is a rule, and the gap is a rule too

The fence's corner is the same piece turned 90°, and the graveyard's south edge has a **3-post
gap**. Both are *rules the run applies*, not pieces the author places:

- **the corner rule** — at a turn, the next piece is turned to follow the turn. The turn is a change
  of direction, and the piece that carries it is the same piece.
- **the gap rule** — a run may have an opening, because a fence with no way through is not a fence.
  The graveyard's gap is 3 posts wide, and it is *part of the shape*, not a thing the author removed
  afterwards.

S9's world-editor chapter makes the same point about special cases: "if regions are always spherical
then the designers might be able to make do with a 'Radius' property in the property grid, but to
define or modify the extents of an arbitrarily shaped region, **a special-case editing tool is
almost certainly required**." A run with a corner and a gap is an arbitrarily shaped region, and
the general tool (the fill) cannot express it.

### 5. The pitch belongs to the run, not to the project's tile

This is the load-bearing measurement, and it is the thing the design must not get wrong. The
editor's lattice is the *project's* — one tile, subdivided — because a tile is what a solver sees
and what a composition's envelope is measured against. A run's pitch is the *piece's* business: a
fence post wants a post every 2.032 m, and that has nothing to do with whether the project's tile is
1 m and subdivided into thirds.

The rule the run must follow: **the pitch is derived from the piece's own footprint, and the run
never contradicts it.** A fence whose footprint is 2.064 m wants its posts 2.064 m apart (or
whatever the author asks for), and the run lays them on that. The run's lattice is *the piece's
lattice*, and it is not the project's.

This is the same discipline the editor already states for the fill — `fill::Lattice` exists
precisely because a fill needed *its own* lattice rather than a second reading of the click's — and
the run is the next case of it.

## What great looks like

Acceptance criteria, phrased so a reviewer could fail the implementation against them.

1. **One gesture draws the run.** The author arms the fence piece, drags a line, and the posts
   appear along it. The drag is the same gesture the removal box and the fill box already use —
   press, drag, release — so no new gesture is learned.
2. **The pitch is the piece's, and it is not the lattice's.** A run of a 2.064 m fence places its
   posts **2.064 m apart** (or the pitch the author asks for), and the result is *not* one of the
   project's snap rungs. A test asserts the spacing is the piece's, at every rung.
3. **The corners turn the piece.** A run that turns lays the corner post at the new angle, using
   the same descriptor. A test asserts the corner row's yaw differs from its neighbours' by 90°.
4. **A gap is expressible.** A run can be given an opening, and the posts around it are absent. A
   test asserts the count and the positions.
5. **The run lands on the piece's lattice, not the map's.** Every post's position is exactly the
   anchor plus `n × pitch` along the direction. A test asserts the positions to the millimetre.
6. **The run is undoable in one act.** `Cmd+Z` takes the whole run back, and `Cmd+Shift+Z` puts it
   back — the same one-act-per-gesture rule the fill and the box already follow.
7. **The run is a preview before it is a commit.** While the drag is in progress the ghost shows
   the posts it will lay, so the author sees the corner and the gap before letting go.
8. **A refusal, not a degradation.** A run that cannot be laid — a piece with no measured
   footprint, a pitch that is not finite, a span shorter than one pitch — **refuses, by name**, and
   lays nothing.
9. **The consumer gets the rows.** The run's posts are ordinary placements, so the game's loader
   needs no change and the count in the inspector includes them. (The previous iteration's work
   makes this true; this iteration must not break it.)
10. **The palette is reachable by keyboard.** The Map tab's filter takes the keyboard, so a piece
    is one keystroke away rather than ninety.

## Gaps

The literature does not settle three things, and this project must decide them on its own.

- **How the gap is expressed.** S3's repetition mechanism infers the repetition from the drawn
  segments and does not mention an opening. S4's "repeat" rule has parameters but no interruption.
  The literature's answer is "the author's input is a constraint on the output", which leaves the
  *form* of the constraint to the tool. **This project's answer**: the gap is a parameter of the
  run — how many posts are missing, and where — because the graveyard needs it and a "draw the gap
  by hand afterwards" workflow would put the 282-piece problem back.
- **Whether the run is one gesture or two.** S3 infers the repetition from drawn segments; S9 says a
  special-case tool is "almost certainly required" for arbitrary shapes. A run of posts is a line,
  so the *drag* can carry it. But the *ring* (a closed rectangle of four runs) is a different
  gesture from the *line*. **This project's answer**: the drag draws a run; a closed ring is four
  runs, and the author draws them — the same way a fence is actually built. What the tool must
  supply is the *corner rule* that makes the four runs meet, not a rectangle primitive.
- **What the run's pitch defaults to.** The piece's own footprint is the obvious default (2.064 m
  for the fence, so the posts abut exactly), but the town map's fence is on 2.032 m — 3.2 cm
  narrower, so the posts overlap slightly rather than leaving a hairline. The literature does not
  settle which an author wants. **This project's answer**: the default is the piece's own
  footprint, because "the posts abut exactly" is the answer that is right for an unseen piece, and
  the author can change it.

The corpus holds no write-up on the *specific* interaction of a drag-that-repeats in a 3D top-down
level editor, on a pitch that is not the editor's snap lattice, with a corner rule. The design
here rests on S3's repetition-management mechanism, S4's repeat rule, S1/S2's constraint framing,
S5's human-final-say rule and S9's special-case-tool argument, and on the measurements above.
