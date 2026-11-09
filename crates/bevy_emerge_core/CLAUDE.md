# bevy_emerge_core — notes for agents

The engine-free half of world building: the asset descriptor and map schemas, a constraint IR with three solver backends, WFC, Poisson/Delaunay geometry, a seeded ChaCha8 RNG, comment-preserving RON surgery, a hand-rolled glTF reader, and animation-clip measurement.

## Source of truth

The source of truth for this crate is [`Ladvien/bevy_emerge`](https://github.com/Ladvien/bevy_emerge) at `crates/bevy_emerge_core/` (MIT OR Apache-2.0, Bevy 0.20). It began as `crates/emerge-core/` in [`Ladvien/foundation_vs_slop`](https://github.com/Ladvien/foundation_vs_slop), history kept; foundation_vs_slop still carries the GPL Bevy 0.19 original until it ports. Make changes here, in their own commits with tests here; a consumer game never vendors or patches a copy.

## Build and test

A leaf — `serde`, `serde_json`, `ron`, `rand`, `rand_chacha` — so it builds and tests on its own: `cargo test -p bevy_emerge_core`.

## The non-negotiable: engine-free is enforced, not intended

`tests/engine_free.rs` is the ratchet. Allowed: `serde`, `serde_json`, `ron`, `rand`, `rand_chacha`. Forbidden as substrings, so `bevy_math` is caught as readily as `bevy`: `bevy`, `avian`, `wgpu`, `winit`. A second test scans the sources as a backstop for the case where someone adds the dependency *and* edits the allowed list to match.

Why it is enforced rather than trusted: **three consumers share this crate** — a game, an offline search that fans out across worker subprocesses, and a standalone editor — and none of them should have to agree on a renderer in order to share a schema. That was a *comment* for months, and a comment cannot fail a build. Widening the dependency list should cost an argument and a deliberate edit there.

**`DetRng` is the workspace's one generator.** `map_elites` depends on this crate solely for it, rather than copying two methods, because a local copy would be a second definition of the generator every reproducibility claim rests on. Changing a draw here moves goldens everywhere.

**The glTF reader is hand-rolled on purpose** (`glb.rs` reads the JSON chunk, data only). Pulling a glTF crate would add "a second, differently-behaved reader of an asset the engine already parses its own way".

**RON surgery preserves comments** because the files it edits are hand-authored and reviewed. A writer that reformats them is not a faster version of this — it is a different, lossy tool.

**A map's stamps reach the consumer, and that is the loader's job.** A map holds *references* to compositions; the rows those references stand for are derived, and somebody has to derive them. That somebody is `project_data::load`, through `Map::expanded` — so a game that loads a project gets the stamped rows in `map.placements`, and the editor and the game cannot disagree about what a stamp means.

It used to be the editor only, and the two were different levels: the editor drew a group's 18 members, the game received 728 rows where the editor had 746, and nothing said so. The editor's save gate checked that the stamps *could* expand while the game did not expand them at all — the gate was on the wrong side of the file. Now both go through the same call.

Two rules follow, and both are load-bearing:

- **One expander.** `composition::expand` is the only function that turns a stamp into rows. `composition::expand_map` is the same call with the map's own stamps. A second implementation is how the editor and the game drift.
- **The count an author reads is the count the game has.** `census::of_map` expands, so the inspector's `N placed` is the row count the consumer will have — the hand-authored rows *and* the ones a group brought. A panel that said "727" beside a game holding 745 would be a panel lying.

**A run's pitch is the piece's, not the lattice's.** `run::lay` puts one piece at a time along a
path, and the pitch it uses is the **piece's own footprint** — not a rung of the project's
snap lattice. The reason is measured: Sanctuary's fence posts stand 2.032 m apart, and the
lattice's rungs are 1.000, 0.333 and 0.111 m, so a fill lays that fence 3.000, 2.333 or 2.111 m
apart — never 2.032, and 8 cm out at best, which is a visible gap in every joint.

A run is the second thing in this crate that needs **its own** lattice rather than a second
reading of the project's (`fill::Lattice` was the first). The rule is the same: the *project's*
lattice is what a solver sees and what a composition's envelope is measured against; a piece's
own spacing is the piece's business.

No Bevy dependency exists here, and none may be added.

## Rules

- **No `unwrap()`**, no `expect` on parsed data. Every input to this crate is a file somebody wrote by hand; malformed input is an error to report, not a panic.
- **One path per feature.** No fallbacks, no legacy shims, no stub placeholders — do not write a degraded result on a solver failure, and do not silently default a missing field.
- Leave academic paper references in comments where a paper informed the code.
- Consult Bevy documentation often. It can be found at codex_fs/offline_reference_docs/bevy-0.19-book/

## In the monorepo

The game re-exports this at its old paths — `crate::{geom, rng, wfc}` (`src/lib.rs:169`), `placement::{ir, manifest, scatter, solver, solvers}` (`src/placement/mod.rs:26`), `site_editor::source_map`'s RON helpers (`src/site_editor/source_map.rs:44`) — so the extraction moved no caller. Root `CLAUDE.md` and `TESTING.md` carry the project-wide rules; neither is part of this mirror.
