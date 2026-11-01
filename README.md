# Cauldron (`bevy_grimoire`)

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

The Cauldron is a world editor for Bevy games, for developers who build levels with an AI agent, the Familiar, working beside them. A grimoire is what it writes and your game loads.

[![crates.io](https://img.shields.io/crates/v/bevy_grimoire.svg)](https://crates.io/crates/bevy_grimoire) [![docs.rs](https://img.shields.io/docsrs/bevy_grimoire_core)](https://docs.rs/bevy_grimoire_core) [![CI](https://github.com/bevy-coven/bevy_grimoire/actions/workflows/ci.yml/badge.svg)](https://github.com/bevy-coven/bevy_grimoire/actions/workflows/ci.yml) [![Bevy 0.20](https://img.shields.io/badge/bevy-0.20-purple.svg)](https://bevyengine.org) [![licence](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue.svg)](#who-and-licence)

![The editor's 3D view of the bedroom map that ships in this repository: an AI agent removes the bed and then turns the sofa 90 degrees through MCP calls; then the person presses Ctrl+Z twice and takes both back](https://raw.githubusercontent.com/bevy-coven/bevy_grimoire/main/docs/media/agent-drives.gif)

*An agent calls `grimoire_apply_operation` twice, a removal and then a turn, and two presses of Cmd+Z (Ctrl+Z on Linux and Windows, as recorded here) take them back; the still below shows the inspector naming each change and who made it. A 64-second recording with snips is [on the book page](https://bevy-coven.github.io/crates/bevy_grimoire.html#the-demo).*

**Status:** usable · Bevy 0.20 · toolchain 1.98.1 (pinned in `rust-toolchain.toml`)

**Why:** a level editor where an agent's change is the same kind of change as yours, through the same named operations, in the same undo history, so you can let it build while you watch, snip what is wrong and take back what you do not want.

## Quick start

```sh
git clone https://github.com/bevy-coven/bevy_grimoire
cd bevy_grimoire
cargo run -p bevy_cauldron -- .
```

The editor opens a menu of the project's kits and maps; choose a map in the MAPS column. On your own game, pass its root instead of `.`; a project from before the rename has its `assets/emerge` folder moved to `assets/grimoire` the first time it opens. [Getting started](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/getting-started.md) walks through your first map.

To let the Familiar drive the open editor, add the MCP server to your MCP client (the editor must be running):

```json
{ "mcpServers": { "bevy_grimoire": {
    "command": "cargo", "args": ["run", "-q", "-p", "bevy_cauldron_mcp", "--", "--project", "/path/to/project"],
    "cwd": "/path/to/bevy_grimoire" } } }
```

## Try it

```sh
cargo run -p bevy_cauldron -- . --kit furniture
```

Opens the repository's own project on its `furniture` kit (`site` and `scp` are the others); choose `bedroom` in the MAPS column to open the one-room map that ships with the repository, or `+ new map` for an empty one. [The MCP tools](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/mcp-tools.md) list what an agent can call.

![The whole editor window on the Map door with the bedroom map, after an agent removed the bed and turned the sofa 90 degrees: the inspector's LAST line and its history list name both changes as the agent's, and the PLACE palette lists the furniture kit](https://raw.githubusercontent.com/bevy-coven/bevy_grimoire/main/docs/media/editor.png)

*The same window mid-clip: every change in the history list says who made it.*

## What you can do

- **Build a map**: place pieces, fill areas, turn and remove them, and undo any of it: [building maps](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/building-maps.md).

  ![The Map door's 3D view and PLACE list on the bedroom map: the list is filtered to the sofa and the sofa armed, R turns it a quarter turn, a click places it, X and a click remove it, and Ctrl+Z puts it back](https://raw.githubusercontent.com/bevy-coven/bevy_grimoire/main/docs/media/build-a-map.gif)

- **Work with the Familiar**: an agent turns, removes, labels, frames and captures through the same operations you use, and one Cmd+Z takes back its turn: [agents](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/agents.md).
- **Show the agent what you mean**: Cmd+\ snips a rectangle of the editor with a note the agent can read and see: [agents](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/agents.md).
- **Continue a layout you started**: the editor learns the rules of a corner you laid out and continues it with WFC (wave function collapse), drawn as translucent pieces before you keep it: [building maps](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/building-maps.md).
- **Make kits and labels**: bring in meshes, group them into kits, and confirm the labels a vision model proposes: [kits and labels](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/kits-and-labels.md).
- **Edit the files your game loads, live**: the editor re-reads changed project files every quarter of a second, and a game can reload them too: [data files](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/data-files.md), [your game](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/your-game.md).
- **Configure the rest of the Coven**: sky, forests, lights, villagers, dialogue and destructible props, each as that crate's own settings: [the book](https://bevy-coven.github.io/with-grimoire.html).

## Part of the Coven

The Cauldron is the editor of the [Bevy Coven](https://github.com/bevy-coven): it takes each family crate as an optional cargo feature and edits that crate's own serde settings, so what it saves is what your game loads with the crate alone. [Using a crate with Grimoire](https://bevy-coven.github.io/with-grimoire.html) explains the arrangement.

## The crates

| Crate | What it is |
|---|---|
| `bevy_cauldron` | The editor. Opens a project directory and writes the files a game loads. Not on crates.io: run it from a clone. |
| `bevy_cauldron_mcp` | The MCP server that gives an agent the editor's operations over stdio. |
| [`bevy_grimoire`](https://crates.io/crates/bevy_grimoire) | The runtime half: loads a library and a map into a Bevy world (`GrimoirePlugin`, `GrimoireLivePlugin`). |
| [`bevy_grimoire_core`](https://crates.io/crates/bevy_grimoire_core) | The engine-free core: the constraint representation, solvers, WFC and the grammar learner. |
| [`bevy_grimoire_solver`](https://crates.io/crates/bevy_grimoire_solver), [`_rng`](https://crates.io/crates/bevy_grimoire_rng), [`_anim`](https://crates.io/crates/bevy_grimoire_anim), [`_snip`](https://crates.io/crates/bevy_grimoire_snip) | A deterministic SAT back-end, one ChaCha8 random stream, clip-weight blending, and cutting a rectangle out of a picture of the editor. |
| `bevy_devshot` | A frame on demand: touch a sentinel file and the next frame is written as a PNG. |

A game depends on `bevy_grimoire_core` and `bevy_grimoire` (`cargo add bevy_grimoire_core bevy_grimoire`). The crates were called `bevy_emerge_*` until 2026-10-31; those names have a final release that points here.

## Bevy compatibility

| `bevy_grimoire_*` | Bevy |
|---|---|
| 0.1 | 0.20 |

## Learn more

- [The guide](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/README.md): every page, in reading order.
- [The book page](https://bevy-coven.github.io/crates/bevy_grimoire.html), with the 64-second demo.
- [MCP tools](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/mcp-tools.md), [`CHANGELOG.md`](https://github.com/bevy-coven/bevy_grimoire/blob/main/CHANGELOG.md) (every visible change has a clip), and [docs.rs/bevy_grimoire_core](https://docs.rs/bevy_grimoire_core).
- [The development loop](https://github.com/bevy-coven/bevy_grimoire/blob/main/docs/guide/loop.md): how changes are proposed, tested and reported.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues and pull requests are welcome at [github.com/bevy-coven/bevy_grimoire/issues](https://github.com/bevy-coven/bevy_grimoire/issues); a game asks for what it needs with the `consumer` label. Read the org's [CONTRIBUTING.md](https://github.com/bevy-coven/.github/blob/main/CONTRIBUTING.md) and [`AGENTS.md`](https://github.com/bevy-coven/bevy_grimoire/blob/main/AGENTS.md) first, and run `cargo test --workspace` before you push.

MIT OR Apache-2.0, at your option.
