# bevy_grimoire — notes for agents

The runtime half of grimoire: take an `bevy_grimoire_core` library plus a map, and put it in a Bevy world.

## Source of truth

The source of truth for this crate is [`bevy-coven/bevy_grimoire`](https://github.com/bevy-coven/bevy_grimoire) at `crates/bevy_grimoire/` (MIT OR Apache-2.0, Bevy 0.20). It began as `crates/emerge-bevy/` in [`Ladvien/foundation_vs_slop`](https://github.com/Ladvien/foundation_vs_slop), history kept; foundation_vs_slop still carries the GPL Bevy 0.19 original until it ports. Make changes here, in their own commits with tests here; a consumer game never vendors or patches a copy.

## Build and test

**Not a leaf.** It path-depends on the sibling `bevy_grimoire_core`, so it builds *inside* the workspace, not on its own: `cargo check -p bevy_grimoire`.

This is the **only** crate in the grimoire set that knows what a renderer is, on purpose. `bevy_grimoire_core` is the schema, the solvers and the validation with no engine in it, and `crates/bevy_grimoire_core/tests/engine_free.rs` fails the build if that stops being true. This is the other side of that line.

## The non-negotiable: one spawner

`spawn_descriptor` is deliberately the **single** shared spawner. `bevy_cauldron` shows you a map and the game plays it, and they must agree about what a placement looks like down to the last degree of yaw. The way to guarantee that is not care — it is having one function.

**Do not add a second spawner**, not for the editor, not for a test, not behind a flag. The alternative was tried elsewhere in this tree and is on the record: `bake.rs` and `site_editor::source_map` independently grew the same RON writer, and one of them had a bug the other did not.

Masks, stacked Y, role masks and seat positions all resolve **once at load**, before anything spawns. Keep that ordering; resolving per-spawn is how two callers start disagreeing.

## Rules

- **Bevy 0.19 is pinned.** Read the vendored source (`~/.cargo/registry/src/index.crates.io-*/bevy-0.19.0/`, and its `examples/`), not bevy.org — that documents `main` and has been wrong for this pin more than once.
- Consult Bevy documentation often. It can be found at codex_fs/offline_reference_docs/bevy-0.19-book/
- **A missing `Res<T>` panics its system in 0.19**; it does not skip. Take `Option<Res<T>>`, or have the plugin that registers the reader `init_resource` it.
- **All run conditions are evaluated — there is no short-circuit.** A bare `Res<T>` in a `.run_if(..)` closure panics whenever that resource is absent, even behind an earlier condition that returned false.
- **A bundle containing two of the same component panics.** Insert afterwards to override; do not pass a second one in the tuple.
- **No `unwrap()`.** A map is a file somebody authored — a missing asset, an unknown role or a bad transform is an error to report, not a panic mid-spawn.
- **One path per feature.** No fallbacks, no legacy shims, no stub placeholders. Never spawn a substitute for a descriptor that failed to resolve; fail loudly instead.

## In the monorepo

The game loads what `bevy_cauldron` writes through `src/grimoire_map.rs`, which uses `GrimoirePlugin` / `GrimoireWorld` — that is the step that makes the editor's output real. Root `CLAUDE.md` and `TESTING.md` carry the project-wide rules; neither is part of this mirror.
