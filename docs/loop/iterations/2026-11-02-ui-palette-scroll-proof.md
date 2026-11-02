# 2026-11-02 ui: the palette's scroll is why the clip would not record (#231)

Focus: #231's owed clip · Commits: none · Machine: big · Outcome: nothing shipped (one cause proven, five hypotheses retired)

## Summary
The last three passes lost takes to a palette row that would not arm. This pass instrumented rather than
reasoned, and the answer is neither coordinates nor picking: **the palette is a scrolling list and
`UiGlobalTransform` reports laid-out, unscrolled positions.** `sofa` is laid out at y = 907 while the list
has scrolled 1553.75 px, so on screen it sits above the panel and every click aimed at 907 hit the map.

## Baseline
Gate: green (`c69e61d`). `accept.py`: not run. Load 13.5, 150 GB free, 12 GB available RAM.

## The measurement
| what | value |
|---|---|
| the palette list's `ScrollPosition` | `[0.0, 1553.75]` |
| where `sofa` reports its centre | y = 907 |
| the window | y 0 … 720 |

Read from a live editor over BRP: `world.query` for `Text`, `ComputedNode`, `UiGlobalTransform` and
`ScrollPosition`, after filtering the list to one row.

## What is retired, with the evidence that retired it
1. **The observer never runs** — pass 242's counter read 0, but the click never reached a row.
2. **`retarget_pointer`** — a real defect (a windowless app's pointer was re-aimed at the viewport image),
   fixed in `7679cef`; not this one's cause.
3. **The harness's target mapping** — the option's `ComputedUiTargetCamera` is the `surface.image`
   camera and the pointer's `Location` agrees.
4. **The physical/logical factor** — tried both directions; the outcome was byte-identical.
5. **`bevy_ui` picking not seeing the click** — it sees it; it arrives at a place with no row.

## Why no fix is committed
The obvious correction is to walk from a row up to its scrolling ancestor and subtract that offset.
`ChildOf` **is not reflected over BRP** — `world.query` returns only the components named, and
`option: ["all"]` comes back empty — so the recorder cannot read the parent chain. Two mechanisms are
open for the next pass, both named on the issue: a reflected component carrying a row's on-screen rect, or
setting `ScrollPosition` itself (readable, and a component, so writable through `world.mutate_components`)
before clicking the laid-out position.

`tools/demo/record.py` is reverted to `464a685`. Nothing is claimed that was not measured.

## Test diffs
None.

## Release
None: nothing changed.

## Next focus
Scroll the palette deterministically, then click; or add a reflected on-screen rect.
