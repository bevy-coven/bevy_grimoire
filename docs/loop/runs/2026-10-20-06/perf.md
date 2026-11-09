# Perf — the composition rename

Run: 2026-10-20-06. Feature: **an author renames a composition from the COMPOSE tab, and every map
that stamps it follows.**

---

## The measurement that decided the design

The plan's own risk table had one row that needed a number before the build:

> **The span surgery cannot address a leaf inside a nested block.** — *to be measured before the
> build.*

**Measured, with `crates/bevy_emerge_core/examples/probe_roundtrip.rs` against Sanctuary's own
files.** The leaves the rename has to address:

```
compositions[0].id                                 text="town/house_cottage_plaster"
compositions[0].members[0].id                      text="p00_wall"
compositions[0].members[0].body.id                 text="wall"
stamps[0].of                                       text="town/house_cottage_plaster"
```

**The two leaves the rename needs are both addressable by path**, and the paths are the point:
`compositions.[i].id` and `stamps.[i].of`. The rename is *"find the leaf at this path"*, not *"find
the string"*.

### What an ordinary rewrite would have cost

| file | bytes | after an ordinary re-serialize |
|---|---|---|
| `compositions.ron` | 204 KB | **byte-identical** |
| `town.map.ron` | 75,089 | **228,856** |

**The map is the reason the surgery is worth it.** An ordinary rewrite expands every one-line
placement into six lines — a 229 KB diff for a one-word change, on a file the author wrote. The
surgical edit leaves the file at **751 lines, with only the `of` leaf changed**.

*(The map's expansion is the editor's **existing** save behaviour, not a regression this run
introduces — but a rename that reformats it would be.)*

### The measured result, on the shipped project

| measurement | value |
|---|---|
| `compositions.ron` after the rename | **5,817 lines** — the same count as before |
| `town.map.ron` after the rename | **751 lines** — the same count as before |
| the hand-written `note:` in `compositions.ron` | **intact, byte for byte** |
| the map's `of` leaf | `of: "abc"` → `of: "house_renamed"` |
| the composition's `id` leaf | `id: "abc"` → `id: "house_renamed"` |

**A rename is a one-word diff in two files**, and that is the whole performance claim: the work is
proportional to the number of stamps, not to the size of the file.

## Timings

| operation | time | how measured |
|---|---|---|
| `cargo test --workspace` | see the table below | wall clock, release-adjacent dev profile |
| the rename, end to end, in the editor | **under 200 ms** | the editor's own status line, between `Enter` and the receipt |
| `scan_ron_leaves` over a 204 KB `compositions.ron` | **under 5 ms** | inside the writer's own timing |

**The rename is not a hot path** and is not treated as one: it is an author-initiated, file-writing
operation that happens once per name change.

## The test suite

| suite | result |
|---|---|
| `cargo test --workspace` | **1,246 passed, 1 failed** |
| the one failure | `no_two_leaders_cross` — **pre-existing**, and unchanged by this run |
| clippy | **84 findings, identical to the baseline** — this run adds none |
| `cargo fmt --check` | **no new diff in the files this run touches** — see below |
| `cargo doc --no-deps` | clean |

### The fmt count, and why the repo is not rustfmt-clean

**The repo is not rustfmt-clean at baseline** — 1,437 diff hunks across the workspace. The loop's
convention, recorded in `docs/loop/log.md` across four runs, is:

> `cargo fmt --check`: **no diff in the new files**; the two files this change edits **keep their
> pre-existing** diffs.

**So the rule is: this run's new code is rustfmt-clean, and the pre-existing diffs stay.** That is
not the same as "run `cargo fmt`", which reformats the whole file and buries the change in 600
lines of noise.

**Measured, per file, before and after:**

| file | baseline hunks | after this run |
|---|---|---|
| `compose.rs` | 94 | **94** |
| `editor.rs` | 87 | **87** |
| `keys.rs` | 22 | **22** |
| `project.rs` | 11 | **11** |
| `chrome.rs` | 16 | **16** |

**The diff is +282 lines, all of them this run's.** A blanket `rustfmt` was tried and **reverted**:
it turned a +282-line change into a +734-line one by reformatting 67 hunks of code this run never
touched, and the diff stops being reviewable.

### The known-red

`no_two_leaders_cross` in `crates/bevy_emerge_mapper/tests/headless.rs` fails at the baseline commit
and fails identically after this run. It is **not** this run's failure and it is **not** fixed by
this run. It is the third item on the log's remaining list, and it stays there.
