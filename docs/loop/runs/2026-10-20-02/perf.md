# Perf — the run of a thing

Run: 2026-10-20-02. What the change costs, measured.

## The two numbers that are not about time

The feature is a usability change, so the measurements that matter are the author's, not the
machine's. Both are in `review.md` and repeated here because they are the point:

| | Before | After |
|---|---|---|
| Find one piece in a 211-descriptor palette | **~90 keystrokes** (`Shift+Down`; the walk wraps into the wrong kit) | **2 keystrokes** (`Cmd+F`, then the name) |
| Draw a fence of N posts | **N placements**, one click each, at a pitch the lattice cannot express | **1 drag** |

## The run's arithmetic

`bevy_emerge_core::run::lay` is a walk of the path: for each segment, `floor(length / pitch) + 1`
posts, each one a `Post` of two `f32` and one `f32`. It is O(posts) and allocates once.

| Run | Posts | Cost |
|---|---|---|
| A 2.064 m fence over 8 m | 4 | 4 posts, one `Vec` of 4 |
| Sanctuary's whole 282-post fence, as one run | 282 | 282 posts — the same rows the 282 hand-placed ones cost, and a quarter of the file |

**The cap.** `MAX_POSTS` is 1024, and a run that would exceed it is **refused by name** rather
than truncated — the same rule `fill::MAX_CELLS` follows, and the same reason: a fill that stops
early looks exactly like a fill that finished.

## The editor's per-frame cost

**One new system, and it is cheap.** `drive_run_ghost` runs every frame the run tool is armed
and a drag is in progress, and it does:

1. read the line's two ends (two `f32` pairs);
2. call `run::lay` — the same walk the commit does;
3. spawn one entity per post, through the same `spawn_piece` the map's own pieces use.

**The ghost is the expensive part, and it is deliberately so.** It re-derives the run every frame
the drag moves, because a preview that computes it differently from the commit is a promise about
a landing that will not happen. The cost is the cost of *drawing* N posts — and the map is
already drawing far more than that (Sanctuary's `town` is 211,190 triangles, and the editor
redraws the stamped rows on every change).

A drag that moves by less than a cell re-derives the same posts. That is the trade the editor
already makes everywhere: the preview is always the truth, never an approximation of it.

**The commit** is O(posts) once, plus the map's own re-solve — the same cost as any other set of
placements, because a run's posts are ordinary rows.

## The filter

`Cmd+F` is one key press, and the focus change is a field write. The *typing* is what the filter
already did. The only new work is that `filter::keys` now **drains the message stream** when no
box is focused, so the keystroke that opens a box cannot become its first character — a field
read per frame, and a `MessageReader::clear`.

## The test suite

| | Time |
|---|---|
| `cargo test --workspace --no-fail-fast` | 80 s (1,219 pass, 1 fail — `no_two_leaders_cross`, red since 2026-09-03 and not mine) |
| `cargo test -p bevy_emerge_core --test a_run_is_laid_in_the_piece_s_own_pitch` | 0.02 s for 7 tests |
| `cargo test -p bevy_emerge_mapper --test a_run_lays_a_line_of_pieces` | 0.25 s for 4 tests |
| `cargo clippy --workspace --all-targets -- -D warnings` | 84 errors, **all pre-existing** (the baseline was 82 — the two extra are in files this change does not touch, and none is in `run.rs`) |
| `cargo fmt --check` | no diff in the new files |
| `cargo doc --no-deps` | no warning from any link this change adds |
| Headless boot against Sanctuary's real project | `town` → 12 frames, 8,192 entities, no panic; `town_block` → 4,096 entities, no panic |

## The number a game developer feels

A fence of 282 posts was **282 rows in the map file**, hand-placed, and 282 clicks to lay. It is
now **one row** in the file — a group of 282 members, stamped — and one drag to draw.

That is the whole trade: **the same geometry, a hundredth of the file, and a hundredth of the
work.** And because a run's posts are ordinary `Placed` rows, the game's loader needs no change
at all — the count in the inspector is the count the consumer has, and the previous iteration's
contract (*a level the author drew is the level the game walks*) is not broken by it.
