# 2026-11-03 relations: a map's relations can be added by a person or an agent, not only hand-written

Focus: the constraint layer (#137) · Commits: ba635ca..748d063 · Machine: big · Outcome: shipped

## Summary

The relation vocabulary has been complete for weeks and `relations_solve` mends a map against it — but
the only way to *add* a relation was to open `.relations.ron` in a text editor. `relations_add` closes
that: one relation in, one out, previewed, atomic, and undone by value. Four tests, all passing.

**The pass also found that the command's own lens is stale**, and recorded it where the owner reviews.

## Baseline

Gate: green before (ba635ca). `accept.py`: not run (no solver path touched). Load 5.3, free disk 121 GB,
available RAM 13 GB.

## Bugs fixed

none.

## Enhancement: `relations_add`, the constraint layer's missing authoring door (#137)

Goal: a person or an agent can add a relation to a map without a text editor.
Files: `crates/bevy_cauldron/src/ops_relations.rs` (new `relations_add` module), `ops.rs` (registry row
and `UI_ENTRY_POINTS` row), `tests/relations_add_is_an_operation.rs` (new), `tests/all.rs` (one `mod`).
Proof: `cargo test -p bevy_cauldron --test editor relations_add_is_an_operation::` — 4 passed;
`operations_have_two_front_ends::` — 11 passed; clippy clean over `--all-targets`.

The operation takes `of`, `kind` and `hard` — **the file's own shape**, deserialised through
`relations::Relation`/`Kind`, so a renamed variant is a compile error here rather than a second
vocabulary that drifts. It:

- writes through `atomic_file::write`, like `data_set` and `map_save`;
- **refuses a relation about a piece the map does not hold**, naming the near misses — the solver would
  skip such a relation silently and the author would believe it held;
- refuses a file that does not read, rather than overwriting it;
- refuses the same relation twice, so it cannot be counted twice;
- takes one out again with `remove`, **by value**, so an undo cannot remove somebody else's relation.

**A first draft got `only_as_undo` wrong** and the undo test caught it: it marked `remove` as
"undo only", so a caller who wanted to delete a relation was refused and told to use `operation_undo`.
Deleting a relation is a change a caller may ask for, the same as deleting a piece; the gate is gone.

MCP: no new tool — the generic `grimoire_apply_operation` reaches it by name, which is the rule for a
small tool surface. Preview: `plan` reports `would` and the exact text it would write. Undo: `Undo::Inverse`,
the relation by value.

## Borrowed from Mixar

none.

## Performance

`perf.py` not measured (needs a live editor on a real display). The operation runs on a click, not a frame.

## Acceptance

`reached` unchanged — no solver path touched.

## Review

Reviewer could not run (no subagent spawned inside the budget). Five questions, mine:

1. **Does the relation the operation writes actually constrain the map?** Yes — asserted, not assumed:
   the test reads the file back, parses it, and runs `relations::solve_map`, which reports
   `unmet_before == 0` for pieces 2 m apart under a 3 m `Near`.
2. **Can it corrupt a file?** No: it refuses a file that does not read, and writes atomically.
3. **Can a typo'd piece id look like a working relation?** No — refused, with the near misses named.
4. **Can the undo remove the wrong relation?** No: the inverse is the relation by value, and the test
   adds a second relation between the target and the undo to prove it.
5. **Does it add a fourteenth MCP tool?** No; it goes through `grimoire_apply_operation`.

## Test diffs

- `crates/bevy_cauldron/tests/all.rs`: one `mod relations_add_is_an_operation;` line, in order (the
  standing exception for a new editor test file).
- `crates/bevy_cauldron/tests/relations_add_is_an_operation.rs`: new.
- No existing assertion changed.

## Release

None: `bevy_cauldron` is `publish = false`; no publishable crate's files changed.

## Next focus

The `Camera`/`View` classification re-pin is now the thing blocking three migrations (`MoveMode`,
`CycleTarget`, `ToggleGhost`); it is recorded on #129 with a recommendation. Otherwise the same: #224 →
#210's owner re-pin, bevy-elements#55, and #113's recorded-reply measurement.
