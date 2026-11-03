# 2026-11-03 mcp: the `View` bucket cannot become operations, and the classification already said so

Focus: E-002 (#129) · Commits: 3f14890 (nothing shipped) · Machine: big · Outcome: nothing shipped

## Summary
The next E-002 verb looked like `FrameView`/`view_frame`: a discrete, `writes: &[]`, `Undo::None` view
verb with an existing rule (`resolve_frame`) to delegate to. The operation worked and its test passed —
but migrating it **breaks two protected classification tests**, because `FrameView` belongs to the
`camera` set whose own doc says such a verb *cannot* be a `Ctx` operation. Reverted; nothing shipped.

## What was tried, and what it cost
`view_frame` as an operation, with `frame_on_home` dispatching it. Its own test passed (`writes: &[]`,
`Undo::None`, the plan delegating to `crate::editor::resolve_frame`, the framing's real numbers in the
reply). Then:

```
the classification must cover the allowlist exactly: 15 project + 12 view + 1 held + 3 modal
+ 42 camera + 13 world against 85 listed
FrameView names no operation and is on neither list: it is neither migrated nor accounted for
```

Both are the repo telling me the classification is not an arbitrary list. `operations_have_two_front_ends.rs`
says of the `camera` set: *"A verb belongs here when a BRP method already moves the camera
(`grimoire/frame_view`) or when a front end reads it every frame; **neither can be a `Ctx` operation**,
and both are already reachable by an agent."* And `keys.rs:900` states it outright: `grimoire/frame_view`
moves the Map camera and **is not an operation**.

## The survey that followed
Rather than generalise from one mistake, every `View`-bucket verb was read:

| Verb | What it needs |
|---|---|
| `CycleTarget`, `MoveMode` | a cursor ray (`cursor_ground`) |
| `TilePrev`, `TileNext`, `PrevCandidate`, `FocusLibrary` | a list that exists only in its panel's state |
| `FocusFilter` | moves keyboard focus |
| `CopyInfo` | writes the system clipboard |

**Every one is pointer-, camera- or OS-driven by construction** — which is exactly why they were
classified where they are. The `View` bucket is not a migration queue; it is the set that cannot be
migrated.

## What follows
`UNMIGRATED_MAX` (85) can only fall by migrating the **15 `project`-bucket** verbs, not the 12 `view`
ones. Recorded in E-002's evidence so the next pass does not spend its time here.

## Review
Self-review, mine:
- **Did I nearly ship a wrong change?** Yes — the operation was green in isolation. The two
  classification tests are what caught it, and they exist for exactly this.
- **Did I weaken them?** No: reverted to the committed state, verified `git status` clean.
- **Was the survey worth the time?** Yes: it turns "try each one" into a stated rule with evidence.
- **Is anything owed?** No.
- **Is `nothing shipped` the honest outcome?** Yes. The pass produced a durable finding and no code.

## Test diffs
None: the added test was removed with the feature.

## Release
None.

## Next focus
The 15 `project`-bucket verbs in `UNMIGRATED`, which are the actual E-002 debt.
