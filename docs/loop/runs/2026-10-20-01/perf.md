# Perf — the stamped group the game never sees

Run: 2026-10-20-01. What the change costs, measured.

## What the change does at runtime

The loader expands a map's stamps once, at load. Nothing in the editor's per-frame path changed:
`redraw_stamps` already expanded the stamps to draw them, and it still does.

## The expansion itself

`composition::expand` is the editor's function and the loader's now — one implementation, so the
number below is the same number for both.

| Map | Stamps | Rows a stamp brought | Expansion |
|---|---|---|---|
| Sanctuary's `town.map.ron` | 0 | — | not called (`expand_map` returns early on an empty stamp list) |
| Sanctuary's `town.map.ron` + one house stamp | 1 | 18 | **below the measurement floor** — the whole expansion is a sort, a flatten and 18 row constructions |

`expand_map` on a map with no stamps is `expand` with an empty slice, which returns
`Expansion::default()` immediately. The added cost on the common path (a map with no groups) is
one slice borrow.

## The count

`census::of_map` now expands, so the inspector's `N placed` costs one expansion per frame the
map changes. The expansion is a **total order** — stamps by id, then members by path — so two
frames of the same map produce the same rows, and the panel does not flicker.

The panel already redrew itself only on a change (`open.is_changed()`), so the count is computed
when the map changes and not on every frame.

## The load

`project_data::load` now expands each map it loads. Measured on Sanctuary's real project:

| Map | Placements before | Placements after | Load |
|---|---|---|---|
| `town` | 727 | 727 (0 stamps) | unchanged — no stamps, no expansion |
| `town` + one house stamp | 728 | 746 | one expansion of 18 rows |

The cost is bounded by `MAX_RESOLVED_MEMBERS` (256) per stamp and `MAX_COMPOSITION_DEPTH` (8),
both of which `expand` already enforced and refuses by name past.

## The editor's per-frame cost

**Unchanged.** The systems that run per frame are the same systems, and the two that touch the
stamps (`redraw_stamps`, `drive_stamp_ghost`) were already expanding.

The one change is `redraw_stamps`' log line, which used `census::of_map` and now reads
`open.map.stamps.len()` directly — the census's expansion would have been a second expansion of
the same stamps on every redraw.

## The build

| | Time |
|---|---|
| `cargo build --workspace` (cold, 16m 06s at baseline) | dominated by the dependency graph, not by this change |
| `cargo test --workspace --no-fail-fast` | 268 s (baseline 1,099 s — the difference is machine load, not the change: several other cargo processes were sharing the target directory) |
| `cargo test -p bevy_emerge_core --test stamps_reach_the_consumer` | 0.01 s for 11 tests |
| `cargo test -p bevy_emerge_mapper --test the_editor_and_the_game_agree` | 0.27 s for 3 tests |

## Headless boot against Sanctuary's real project

```
town       → 12 frames, 8,192 entities alive, no panic
town_block → 12 frames, 4,096 entities alive, no panic
```

Identical to the baseline. A map with no stamps is not expanded at all.

## The measurement that matters

Before the change, loading Sanctuary's `town.map.ron` with one house stamped gave the game
**728** placements. After, it gives **746**. The editor drew 746 in both cases.

That is not a performance number. It is the whole point of the run, and it is the only number in
this document that a game developer would feel.
