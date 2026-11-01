# bevy_devshot — notes for agents

A frame on demand: `touch screenshot.request` and the next frame writes `screenshot.png`, rendered straight from the GPU.

## Source of truth

The source of truth for this crate is [`bevy-coven/bevy_grimoire`](https://github.com/bevy-coven/bevy_grimoire) at `crates/bevy_devshot/` (MIT OR Apache-2.0, Bevy 0.20). It began as `crates/bevy_devshot/` in [`Ladvien/foundation_vs_slop`](https://github.com/Ladvien/foundation_vs_slop), history kept; foundation_vs_slop still carries the GPL Bevy 0.19 original until it ports. Make changes here, in their own commits with tests here; a consumer game never vendors or patches a copy.

## Build and test

A leaf — it takes only `bevy` with defaults off (`bevy_render`, `bevy_window`, `bevy_log`, `png`), so it builds on its own: `cargo check -p bevy_devshot`.

## The non-negotiable: one screenshot path

This crate exists *because* there were two. The editor carried a byte-for-byte copy of the same capture rig; that copy was deleted in favour of this. **Do not grow a second capture path anywhere** — not in a caller, not behind a flag, not "just for tests". Two paths is how a screenshot starts coming out right in one place and blank in the other.

**A file, not a key binding**, deliberately: the thing that wants the screenshot is usually not a person at a keyboard. A sentinel file works over SSH, from a Makefile, inside CI, and from an agent driving a window it does not own — no OS screen-capture, no accessibility prompt, no compositor. Do not "improve" this into an input-driven trigger.

**Gate it.** Register behind `#[cfg(debug_assertions)]`; a shipped game has no reason to watch the filesystem for a screenshot request.

The whole API is one `Path::exists` per frame. Keep it that way — a watcher thread, an inotify dependency, or a polling interval would each be a new failure mode in something whose entire value is that it always works.

## Rules

- **Bevy 0.19 is pinned.** Read the vendored source (`~/.cargo/registry/src/index.crates.io-*/bevy-0.19.0/`, and its `examples/`), not bevy.org — that documents `main` and has been wrong for this pin more than once.
- Consult Bevy documentation often. It can be found at codex_fs/offline_reference_docs/bevy-0.19-book/
- **A missing `Res<T>` panics its system in 0.19**; it does not skip. Take `Option<Res<T>>`, or `init_resource` it in the plugin that registers the reader.
- **All run conditions are evaluated — there is no short-circuit.** A bare `Res<T>` in a `.run_if(..)` closure panics whenever that resource is absent, even behind an earlier condition that returned false.
- **No `unwrap()`.** A failed capture must report, not crash the game it was only observing.
- **One path per feature.** No fallbacks, no legacy shims, no stub placeholders.

## In the monorepo

The game reaches this as `crate::devshot` (`src/lib.rs:52`); `bevy_cauldron` depends on it directly. The root `CLAUDE.md` §Screenshots explains the capture workflow and the `debug_screenshots/` convention; neither it nor `TESTING.md` is part of this mirror.
