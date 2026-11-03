# 2026-11-03 mcp: the `project` bucket needs `Ctx` to carry panel state — an owner decision

Focus: E-002 (#129) · Commits: bba4694 (nothing shipped) · Machine: big · Outcome: nothing shipped

## Summary
Last pass established that the `View` bucket cannot become operations. This pass read the other half —
the **15 `project`-bucket verbs** that were called "the debt" — and found the same wall from the other
side: each one needs a whole resource, while `Ctx` carries only values.

## The survey (all 15, read not assumed)
| Verb | What it needs |
|---|---|
| `FoldPack` | toggles `ImportState` (the Meshes tab's candidates, selection, scan flags) |
| `EditTile` | a cursor ray, a camera, `ImportState` and `Mode` |
| `PickComposition`, `CompositionPrev/Next`, `ComposePrev/Next`, `ComposeMemberPrev/Next`, `CarouselPrev/Next` | `ComposeState` walks |
| `TileListPrev/Next`, `PrevRig/NextRig` | `keys::repeating` list walks |
| `ComposePrev/Next` (again, by design) | the compose tab's own cursor |

`Ctx` carries `project`, `root`, `open`, `source`, `session`, `view`, `proposal` — **every one a value**,
cloned in and cloned out (`ops::take`'s doc explains why: the registry is `dyn`, so a lifetime cannot be
held).

**So the debt is not "migrate 15 verbs"**: it is "give `Ctx` a way to carry one panel's state". That is an
API decision — which panel state, whether it is cloned like `Project`, whether `ImportState` becomes part
of the operation vocabulary — and it is the owner's, not a pass's. Recorded in E-002's evidence.

## A clip attempt, and why it is owed
E-037's line ("an operation arms a brush piece, so an agent can arm one and its hint shows") had no clip.
The operation **works and is visible**: `brush_arm lamp_tall` returns *"armed lamp_tall"*, and the sheet
shows the `lamp_tall` row going from grey to highlighted teal.

Two attempts at a clip, both failing `check.py`:
1. On a fresh editor, `lamp_tall` is **already armed** at startup, so the operation changes 0.00% of the
   scene — the highlight was already there.
2. Arming `pipe` first and then `lamp_tall` changes **0.42%**, under the 5% floor: a single row's tint is
   a small share of any crop that also shows the viewport, and the pre-operation run is 3.33 s static
   against a 1.5 s limit.

Stopped after two, per the rule. The clip needs a storyboard whose crop is the palette alone (the row is
then a large share of the frame) and a shorter opening — a note for the pass that records it.

## Review
Self-review, mine:
- **Did I assume the classification was wrong again?** No: the survey was done before concluding, which is
  exactly what pass 285 failed to do.
- **Did I ship a wrong change?** None was made; `git status` shows only `backlog.json`.
- **Is the clip attempt wasted?** No: it establishes that the operation works and is visible, and records
  the two specific reasons the check fails.
- **Anything owed?** E-037's clip, with the shape it needs named.
- **Honest outcome?** Yes — two passes with nothing shipped is a pattern worth breaking, and the next pass
  should take the clip (concrete, bounded) before the API question.

## Test diffs
None.

## Release
None.

## Next focus
E-037's clip with a palette-only crop; then the `Ctx`-carries-panel-state question as an owner decision.
