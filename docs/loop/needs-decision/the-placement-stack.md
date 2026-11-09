# `placement/`: delete it, or finish it

**Status:** open — owner's call. Found 2026-10-20 by the technical review.

## The problem

`crates/bevy_emerge_core/src/placement/` is 2,546 lines: a staged placement stack with an IR
(`ir.rs`, 464), a `Solver` trait and orchestrator (`solver.rs`, 339), a RON asset manifest
(`manifest.rs`, 290), fine-level prop scatter (`scatter.rs`, 312), a closed surface-class table
(`surfaces.rs`, 68), and three backends — WFC (`solvers/wfc.rs`, 209), Metropolis
(`solvers/metropolis.rs`, 642) and constraint (`solvers/constraint.rs`, 196).

**Sanctuary uses none of it.** The game imports 9 of core's 31 top-level modules and never touches
the `placement/` subtree. The editor does not use it either.

Two concrete problems, both verified by reading the source:

- **The WFC support table is fully permissive.** `placement/solvers/wfc.rs:76-81` sets `support` to
  `full` in all four directions, so every prototype is compatible beside every other. The backend
  is a **weighted fill, not a constrained one** — it can never contradict. The doc comment says
  adjacency predicates are compiled in at "Stage 4+", which has not arrived.
- **The 32-prototype limit is unguarded on this path.** `n = tiled.len() + 1`, then `1u32 << n`.
  Above 32 that overflows in debug, and `collapse_grid` asserts `n <= 32`. `grammar::learn` guards
  its own path (`MAX_PROTOTYPES = 32`, refused by name); this one does not.

Also: `#![allow(dead_code)]` in `ir.rs:11` ("Staged scaffolding") and `solver.rs:7`; `category` is
parsed and unused in `manifest.rs:23-24`; the Metropolis backend is a documented departure from its
paper (no swap move, single chain rather than parallel tempering).

Neither defect is a live bug, because nothing calls this code. But it is **a second, parallel idea
about how to place things**, and the crate's own rule is "one path per feature, no fallbacks, no
legacy shims, no stub placeholders".

## The options

### A. Delete it

Remove `placement/` and its three backends, and the `bevy_emerge_solver` dependency if nothing else
uses it. Fold the two pieces worth keeping into their real homes: `surfaces.rs` (a closed
surface-class table) into `descriptor.rs` or `vocab.rs`; `scatter.rs` (fine-level prop scatter) if
it is wanted, into a module of its own with a real name.

- **Cost:** ~2,546 lines go. Any future that wanted a general placement solver has to start over.
  `bevy_emerge_solver` (604 lines, batsat) loses its only consumer, though `constraints.rs` also
  uses it, so it survives.
- **Gains:** one placement path — the grammar/WFC one that `rules.rs` implements and the editor's
  `G` preview shares with the game. The "staged scaffolding" stops being scaffolding.

### B. Finish it

Give the WFC backend a real adjacency table, guard the prototype limit, give the constraint
backend the fields the IR declares, and write the tests. Then it is a real feature, and the
offline search it was built for becomes reachable.

- **Cost:** the largest option by far, and it competes with authoring features for attention. The
  `bin/train` consumer and the `map_elites` crate it was extracted for are not in this repo.
- **Gains:** a general, constrained placement solver in the engine-free core, usable by any
  consumer.

### C. Freeze it, do not grow it

Leave the code, add a module-level note that it is unfinished and not on the path, and do not
build features on it. The loop's candidate list may not include it.

- **Cost:** the least. The dead code stays, and a future agent may still build on it by mistake.
- **Gains:** nothing is thrown away; the misleading parts are labelled.

## Recommendation

**A — delete it** — unless the offline search (`bin/train`, `map_elites`) is coming back, in which
case **B** with a named owner and a scope.

The reasoning: the editor and the game already have a placement path that works, it is shared
between them, and it is the one the WFC learning in `grammar.rs` and `rules.rs` is built on. A
second placement stack with a permissive constraint table is the kind of thing that looks like a
feature and is not. The crate's own rule — "no stub placeholders" — is the tiebreaker.

If the offline search is coming back, that is the fact that decides it, and it is the owner's to
state.

**Owner decisions needed:**

1. Is the offline search (`bin/train`, `map_elites`) coming back? If yes, this is B, and it needs
   an owner and a scope.
2. If no, is anything in `placement/` worth keeping on its own terms — the surface table, the
   scatter, the manifest?

## What the loop must not do

Do not grow `placement/`, and do not quietly re-implement it elsewhere. A feature that needs
constrained placement builds on `grammar.rs` / `rules.rs` / `wfc.rs`, which is the path the game
and the editor already share. If that path cannot do what the feature needs, that is a reason to
open this decision, not to start a fourth one.
