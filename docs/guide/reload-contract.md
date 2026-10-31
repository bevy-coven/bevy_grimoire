# The reload contract

This page lists every kind of file the editor writes and says what a running program does when one of them changes on disk. It is the audit that the hot-reload series (issues #198 to #204) builds on.

The list lives in code, in `bevy_grimoire_core::reload::KINDS`, and the table below is rendered from it and from the editor's operations. A test fails when the table is out of date.

## How to read the table

- **kind** is the id an operation names in `Operation::writes`.
- **path** is relative to `<grimoire>` (the directory holding `kits.ron`, `assets/grimoire` in a default project; for a kit's `library.ron` and `project.ron`, the kit's own directory) or to `<root>` (the game's root, the directory holding `assets/`). `<name>` stands for one word of letters, digits and underscores, and for a per-map kind it is the map's name.
- **parses to** is the Rust type the file is read as.
- **class** is what a change costs the programs this repository ships (the editor, and the game-side `GrimoireLivePlugin`), the dearest of them when they differ.
- **lifetime** is what the file belongs to.
- **written by** names the operations that write it. A kind no operation writes says who does.

### Classes

| class | meaning |
|---|---|
| `Live` | The new content replaces the old in place, or nothing holds the old content because the file is read fresh each time it is used. |
| `Respawn` | What was built from the file (entities, meshes, a world) is dropped and built again. The rest of the program carries on. |
| `Restart` | A running program that holds the kind cannot take a change to it. It keeps the old content until it is restarted. |

### Lifetimes

| lifetime | meaning |
|---|---|
| `global` | One per project, kit, species or crate. It outlives every map, and a change reaches everything that reads it. |
| `per map` | One per map, named after it. It loads with its map, and a change reaches only that map. |

## The table

<!-- reload-contract:start -->
<!-- reload-contract:end -->

## Two ratchets keep the table honest

- `every_written_kind_has_a_reload_class` (`crates/bevy_grimoire_mapper/tests/reload_contract.rs`). Every operation declares the kinds it writes, and `Operation::writes` has no default, so a new operation cannot skip it. The test fails when an operation names a kind the list does not have. A second test runs an operation and checks that every file it changed is a kind it declares.
- `the_restart_list_only_shrinks` (`crates/bevy_grimoire_core/tests/reload_contract.rs`). Every `Restart` kind is on a checked-in list. A kind joins `Restart` only by editing that list and raising its ceiling, so the debt can be paid down and never grows unseen. Today it is `rigs` and `game_data`.

## A request for a kind that was never loaded

`reload_for(kind, loaded)` is the one policy. A change to a kind the program never loaded is the outcome `Reload::NotLoaded`: it is reported and nothing else happens. It is never an implicit load. A load in the middle of play is a hitch, and a program that did not ask for the file has no state to reload.

## What the audit found

These are the facts the later issues in the series start from.

- **The game's watcher sees every `.ron` under its directory** (`GrimoireLivePlugin`, the extension set `{ron}`) and runs the whole project load. It diffs placements, the library and masks. A change to a kind it does not read (notes, plans, references, forests) fires a reload that changes nothing, and a broken file of such a kind does not turn the verdict false, because nothing parses it. The class column names what each kind costs, not what the watcher notices.
- **The editor watches by name** (`watch.rs`): `kits.ron`, `vocab.ron`, `compositions.ron`, `rules.ron`, `standards.ron`, `repetition.ron`, each kit's `library.ron` and `project.ron`, and the open map. Terrain, water, volume, forest and tree files are polled by modification time in the views that draw them, and family settings by the family host every 0.1 s. Notes are polled by modification time every frame.
- **A file that does not read keeps the last good state in the project watcher and the game** (the editor says "kept what it had"). It does not everywhere: the terrain view despawns its ground and draws none. Making that uniform is #200.
- **Some writes are not atomic.** Maps, the library, compositions, rules, notes, plans and references are written to a temporary file and renamed. `family`, `game_data`, `people`, `forest`, `scripts`, `terrain`, `trees`, `volume`, `water_plan`, `water_field`, `destructible_marks` and `destructible_bake` are plain writes, so a reader can see half a file. Making them atomic is #199.
- **`game_data` is a `Restart` kind** because `data_set` writes files the game owns and this repository ships no reader for them. Whether a game re-reads them is the game's. #202 moves behaviour into data and scripts so the list shrinks.

## What is not a kind

These are written but are not project data, so they have no class. None is read back as project data.

- `<grimoire>/.live/game.ron`, the game's verdict on its latest reload. The game writes it, and the watcher skips every name that starts with a dot so it cannot trigger the reload it reports.
- `slop/llm/vocab_proposals.ron`, a VLM's label proposals, `target/grimoire_thumbs/`, `target/anim_bench_cache.ron` and `target/loop/captures/snips/`: caches and captures.
