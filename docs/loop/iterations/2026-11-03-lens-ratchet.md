# 2026-11-03 loop: the command's research lenses are checked against the tree

Focus: the loop's own steering (#280) · Commits: 9442120..491fe09 · Machine: big · Outcome: shipped

## Summary

**Every lens in the command says it is unbuilt; every one of them is built.** That is the pass's finding,
and it has cost three passes their item: each read the list, chose work from it, and found the work
already done. The command is the owner's file, so the pass filed the correction (#280) and made the drift
checkable with a test that fails when the set of stale lenses changes.

## Baseline

Gate: green before (9442120). `accept.py`: not run (no solver path touched). Load 4.8, free disk 115 GB,
available RAM 13 GB.

## Bugs fixed

none.

## Enhancement: the lens ratchet (#280)

Goal: a lens that says "not built" cannot silently name work that is in the tree.
Files: `crates/bevy_cauldron/tests/the_commands_lenses_match_the_tree.rs` (new), `tests/all.rs` (one `mod`).
Proof: `cargo test -p bevy_cauldron --test editor the_commands_lenses_match_the_tree::` — 2 passed; and
**verified load-bearing** by pointing one lens at a path that exists, which fails with *"the set of stale
lenses changed"*.

The measurement behind it, file by file:

| the command says | the tree has |
|---|---|
| "no constraint layer" | `relations::Kind`, **11** kinds from `Faces` to `ReachableFrom`, `.relations.ron` beside the map, `relations_solve` + `relations_add`, `constraints.rs` beneath |
| "Kit admission rules — not built" | `standards::{Standards, Rule, Dim, Why, Breach, MEET_WITHIN, Unnested}` |
| "kit stress tests — not built" | `adjacency::ring` and `adjacency::room`, surfaced by `grimoire_state.stress` |
| "Role-weighted repetition" | `repetition::{repeats, Role}` |
| "partially built … 14 tools today" | 17 tools; the count is now checked against the server's own `ToolDoc` table |
| "Options on request" | `layout_propose`'s `options` |

**Why the test reports rather than edits.** The command is protected; a test that failed on its staleness
would put `main` red over a sentence no pass may change. So today's two known-stale lenses are written
into the test as the receipt, and the assertion is that **the set does not change** — a new stale lens
fails, and one the owner fixes fails until the list is shortened. That is a check that can fail, which the
first draft of this file could not (it only `eprintln!`ed).

MCP: none. Preview/undo: n/a.

## Borrowed from Mixar

none.

## Performance

Not measured (`perf.py` needs a live editor). The test is a source scan of one file.

## Acceptance

`reached` unchanged.

## Review

Reviewer could not run (no subagent spawned inside the budget). Five questions, mine:

1. **Does the test fail when it should?** Yes, verified: one lens repointed at an existing path fails it.
2. **Can it pass by reading nothing?** No — a missing `build-loop.md` returns early and says so; a missing
   `ToolDoc` table asserts `declared > 0`.
3. **Does it edit the owner's file?** No, and the commit says so twice.
4. **Is the claimed measurement right?** Each row of the table names a path or symbol I read this pass; the
   ones I did not read (`options`) are marked by their operation parameter, which I did read.
5. **What did this pass not do?** E-002, #113, #193: the first two are behind protected re-pins and the
   third behind a family crate's own loop — all recorded on their issues.

## Test diffs

- `crates/bevy_cauldron/tests/all.rs`: one `mod the_commands_lenses_match_the_tree;` line (the standing
  exception for a new editor test file).
- New test file. No existing assertion changed.

## Release

None: no publishable crate's files changed.

## Next focus

#280 needs the owner to update the lens lines; #279 (the wall-clock test) needs a decision between
deleting two sleeps and injecting a clock; #210's first item needs the three-way re-pin.
