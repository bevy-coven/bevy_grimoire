# Set up your game

bevy_emerge keeps a world in data files. The editor writes those files, and your game reads them when it starts. Your game never has to know how a map was made.

Some terms used on this page:

- A **kit** is a folder of pieces, with its own `library.ron` and `project.ron`.
- A **piece** is one thing a map can place, such as a table, a wall or a chair. It is one entry in a kit's library.
- A **placement** is one piece at one spot on a map.
- A **composition** is a named group of pieces that belong together, such as a wall with its door and lamp.
- A **stamp** is one composition placed on a map. The map keeps the stamp, and the game expands it into placements when it loads.
- A **door** is one of the editor's main areas. The map door edits a map. The kit door edits a kit.
- **bash** is a named group of bound kits. A map can belong to one bash.

## Project layout

Your game keeps its bevy_emerge data in one folder, `assets/emerge/`, inside the game's root.

```text
<game root>/
  assets/
    emerge/
      kits.ron           which kits the project uses, and the lattice
      vocab.ron          the tokens every piece is described with
      compositions.ron   named groups of pieces (optional)
      rules.ron          rules the editor checks (optional)
      rigs.ron           animation rigs (optional)
      oplog.jsonl        the editor's history of changes (written by the editor)
      maps/
        town.map.ron     one file per map
      <kit>/
        library.ron      the pieces this kit defines
        project.ron      how this project uses the kit
```

A kit folder must be bound in `kits.ron` before the game can load it. See the data file reference for every file's format.

## Load a project in your game

**Goal:** the game loads one map and spawns its pieces when it starts.

1. Add `bevy`, `bevy_emerge` and `bevy_emerge_core` to your game's `Cargo.toml`. `bevy_emerge` is the runtime crate: `EmergePlugin` and the spawn helpers. `bevy_emerge_core` loads and checks the data, with no engine dependency. A game that uses only the data, and spawns its own entities, can depend on `bevy_emerge_core` alone.
2. Put your data files under `assets/emerge/`, as shown above.
3. At start-up, call `project_data::load` with the `assets/emerge` path and the names of the maps you need. The path is read from the working directory, so start the game from the game's root.
4. Take the map out of the loaded project. Build an `EmergeWorld` from the merged library, the map, the vocabulary, the compositions, and the lattice.
5. Add `EmergePlugin` to your app and insert the `EmergeWorld` as a resource.
6. Run the app. The pieces appear on the next update.

```rust
use std::path::Path;

use bevy::prelude::*;
use bevy_emerge::{EmergePlugin, EmergeWorld};
use bevy_emerge_core::project_data;

fn main() {
    let mut data = project_data::load(Path::new("assets/emerge"), &["town"])
        .unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.remove("town").expect("town was loaded above");

    let world = EmergeWorld::with_compositions(
        data.bound.library,
        map,
        data.vocab,
        &data.compositions.compositions,
        data.kits.lattice,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    App::new()
        .add_plugins((DefaultPlugins, EmergePlugin))
        .insert_resource(world)
        .run();
}
```

Use `EmergeWorld::new` instead of `with_compositions` for a project with no compositions. It takes the same arguments, without the compositions.

**What you see.** Each placement becomes one entity. The entity carries a `Placement` with the placement's id and an `OfDescriptor` with the piece's id. It also has a `Tags` component holding the piece's vocabulary bits, so a query can test a tag with one `&` instead of comparing strings. A placement that paints a non-zero paint order also gets a `Paint` component. Pieces that need another piece under them are linked to it with `MountedOn`.

A piece with a mesh is spawned as a child scene of that mesh. A piece with only a footprint is spawned as a coloured marker, so a map still works before its meshes are ready.

**If it goes wrong.**

- The loader refuses a file. The error names the file and says what is wrong with it. The game does not start with half a project.
- A map names a piece that the library does not define. The loader refuses the map at load, and names the piece.
- A map that you ask for and that is not on disk is an error. The loader does not invent an empty map.
- A placement's host spawns no entity. That happens only when the host has neither a mesh nor a footprint. The spawner places the piece at its height, without a `MountedOn` link, and logs a warning.

## Spawn a piece yourself

`EmergePlugin` spawns the pieces of an `EmergeWorld`. To spawn one piece from your own system, call `spawn_descriptor`. It takes the piece's descriptor, its tags, its position and turn, the map's origin, and its height. It returns the new entity, or nothing for a piece with neither a mesh nor a footprint.

The height depends on what a piece rests on. `bevy_emerge_core::stack::resolve_y` answers it for a whole map at once.

## Edit the project while the game runs

The editor is a separate program. It opens a project and writes its files:

```sh
cargo run -p bevy_emerge_mapper -- <game root> <map> --kit <kit>
```

Use `--kit` to name the kit you are working on. The editor watches the files it reads and reloads them when they change. It checks for changes four times a second.

- If you save `town.map.ron` in a text editor while the editor has it open, the editor reloads the map.
- If the file you save does not parse, the editor refuses the change. It keeps the last good state and says why.
- The editor ignores its own writes. A file whose content matches what the editor holds is not a change.

The game does not watch its files. It reads the project once, when it starts. To see an edit in the game, restart the game.

## Generate a map

bevy_emerge_core has three ways to fill a map. None of them needs the engine.

- **Wave function collapse.** `wfc::generate` fills a coarse room graph, such as a dungeon's rooms, from a width, a height and a seed. `wfc::collapse_grid` and `wfc::collapse_graph` are the lower-level collapsers that it is built on.
- **Grammar continuation.** `grammar` learns the rules of a map from the pieces the author has placed, and continues that map with wave function collapse.
- **Runs.** `run` repeats one piece along a path at its own pitch. Use it for fences and walls, not for a grid of cells.

Two supporting tools sit beside these:

- `range` measures a solved grid of cells, reporting how enclosed and how open it is.
- `placement::solver` routes each group of placement constraints to the first solver that can handle it. `bevy_emerge_solver` is one such solver, a Boolean satisfiability back-end. It gives the same answer on every run, and it gives up after a fixed number of conflicts rather than a number of seconds.

Use `bevy_emerge_rng::seeded(seed)` for any randomness in your game. It returns one ChaCha8 stream from a single `u64`.

## Crates

| Crate | What it does |
|---|---|
| `bevy_devshot` | A frame on demand: touch a sentinel file and the next frame writes a PNG, straight from the GPU. |
| `bevy_emerge` | The runtime half of emerge: load a library and a map, and put it in a Bevy world. |
| `bevy_emerge_anim` | Blends a skinned model's clip weights on its AnimationPlayer, in place of transitions. |
| `bevy_emerge_core` | Engine-free world-building core: constraint IR, solvers, WFC, deterministic RNG. |
| `bevy_emerge_mapper` | A standalone world-building editor: author maps from an asset library, save what a game can load. |
| `bevy_emerge_mcp` | An MCP server for the bevy_emerge world editor: typed operations over stdio, and a BRP client for the running editor. |
| `bevy_emerge_rng` | One deterministic RNG for a whole simulation: a seeded ChaCha8 stream plus the unbiased integer and float draws generators actually need, reproducible from a single u64 across platforms and thread counts. |
| `bevy_emerge_snip` | Cutting one rectangle out of a PNG of the bevy_emerge editor, shared by the editor and the MCP server. |
| `bevy_emerge_solver` | A Boolean satisfiability back-end that answers the same way every run, on every machine: no clock, no threads, no entropy, and a give-up budget counted in conflicts rather than seconds. |

`bevy_emerge_core` has no engine dependency. A test in that crate fails the build if one is added.
