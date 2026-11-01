# Set up your game

bevy_grimoire keeps a world in data files. The editor writes those files, and your game reads them when it starts. Your game never has to know how a map was made.

Some terms used on this page:

- A **kit** is a folder of pieces, with its own `library.ron` and `project.ron`.
- A **piece** is one thing a map can place, such as a table, a wall or a chair. It is one entry in a kit's library.
- A **placement** is one piece at one spot on a map.
- A **composition** is a named group of pieces that belong together, such as a wall with its door and lamp.
- A **stamp** is one composition placed on a map. The map keeps the stamp, and the game expands it into placements when it loads.
- A **door** is one of the editor's main areas. The map door edits a map. The kit door edits a kit.
- **bash** is a named group of bound kits. A map can belong to one bash.

## Project layout

Your game keeps its bevy_grimoire data in one folder, `assets/grimoire/`, inside the game's root.

```text
<game root>/
  assets/
    grimoire/
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

1. Add `bevy`, `bevy_grimoire` and `bevy_grimoire_core` to your game's `Cargo.toml`, by path or git to follow this repository's `main`; `bevy_grimoire_core = "0.6"` and `bevy_grimoire = "0.3"` are also on crates.io. `bevy_grimoire` is the runtime crate: `GrimoirePlugin` and the spawn helpers. `bevy_grimoire_core` loads and checks the data, with no engine dependency. A game that uses only the data, and spawns its own entities, can depend on `bevy_grimoire_core` alone.
2. Put your data files under `assets/grimoire/`, as shown above.
3. At start-up, call `project_data::load` with the `assets/grimoire` path and the names of the maps you need. The path is read from the working directory, so start the game from the game's root.
4. Take the map out of the loaded project. Build an `GrimoireWorld` from the merged library, the map, the vocabulary, the compositions, and the lattice.
5. Add `GrimoirePlugin` to your app and insert the `GrimoireWorld` as a resource.
6. Run the app. The pieces appear on the next update.

```rust
use std::path::Path;

use bevy::prelude::*;
use bevy_grimoire::{GrimoirePlugin, GrimoireWorld};
use bevy_grimoire_core::project_data;

fn main() {
    let mut data = project_data::load(Path::new("assets/grimoire"), &["town"])
        .unwrap_or_else(|e| panic!("{e}"));
    let map = data.maps.remove("town").expect("town was loaded above");

    let world = GrimoireWorld::with_compositions(
        data.bound.library,
        map,
        data.vocab,
        &data.compositions.compositions,
        data.kits.lattice,
    )
    .unwrap_or_else(|e| panic!("{e}"));

    App::new()
        .add_plugins((DefaultPlugins, GrimoirePlugin))
        .insert_resource(world)
        .run();
}
```

Use `GrimoireWorld::new` instead of `with_compositions` for a project with no compositions. It takes the same arguments, without the compositions.

**What you see.** Each placement becomes one entity. The entity carries a `Placement` with the placement's id and an `OfDescriptor` with the piece's id. It also has a `Tags` component holding the piece's vocabulary bits, so a query can test a tag with one `&` instead of comparing strings. A placement that paints a non-zero paint order also gets a `Paint` component. Pieces that need another piece under them are linked to it with `MountedOn`.

A piece with a mesh is spawned as a child scene of that mesh. A piece with only a footprint is spawned as a coloured marker, so a map still works before its meshes are ready.

**If it goes wrong.**

- The loader refuses a file. The error names the file and says what is wrong with it. The game does not start with half a project.
- A map names a piece that the library does not define. The loader refuses the map at load, and names the piece.
- A map that you ask for and that is not on disk is an error. The loader does not invent an empty map.
- A placement's host spawns no entity. That happens only when the host has neither a mesh nor a footprint. The spawner places the piece at its height, without a `MountedOn` link, and logs a warning.

## Spawn a piece yourself

`GrimoirePlugin` spawns the pieces of an `GrimoireWorld`. To spawn one piece from your own system, call `spawn_descriptor`. It takes the piece's descriptor, its tags, its position and turn, the map's origin, and its height. It returns the new entity, or nothing for a piece with neither a mesh nor a footprint.

The height depends on what a piece rests on. `bevy_grimoire_core::stack::resolve_y` answers it for a whole map at once.

## Edit the project while the game runs

The editor is a separate program. It opens a project and writes its files:

```sh
cargo run -p bevy_cauldron -- <game root> <map> --kit <kit>
```

Use `--kit` to name the kit you are working on. The editor watches the files it reads and reloads them when they change. It checks for changes four times a second.

- If you save `town.map.ron` in a text editor while the editor has it open, the editor reloads the map.
- If the file you save does not parse, the editor refuses the change. It keeps the last good state and says why.
- The editor ignores its own writes. A file whose content matches what the editor holds is not a change.

The game follows the same files if you add `GrimoireLivePlugin`:

```rust
app.add_plugins(bevy_grimoire::live::GrimoireLivePlugin {
    dir: "assets/grimoire".into(),
    maps: vec!["town".into()],
});
```

It needs an `GrimoireWorld` resource, which `GrimoirePlugin` spawns from. About a second after the last write, the plugin reloads the project and redraws only what changed.

- A placement that is new, edited, removed, or resting on something that was redrawn is despawned and spawned again. A placement that did not change keeps its `Entity`.
- A placement that only changed its id is a rename and keeps its `Entity` too: its row is the same in every other field, and no other row is the same, so it is the same piece under another name. Its `Placement` component now holds the new id and whatever you hung on the entity stays. `ProjectReloaded::diffs` come from `diff_maps_renaming`, whose `MapDiff::renamed` lists `(old id, new id)`, and neither id is in `added` or `removed` (the plain `diff_maps` takes an id for the piece and never guesses). A piece renamed and edited in one save is redrawn, and so is a pair of identical rows that were both renamed, because nothing says which is which.
- A placement whose height changed is redrawn too. Moving a table moves the lamp on it.
- A piece that is drawn again gets a new `Entity`, so what you hung on the old one is lost unless you say how it moves, once, on the app: `use bevy_grimoire::live::GrimoireLiveApp;` then `app.carry_across_respawn::<Booked>();` copies your component from the old entity to the new one, and `app.migrate_on_respawn(|world, old, new| { … })` runs your own step for state that is not a plain copy. A hook runs after the new entity is drawn and before the old one is despawned, once for each piece drawn again (a rider drawn again with its host too), and never for a piece the edit left alone or removed.
- What points at a piece follows it the same way. A person you spawned is not a placement, so a reload leaves it alone, but its `Home(entity)` names a placement, and a placement that is drawn again has a new entity. Say how a kind of reference reads the entity it names, once: `app.follow_respawn::<Home>(|home, respawned| home.0 = respawned.follow(home.0));`. After each reload that drew a piece again, the closure runs for every `Home`, and `respawned.follow(e)` is the entity that stands for `e` now (`e` itself when the piece was kept or removed, or is not a placement). A component is marked changed only when something moved. It takes a closure, not `MapEntities`, because you cannot implement that trait for another crate's component.
- Editing a descriptor redraws every placement of that descriptor. `ProjectReloaded::library_changed` says so.
- A successful reload replaces the `GrimoireWorld` resource and writes a `ProjectReloaded` message. Its `diffs` hold one `MapDiff` per map you listed, so a game can react to the ids that changed.
- If the project does not load, the plugin logs why, keeps every entity and the last good `GrimoireWorld`, and writes no `ProjectReloaded`. The reason goes to `.live/game.ron`, where the editor shows it: which file, which field of it and why (`maps/town.map.ron (placements[3].at[1]): map: 1:47: Expected float`), with an `applied` or `refused` entry for every file the load reads and a `seq` that counts one edit once. [The reload contract](reload-contract.md) has the detail.
- The map in the `GrimoireWorld` resource is the one that is redrawn. Other maps in `maps` are diffed but not drawn.
- The game's own derived data, such as a navmesh built from the map, is yours to rebuild when `ProjectReloaded` arrives.

## Generate a map

bevy_grimoire_core has three ways to fill a map. None of them needs the engine.

- **Wave function collapse.** `wfc::generate` fills a coarse room graph, such as a dungeon's rooms, from a width, a height and a seed. `wfc::collapse_grid` and `wfc::collapse_graph` are the lower-level collapsers that it is built on.
- **Grammar continuation.** `grammar` learns the rules of a map from the pieces the author has placed, and continues that map with wave function collapse.
- **Runs.** `run` repeats one piece along a path at its own pitch. Use it for fences and walls, not for a grid of cells.

A map can hold a run as itself. `Map::runs` stores each run as one `RunPlaced` (an id and a `Run`), and `Map::expanded()` lays it into rows named `<run id>/<n>`, so a fence is one entry and moving its end is one edit. The operations `run_add`, `run_set` and `run_remove` edit runs and each is undone by its `operation_id`. `placements_to_run` turns rows that `run::lay` would produce into the run that lays exactly them, and refuses, naming the first row, when one is off the pitch. In the editor, Shift-drag in run mode lays a run, dragging a run's end moves it, and Cmd-drag over rows turns them into a run.

A block laid out under `rules.ron` can be offered more than one way, on request. `rules::options(alphabet, rules, pins, seed_base, count, min_differ)` tries seeds in order, at most `attempts * count` of them, and keeps up to `count` layouts that each differ from every other kept one in at least `min_differ` cells. Each `Choice` carries the seed that made it, so `rules::solve_block` with that seed rebuilds the layout the developer picked, and `nearest` says how many cells it differs from the closest other option. Fewer than `count` is an answer, not an error: `Options::too_close` and `Options::failed` say where the seeds went.

Two supporting tools sit beside these:

- `range` measures a solved grid of cells, reporting how enclosed and how open it is.
- `placement::solver` routes each group of placement constraints to the first solver that can handle it. `bevy_grimoire_solver` is one such solver, a Boolean satisfiability back-end. It gives the same answer on every run, and it gives up after a fixed number of conflicts rather than a number of seconds.

Use `bevy_grimoire_rng::seeded(seed)` for any randomness in your game. It returns one ChaCha8 stream from a single `u64`.

## Crates

| Crate | What it does |
|---|---|
| `bevy_devshot` | A frame on demand: touch a sentinel file and the next frame writes a PNG, straight from the GPU. |
| `bevy_grimoire` | The runtime half of grimoire: load a library and a map, and put it in a Bevy world. |
| `bevy_grimoire_anim` | Blends a skinned model's clip weights on its AnimationPlayer, in place of transitions. |
| `bevy_grimoire_core` | Engine-free world-building core: constraint IR, solvers, WFC, deterministic RNG. |
| `bevy_cauldron` | A standalone world-building editor: author maps from an asset library, save what a game can load. |
| `bevy_cauldron_mcp` | An MCP server for the bevy_grimoire world editor: typed operations over stdio, and a BRP client for the running editor. |
| `bevy_grimoire_rng` | One deterministic RNG for a whole simulation: a seeded ChaCha8 stream plus the unbiased integer and float draws generators actually need, reproducible from a single u64 across platforms and thread counts. |
| `bevy_grimoire_snip` | Cutting one rectangle out of a PNG of the bevy_grimoire editor, shared by the editor and the MCP server. |
| `bevy_grimoire_solver` | A Boolean satisfiability back-end that answers the same way every run, on every machine: no clock, no threads, no entropy, and a give-up budget counted in conflicts rather than seconds. |

`bevy_grimoire_core` has no engine dependency. A test in that crate fails the build if one is added.

## Playing the map in your game

The editor never links your game. A project says how to run it in `play.ron` beside `family.ron`:

```ron
(version: 1, command: ["cargo", "run", "--", "{map}"], cwd: Some("../game"), port: 15702)
```

`{map}`, `{root}` and `{camera}` (`x,y,z`) in an argument are replaced by the open map, the project root and the editor camera's position; the game also gets them as `GRIMOIRE_PLAY_MAP`, `GRIMOIRE_PLAY_ROOT` and `GRIMOIRE_PLAY_CAMERA`. **Play** in the PLAY section of the controls panel (or the `play` operation, with `wait_s` to wait until the game's `bevy_fairy/status` says `ready: true`) starts it; **Stop** (`play_stop`) ends the process the editor started, by pid. A second Play while one runs is refused. Neither is an edit, so neither has an undo. An agent then reads the running game through `bevy_fairy`'s own MCP.

## Conversations over the speaker

With the editor built with `--features bevy_speech_bubbles` and `bevy_speech_bubbles` listed in `family.ron`, a project's conversations are `bevy_speech_bubbles`' own `DialogueScript`, saved as `family/bevy_speech_bubbles.ron`; a graph `validate_script` refuses is not written. A placed piece carries one by name (`placement_set_family` with `"crate": "bevy_speech_bubbles"` and `"value": "\"market\""`, or `@<id> bevy_speech_bubbles "market"` in FAMILY SETTINGS). Typing `dialogue market 0` in FAMILY SETTINGS walks that conversation (`dialogue_preview`), each number picking an option at the next choice (0 is the first), and draws the lines it says as one balloon over the piece carrying it; a choice with no number left stops the walk there.
